#!/usr/bin/env bash
# RFC 0006 §6.P against the local stack (task dev:stack, then task dev:run):
# where each device stands on a vault, with the real `kura` binary,
# Authentik, PostgreSQL, OpenBao and RustFS. In the tenant `dev`, alice owns
# the tenant (every device, owners named), bob is a member (his own only).
# Vault `notes` only; `elsewhere` (tenant `other`) for the stranger.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
kura="$root/target/debug/kura"
server=${KURA_DEV_SERVER:-http://127.0.0.1:58080}
issuer=https://localhost:59443/application/o/kura/
work=$(mktemp -d)
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }
sql() { podman exec kura-dev-postgres psql -U postgres -d kura -tAc "$1"; }
as() { # as <who> <kura args…>
  local who=$1; shift
  KURA_CLI_HOME="$work/$who" KURA_ACCESS_TOKEN="${tokens[$who]}" "$kura" "$@"
}
device() { jq -r .device "$work/$1/config.json"; }
api() { # api <who> <method> <path> [json]: body, then " http=<status>"
  local who=$1 method=$2 path=$3
  curl -s -w ' http=%{http_code}' -X "$method" "$server/v1$path" \
    -H "authorization: Bearer ${tokens[$who]}" -H "x-kura-device: $(device "$who")" \
    ${4:+-H 'content-type: application/json' -d "$4"}
}
status() { echo "${1##* http=}"; }
body() { echo "${1% http=*}"; }

