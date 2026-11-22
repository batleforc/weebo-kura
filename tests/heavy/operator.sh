#!/usr/bin/env bash
# RFC 0009 §6.P on kind: the kura chart and kura-operator's chart, the real
# images, kubectl. Observations 1 to 6 (see the RFC), each printing ok or
# failing the run. No Authentik and no Vault in the cluster: the server takes
# its database from KURA_DATABASE_URL, and what the RFC observes through the
# kura CLI as alice is observed in PostgreSQL instead (plan, §6.P notes).
#
# Needs kind, kubectl, helm, podman and the two images (task operator:proof
# builds them). KEEP=1 leaves the cluster up; it is reused when it exists.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
cluster=${KURA_PROOF_CLUSTER:-kura-proof}
ctx="kind-$cluster"
server_image=${SERVER_IMAGE:-localhost/kura-server:proof}
operator_image=${OPERATOR_IMAGE:-localhost/kura-operator:proof}
cert_manager=${CERT_MANAGER_VERSION:-v1.21.3}
export KIND_EXPERIMENTAL_PROVIDER=${KIND_EXPERIMENTAL_PROVIDER:-podman}
work=$(mktemp -d)
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }
k() { kubectl --context "$ctx" "$@"; }
sql() { k -n kura exec deploy/postgres -- psql -U kura_owner -d kura -tAc "$1"; }
generation() { sql "SELECT encode(generation, 'hex') FROM config_state ORDER BY id DESC LIMIT 1"; }
cleanup() {
  for pid in $(jobs -p); do kill "$pid" 2>/dev/null || true; done
  rm -rf "$work"
  [ "${KEEP:-0}" = 1 ] || kind delete cluster --name "$cluster" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# ── The cluster, the images, cert-manager ────────────────────────────────────
kind get clusters 2>/dev/null | grep -qx "$cluster" || kind create cluster --name "$cluster" --wait 180s
for image in "$server_image" "$operator_image"; do
  rm -f "$work/image.tar"
  podman save -q "$image" -o "$work/image.tar"
  kind load image-archive "$work/image.tar" --name "$cluster" >/dev/null
done
helm --kube-context "$ctx" upgrade --install cert-manager oci://quay.io/jetstack/charts/cert-manager \
  --version "$cert_manager" --namespace cert-manager --create-namespace --set crds.enabled=true --wait >/dev/null

# ── PostgreSQL (one pod) and its roles ───────────────────────────────────────
k create namespace kura --dry-run=client -o yaml | k apply -f - >/dev/null
k -n kura apply -f - >/dev/null <<'YAML'
apiVersion: apps/v1
kind: Deployment
metadata: { name: postgres }
spec:
  selector: { matchLabels: { app: postgres } }
  template:
    metadata: { labels: { app: postgres } }
    spec:
      containers:
        - name: postgres
          image: docker.io/library/postgres:18-alpine
          env:
            - { name: POSTGRES_USER, value: kura_owner }
            - { name: POSTGRES_PASSWORD, value: proof }
            - { name: POSTGRES_DB, value: kura }
          readinessProbe: { exec: { command: [pg_isready, -U, kura_owner] } }
---
apiVersion: v1
kind: Service
metadata: { name: postgres }
spec: { selector: { app: postgres }, ports: [{ port: 5432 }] }
---
apiVersion: v1
kind: Secret
metadata: { name: kura-db-owner }
stringData: { uri: "postgres://kura_owner:proof@postgres:5432/kura" }
---
apiVersion: v1
kind: Secret
metadata: { name: kura-db-app }
stringData: { uri: "postgres://kura_app:proof@postgres:5432/kura" }
YAML
k -n kura rollout status deploy/postgres --timeout 180s >/dev/null
k -n kura exec -i deploy/postgres -- psql -q -U kura_owner -d kura < "$root/crates/kura-server/sql/bootstrap-roles.sql" >/dev/null
sql "ALTER ROLE kura_app PASSWORD 'proof'" >/dev/null

# ── kura-operator, then kura-server in files mode (the migration's step 1) ───
helm dependency build --skip-refresh "$root/deploy/charts/kura-operator" >/dev/null
helm --kube-context "$ctx" upgrade --install kura-operator "$root/deploy/charts/kura-operator" \
  --namespace kura-operator --create-namespace --set replicaCount=1 \
  --set image.repository="${operator_image%:*}" --set image.tag="${operator_image##*:}" \
  --set image.pullPolicy=Never --wait --timeout 5m >/dev/null

cat > "$work/kura.yaml" <<YAML
image: { repository: ${server_image%:*}, tag: ${server_image##*:}, pullPolicy: Never }
replicas: 1
backupsWaived: true
cnpg: { enabled: false }
migrate: { databaseUrlSecret: { name: kura-db-owner, key: uri } }
extraEnv:
  - name: KURA_DATABASE_URL
    valueFrom: { secretKeyRef: { name: kura-db-app, key: uri } }
ingress: { enabled: false }
monitoring: { serviceMonitor: { enabled: false }, prometheusRule: { enabled: false } }
networkPolicy: { operatorNamespace: kura-operator }
server:
  vault: null
  oidc:
    issuer: https://auth.invalid/application/o/kura/
    discovery: null
YAML
helm_kura() { helm --kube-context "$ctx" upgrade --install kura "$root/deploy/helm/kura" --namespace kura -f "$work/kura.yaml" "$@" >/dev/null; }
helm_kura --set-file "documents.tenants\.yaml=$root/dev/config/tenants.yaml"
k -n kura wait --for=condition=complete job -l app.kubernetes.io/component=migrate --timeout 300s >/dev/null
k -n kura rollout status deploy/kura --timeout 300s >/dev/null

for ns in dev jdr other team-b; do k create namespace "$ns" --dry-run=client -o yaml | k apply -f - >/dev/null; done
k apply -f - >/dev/null <<'YAML'
apiVersion: kura.weebo.io/v1alpha1
kind: KuraInstance
metadata: { name: kura }
spec:
  url: http://kura.kura.svc:8080
  auth: { serviceAccountToken: { audience: kura } }
YAML
k apply -f "$root/dev/config/tenants.yaml" >/dev/null

# 4. Files mode: every resource says so, the refused pushes are counted, the
# stored configuration is the files'.
reason() { k get "$1" "${@:2}" -o jsonpath='{.status.conditions[?(@.type=="Ready")].reason}'; }
for _ in $(seq 60); do [ "$(reason kuravault notes -n dev)" = SourceIsFiles ] && break; sleep 1; done
[ "$(reason kuravault notes -n dev)" = SourceIsFiles ] || fail "4: notes shows $(reason kuravault notes -n dev)"
[ "$(reason kuratenant dev)" = SourceIsFiles ] || fail "4: the tenant shows $(reason kuratenant dev)"
k -n kura port-forward svc/kura 19090:9090 18080:8080 >/dev/null 2>&1 &
for _ in $(seq 30); do curl -sf http://127.0.0.1:18080/readyz >/dev/null && break; sleep 1; done
refused=$(curl -sf http://127.0.0.1:19090/metrics | sed -n 's/^kura_documents_pushes_total{outcome="refused"} //p')
[ "${refused:-0}" -ge 1 ] || fail "4: kura_documents_pushes_total{refused} is ${refused:-absent}"
files_generation=$(generation)
pass "4: files mode, every resource SourceIsFiles, $refused refused pushes counted, generation ${files_generation:0:12} kept"

# ── The switch (the migration's step 3) ──────────────────────────────────────
kill %1 2>/dev/null || true
helm_kura --set server.documents.source=operator \
  --set server.documents.operator.issuer=https://kubernetes.default.svc.cluster.local \
  --set server.documents.operator.audience=kura \
  --set server.documents.operator.subject=system:serviceaccount:kura-operator:kura-operator
k -n kura rollout status deploy/kura --timeout 300s >/dev/null
k -n kura port-forward svc/kura 19090:9090 18080:8080 >/dev/null 2>&1 &
for _ in $(seq 30); do curl -sf http://127.0.0.1:18080/readyz >/dev/null && break; sleep 1; done

# 1. The YAML of the local stack, unchanged, as resources: every one Ready,
# the vaults enabled in PostgreSQL, nothing removed by the switch.
k wait --for=condition=Ready kuravault --all -n dev --timeout 120s >/dev/null || fail "1: the dev vaults are not Ready"
k wait --for=condition=Ready kuratenant --all --timeout 60s >/dev/null || fail "1: the tenants are not Ready"
[ "$(generation)" != "$files_generation" ] || fail "1: the operator's set was not written"
enabled=$(sql "SELECT count(*) FROM vault WHERE disabled_at IS NULL")
disabled=$(sql "SELECT count(*) FROM vault WHERE disabled_at IS NOT NULL")
[ "$enabled" = 6 ] && [ "$disabled" = 0 ] || fail "1: $enabled enabled and $disabled disabled vaults, want 6 and 0"
source=$(k get kuravault notes -n dev -o jsonpath='{.status.instance.name}')
[ "$source" = kura ] || fail "1: notes is bound to '$source'"
pass "1: the dev documents as resources, all Ready, 6 vaults enabled, none removed by the switch"

# 2. A vault in a namespace its tenant does not list: refused by the webhook,
# nothing reached Kura.
before=$(generation)
out=$(k apply -f - 2>&1 <<'YAML' || true
apiVersion: kura.weebo.io/v1alpha1
kind: KuraVault
metadata: { name: recipes, namespace: team-b }
spec: { tenantRef: { name: dev } }
YAML
)
grep -q 'denied the request: VaultNamespaceNotAllowed' <<<"$out" || fail "2: kubectl said: $out"
[ -z "$(k get kuravault -n team-b -o name)" ] || fail "2: the vault exists in team-b"
[ "$(sql "SELECT count(*) FROM vault WHERE namespace = 'team-b'")" = 0 ] || fail "2: team-b reached PostgreSQL"
[ "$(generation)" = "$before" ] || fail "2: the stored configuration moved"
pass "2: VaultNamespaceNotAllowed at kubectl apply, nothing in Kubernetes or PostgreSQL"

# 3. No other identity writes the documents: another ServiceAccount's token
# for the right audience (403), a token for the API server's audience (401),
# no token at all (401). The stored generation does not move.
other=$(k create token default -n default --audience kura)
apiserver=$(k create token default -n default)
put() { curl -s -o /dev/null -w '%{http_code}' -X PUT -H 'content-type: application/json' "$@" \
  --data '{"documents":[]}' http://127.0.0.1:18080/v1/admin/documents; }
[ "$(put -H "authorization: Bearer $other")" = 403 ] || fail "3: another ServiceAccount was not refused with 403"
[ "$(put -H "authorization: Bearer $apiserver")" = 401 ] || fail "3: a token of another audience was not refused with 401"
[ "$(put)" = 401 ] || fail "3: no token was not refused with 401"
[ "$(generation)" = "$before" ] || fail "3: the stored configuration moved"
pass "3: 403 for another ServiceAccount, 401 for another audience and for no token, generation unchanged"

# 5. Removal is the server's, and reversible: Orphan disables, the same file
# brings the vault back with its row.
id=$(sql "SELECT id FROM vault WHERE namespace = 'dev' AND name = 'notes'")
k delete kuravault notes -n dev >/dev/null
for _ in $(seq 30); do [ "$(sql "SELECT disabled_at IS NOT NULL FROM vault WHERE id = '$id'")" = t ] && break; sleep 1; done
[ "$(sql "SELECT disabled_at IS NOT NULL FROM vault WHERE id = '$id'")" = t ] || fail "5: notes was not disabled"
k apply -f "$root/dev/config/tenants.yaml" >/dev/null
k wait --for=condition=Ready kuravault notes -n dev --timeout 60s >/dev/null || fail "5: notes did not come back"
[ "$(sql "SELECT disabled_at IS NULL FROM vault WHERE id = '$id'")" = t ] || fail "5: notes is still disabled"
pass "5: notes disabled on delete (Orphan), back with the same id on apply"

# 6. The operator down: Kura keeps the last set; back up, the edit lands.
binding() { sql "SELECT count(*) FROM role_binding b JOIN vault v ON v.id = b.vault_id WHERE v.namespace = 'dev' AND v.name = 'shared' AND b.subject = 'proof-subject'"; }
k -n kura-operator scale deploy/kura-operator --replicas=0 >/dev/null
k -n kura-operator wait --for=delete pod -l app.kubernetes.io/name=kura-operator --timeout 60s >/dev/null || true
k -n dev patch kuravault shared --type json \
  -p '[{"op":"add","path":"/spec/bindings/-","value":{"role":"viewer","subject":"proof-subject"}}]' --dry-run=server >/dev/null 2>&1 \
  && fail "6: the edit was admitted with the operator down (failurePolicy Fail expected)"
[ "$(binding)" = 0 ] || fail "6: the binding exists already"
[ "$(sql "SELECT count(*) FROM vault WHERE disabled_at IS NULL")" = 6 ] || fail "6: Kura lost vaults with the operator down"
k -n kura-operator scale deploy/kura-operator --replicas=1 >/dev/null
k -n kura-operator rollout status deploy/kura-operator --timeout 120s >/dev/null
started=$(date +%s)
k -n dev patch kuravault shared --type json \
  -p '[{"op":"add","path":"/spec/bindings/-","value":{"role":"viewer","subject":"proof-subject"}}]' >/dev/null
for _ in $(seq 30); do [ "$(binding)" = 1 ] && break; sleep 1; done
[ "$(binding)" = 1 ] || fail "6: the edit did not land after the operator came back"
pass "6: operator down, Kura kept its 6 vaults and refused the edit at admission; back up, the edit landed in $(( $(date +%s) - started )) s"
