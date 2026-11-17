#!/usr/bin/env bash
# RFC 0003 §6.P observations 4 to 7 against the local stack (task dev:stack,
# task ui:build, then task dev:run with dev/server.yaml, which enables
# notifications for 127.0.0.1/32 and trusts dev/certs/ca.pem). The receiver is
# a Python https listener with the dev certificate on 127.0.0.1:8443, logging
# each request; a second one on 127.0.0.2:8443 must never be reached.
# Observations 6 (the half at send time) and 8 start two more servers on the
# same database with other settings, on ports 58081 and 58082.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
server=${KURA_DEV_SERVER:-http://127.0.0.1:58080}
ak=${KURA_DEV_AUTHENTIK:-https://localhost:59443}
issuer=$ak/application/o/kura/
work=$(mktemp -d)
pids=()
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }
sql() { podman exec kura-dev-postgres psql -U postgres -d kura -tAc "$1"; }
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  sql "DELETE FROM notification_rule WHERE name LIKE 'hub-%' OR name = 'inside-$$'" >/dev/null || true
  sql "UPDATE device SET revoked_at = now() WHERE revoked_at IS NULL AND name LIKE 'notify-%-$$'" >/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

curl -sf "$server/readyz" >/dev/null || fail "no server on $server: task dev:run"
cargo build -q -p kura-cli

# The receivers: one log line of JSON per request; status from a file.
cat > "$work/receiver.py" <<'PY'
import http.server, json, ssl, sys
host, log, status, cert, key = sys.argv[1:6]
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("content-length", 0)))
        with open(log, "a") as f:
            f.write(json.dumps({"headers": {k.lower(): v for k, v in self.headers.items()}, "body": body.decode()}) + "\n")
        self.send_response(int(open(status).read().strip()))
        self.send_header("content-length", "0")
        self.end_headers()
    def log_message(self, *a):
        pass
s = http.server.HTTPServer((host, 8443), H)
c = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
c.load_cert_chain(cert, key)
s.socket = c.wrap_socket(s.socket, server_side=True)
s.serve_forever()
PY
echo 200 > "$work/status"
for host in 127.0.0.1 127.0.0.2; do
  : > "$work/log-$host"
  python3 -I "$work/receiver.py" "$host" "$work/log-$host" "$work/status" "$root/dev/certs/localhost.pem" "$root/dev/certs/localhost.key" &
  pids+=($!)
done
sleep 1

token=$("$root/dev/token.sh" alice)
KURA_CLI_HOME="$work/alice" KURA_ACCESS_TOKEN="$token" "$root/target/debug/kura" login --server "$server" --issuer "$issuer" --device-name "notify-alice-$$" >/dev/null
device=$(jq -r .device "$work/alice/config.json")
api() { curl -s -o "$work/body" -w '%{http_code}' -H "authorization: Bearer $token" -H "x-kura-device: $device" -H 'content-type: application/json' "$@"; }
[ "$(api "$server/v1/tenants/dev/notifications")" = 200 ] || fail "notifications are off on $server: $(cat "$work/body")"

# 4. From the console: a rule with an authorization value, tested.
(cd "$root/ui" && KURA_E2E_HOOK=https://127.0.0.1:8443/hook KURA_E2E_SECRET_FILE="$work/secret" KURA_E2E_AUTHENTIK=$ak \
  KURA_DEV_SERVER=$server pnpm exec playwright test e2e/notify.spec.ts --reporter=line) || fail "4: the console could not create and test a rule"
[ "$(wc -l < "$work/log-127.0.0.1")" = 1 ] || fail "4: the receiver logged $(wc -l < "$work/log-127.0.0.1") requests"
head -1 "$work/log-127.0.0.1" > "$work/ping"
[ "$(jq -r '.headers["x-kura-event"]' "$work/ping")" = ping ] || fail "4: $(cat "$work/ping")"
[ "$(jq -r '.headers.authorization' "$work/ping")" = "Bearer test-key" ] || fail "4: no authorization header"
secret=$(cat "$work/secret")
hexkey=$(printf '%s' "${secret#whsec_}" | tr '_-' '/+' | awk '{ n = length($0) % 4; if (n) $0 = $0 substr("===", 1, 4 - n); print }' | base64 -d | od -An -tx1 | tr -d ' \n')
ts=$(jq -r '.headers["x-kura-timestamp"]' "$work/ping")
jq -j .body "$work/ping" > "$work/ping-body"
mac=$( { printf '%s.' "$ts"; cat "$work/ping-body"; } | openssl dgst -sha256 -mac HMAC -macopt "hexkey:$hexkey" | awk '{print $NF}')
[ "v1=$mac" = "$(jq -r '.headers["x-kura-signature"]' "$work/ping")" ] || fail "4: the signature does not match openssl's HMAC"
api "$server/v1/tenants/dev/notifications" >/dev/null
! grep -q 'test-key\|/hook\|whsec_' "$work/body" || fail "4: the rules list shows a secret: $(cat "$work/body")"
pass "4: Test reaches the receiver with Bearer test-key, openssl recomputes the signature, the list shows none of it"