curl -sf "$server/readyz" >/dev/null || fail "no server on $server: task dev:run"
cargo build -q -p kura-cli
declare -A tokens=([alice]=$("$root/dev/token.sh" alice) [bob]=$("$root/dev/token.sh" bob))
as alice login --server "$server" --issuer "$issuer" --device-name "heavy-sync-a-$$" >/dev/null
as bob login --server "$server" --issuer "$issuer" --device-name "heavy-sync-b-$$" >/dev/null
cleanup() {
  as alice devices revoke "heavy-sync-a-$$" >/dev/null 2>&1 || true
  as bob devices revoke "heavy-sync-b-$$" >/dev/null 2>&1 || true
  sql "UPDATE scoped_token SET revoked_at = now() WHERE revoked_at IS NULL AND name = 'heavy-sync-$$'" >/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT
notes=$(sql "SELECT id FROM vault WHERE namespace = 'dev' AND name = 'notes' AND disabled_at IS NULL")
elsewhere=$(sql "SELECT id FROM vault WHERE namespace = 'other' AND name = 'elsewhere' AND disabled_at IS NULL")
[ -n "$notes" ] && [ -n "$elsewhere" ] || fail "no vault notes or elsewhere: restart task dev:run"
row() { jq -c --arg d "$2" '.[] | select(.device == $d)' <<<"$1"; }

# 1. A full pass records the head: bob's row, up to date, with his run's counts.
mkdir -p "$work/b" "$work/a"
printf 'cannot sync\n' > "$work/b/Q3: plan-$$.md"
as bob attach notes "$work/b" --yes >/dev/null || true
r=$(api bob GET "/vaults/$notes/sync"); [ "$(status "$r")" = 200 ] || fail "1: $r"
mine=$(row "$(body "$r")" "$(device bob)")
[ "$(jq -r .behind <<<"$mine")" = 0 ] || fail "1: bob is not up to date: $mine"
[ "$(jq -r .report.unsyncable <<<"$mine")" = 1 ] || fail "1: the run's counts did not arrive: $mine"
[ "$(jq -r '.owner // "none"' <<<"$mine")" = none ] || fail "1: a member is told owners: $mine"
pass "1: bob's pass recorded, up to date, 1 unsyncable reported, no owner named"

# 2. alice pushes three notes; bob's row says 3 behind with no call from bob.
for i in 1 2 3; do printf 'note %s\n' "$i" > "$work/a/heavy-devices-$$-$i.md"; done
as alice attach notes "$work/a" --yes >/dev/null
r=$(api alice GET "/vaults/$notes/sync"); [ "$(status "$r")" = 200 ] || fail "2: $r"
his=$(row "$(body "$r")" "$(device bob)")
behind=$(jq -r .behind <<<"$his")
[ "$behind" -ge 3 ] || fail "2: bob is $behind behind, expected at least 3: $his"
# The owner: the OIDC subject (Authentik's hashed `sub`), and the name from bob's own token (D13).
[ -n "$(jq -r '.owner // empty' <<<"$his")" ] || fail "2: the owner is not named to the tenant owner: $his"
[ "$(jq -r '.ownerName // empty' <<<"$his")" = Bob ] || fail "2: bob's name is not shown: $his"
[ "$(jq -r '.pushedAt // empty' <<<"$(row "$(body "$r")" "$(device alice)")")" != "" ] || fail "2: alice's push not recorded"
pass "2: bob is $behind behind after alice's push, named Bob to alice"

# 3. bob syncs again: up to date.
as bob sync notes >/dev/null || true
r=$(api bob GET "/vaults/$notes/sync")
[ "$(jq -r .behind <<<"$(row "$(body "$r")" "$(device bob)")")" = 0 ] || fail "3: bob still behind: $(body "$r")"
[ -z "$(row "$(body "$r")" "$(device alice)")" ] || fail "3: bob, a member, sees alice's device: $(body "$r")"
[ "$(jq '[.[] | select(.owner)] | length' <<<"$(body "$r")")" = 0 ] || fail "3: bob is told owners"
pass "3: bob caught up, and sees his own devices only"

# 4. Refusal: bob is a stranger to tenant other's vault, and to an unknown id.
r=$(api bob GET "/vaults/$elsewhere/sync"); [ "$(status "$r")" = 404 ] || fail "4: elsewhere answered $r"
r=$(api bob GET "/vaults/00000000-0000-4000-8000-000000000000/sync"); [ "$(status "$r")" = 404 ] || fail "4: unknown answered $r"
r=$(api bob PUT "/vaults/$elsewhere/sync/report" '{"pending":0,"conflicts":0,"unsyncable":0,"failed":0}')
[ "$(status "$r")" = 404 ] || fail "4: a report on elsewhere answered $r"
[ "$(sql "SELECT count(*) FROM device_sync WHERE vault_id = '$elsewhere'")" = 0 ] || fail "4: a row exists on elsewhere"
pass "4: a stranger gets 404, the same as an unknown vault, and writes nothing"

# 5. Refusal: a scoped token cannot read the list, nor report.
out=$(as alice token create notes --name "heavy-sync-$$" --read "heavy-sync-$$/" --expires 1d --cidr 127.0.0.0/8)
key=$(awk 'NF==1 && /^ *kura_/{print $1}' <<<"$out")
[ -n "$key" ] || fail "5: no scoped token issued"
code=$(curl -s -o /dev/null -w '%{http_code}' "$server/v1/vaults/$notes/sync" -H "authorization: Bearer $key")
[ "$code" = 403 ] || fail "5: a scoped token read the list: $code"
code=$(curl -s -o /dev/null -w '%{http_code}' -X PUT "$server/v1/vaults/$notes/sync/report" -H "authorization: Bearer $key" -H 'content-type: application/json' -d '{"pending":0,"conflicts":0,"unsyncable":0,"failed":0}')
[ "$code" = 403 ] || fail "5: a scoped token reported: $code"
pass "5: a scoped token is refused on both routes (403)"

# 6. Paths are not counts: an extra field is refused.
r=$(api bob PUT "/vaults/$notes/sync/report" '{"pending":0,"conflicts":0,"unsyncable":0,"failed":0,"paths":["secret.md"]}')
[ "$(status "$r")" = 400 ] || fail "6: a report with paths answered $r"
pass "6: a report carrying paths is refused (400)"

# 7. The console in Chromium: alice's vault page shows bob's device by name,
# up to date, with its last run (ui/e2e/devices.spec.ts).
[ -f "$root/ui/dist/index.html" ] || fail "7: no console build: task ui:build, then restart task dev:run"
ak=${KURA_DEV_AUTHENTIK:-https://localhost:59443}
(cd "$root/ui" && KURA_E2E_NOTES=$notes KURA_E2E_DEVICE=$(device bob) KURA_E2E_OWNER=Bob KURA_E2E_AUTHENTIK=$ak \
  KURA_DEV_SERVER=$server pnpm exec playwright test e2e/devices.spec.ts --reporter=line) || fail "7: the console's browser proof failed"
pass "7: the vault page shows bob's device by name, up to date, with its last run"