# 5. A device revoked: device.revoked within 10 s, ids only.
other=$("$root/dev/token.sh" alice)
KURA_CLI_HOME="$work/alice2" KURA_ACCESS_TOKEN="$other" "$root/target/debug/kura" login --server "$server" --issuer "$issuer" --device-name "notify-other-$$" >/dev/null
spare=$(jq -r .device "$work/alice2/config.json")
[ "$(api -X DELETE "$server/v1/devices/$spare")" = 204 ] || fail "5: revocation $(cat "$work/body")"
for _ in $(seq 10); do grep -q '"x-kura-event": "device.revoked"' "$work/log-127.0.0.1" && break; sleep 1; done
grep '"x-kura-event": "device.revoked"' "$work/log-127.0.0.1" | head -1 | jq -r .body > "$work/revoked"
[ -s "$work/revoked" ] || fail "5: no device.revoked within 10 s"
[ "$(jq -r .data.device.id "$work/revoked")" = "$spare" ] || fail "5: $(cat "$work/revoked")"
[ "$(jq '[paths | .[-1] | select(. == "sub" or . == "path")] | length' "$work/revoked")" = 0 ] || fail "5: a subject or path key: $(cat "$work/revoked")"
[ "$(jq '.data.device | has("name")' "$work/revoked")" = false ] || fail "5: the device's name was sent"
pass "5: device.revoked arrives within 10 s with the device's id and kind, no subject, path or name"

# 6. An address not allowed: refused, and never connected to.
[ "$(api -d "{\"name\":\"inside-$$\",\"events\":[\"ping\"],\"channel\":{\"type\":\"webhook\",\"url\":\"https://127.0.0.2:8443/hook\"}}" "$server/v1/tenants/dev/notifications")" = 400 ] \
  || fail "6: $(cat "$work/body")"
grep -q 'this address cannot be used' "$work/body" || fail "6: $(cat "$work/body")"
[ ! -s "$work/log-127.0.0.2" ] || fail "6: 127.0.0.2 was reached"
pass "6: a rule to 127.0.0.2 is 400 with the one message, and that listener logged nothing"

# 7. The receiver fails: one attempt, pending, retried in about a minute.
echo 500 > "$work/status"
rule=$(sql "SELECT id FROM notification_rule WHERE name LIKE 'hub-%' ORDER BY created_at DESC LIMIT 1")
[ "$(api -X POST "$server/v1/tenants/dev/notifications/$rule/test")" = 200 ] || fail "7: $(cat "$work/body")"
[ "$(jq -r .result "$work/body")" = status_5xx ] || fail "7: $(cat "$work/body")"
row=$(sql "SELECT d.attempts || ' ' || d.status || ' ' || round(extract(epoch FROM d.next_attempt_at - now())) FROM notification_delivery d WHERE d.rule_id = '$rule' ORDER BY d.updated_at DESC LIMIT 1")
read -r attempts status wait <<<"$row"
[ "$attempts" = 1 ] && [ "$status" = pending ] && [ "$wait" -ge 50 ] && [ "$wait" -le 65 ] || fail "7: $row"
pass "7: a 500 leaves the delivery pending after 1 attempt, next in ${wait} s"
# 6 and 8: the same database, other settings. `serve <port> <sed script>`
# starts a server with dev/server.yaml changed by the script.
serve() {
  local port=$1 metrics=$(( $1 + 1000 ))
  sed -e "s/127.0.0.1:58080/127.0.0.1:$port/; s/127.0.0.1:59090/127.0.0.1:$metrics/" -e "$2" "$root/dev/server.yaml" > "$work/server-$port.yaml"
  (cd "$root" && KURA_VAULT_TOKEN=kura-dev KURA_CONFIG_DIR=dev/config SSL_CERT_FILE=dev/certs/ca.pem \
    ./target/debug/kura-server --config "$work/server-$port.yaml" > "$work/server-$port.log" 2>&1) &
  pids+=($!)
  for _ in $(seq 60); do curl -sf "http://127.0.0.1:$port/readyz" >/dev/null && return 0; sleep 1; done
  fail "the server on $port did not start: $(tail -3 "$work/server-$port.log")"
}
cargo build -q -p kura-server
# The console off, and no private range allowed any more.
serve 58081 '/^ui:/,/oidc:/ s/enabled: true/enabled: false/; s#allowedCidrs: \[127.0.0.1/32\]#allowedCidrs: []#'
[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:58081/app/)" = 404 ] || fail "8: /app/ is served with ui.enabled false"
pass "8: with ui.enabled false, /app/ is 404"
before=$(wc -l < "$work/log-127.0.0.1")
echo 200 > "$work/status"
status=$(curl -s -o "$work/body" -w '%{http_code}' -H "authorization: Bearer $token" -H "x-kura-device: $device" -X POST "http://127.0.0.1:58081/v1/tenants/dev/notifications/$rule/test")
[ "$status" = 200 ] && [ "$(jq -r .result "$work/body")" = address_refused ] || fail "6: $status $(cat "$work/body")"
[ "$(wc -l < "$work/log-127.0.0.1")" = "$before" ] || fail "6: the refused address was connected to"
pass "6: a rule saved while 127.0.0.1 was allowed is refused at send time once it is not (address_refused), and nothing connects"
# Notifications off.
serve 58082 '/^notifications:/,/caFile/ s/enabled: true/enabled: false/'
status=$(curl -s -o "$work/body" -w '%{http_code}' -H "authorization: Bearer $token" -H "x-kura-device: $device" "http://127.0.0.1:58082/v1/tenants/dev/notifications")
if [ "$status" != 409 ] || ! jq -e '.type == "/v1/problems/notifications_disabled"' "$work/body" >/dev/null; then
  fail "8: $status $(cat "$work/body")"
fi
pass "8: with notifications.enabled false, the rule routes are 409 notifications_disabled"
echo "notify.sh: RFC 0003 §6.P observations 4 to 8 hold on $server"
