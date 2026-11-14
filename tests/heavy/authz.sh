#!/usr/bin/env bash
# RFC 0001 §6.P against the local stack (task dev:stack, then task dev:run),
# the authorization observations: 3 (a scoped token from outside its
# addresses), 4 (another tenant's vault looks missing), 5 (a revoked device
# is refused while another device of the same user goes on).
# KURA_DEV_LOG: the dev server's log, to check observation 3's audit event.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
kura="$root/target/debug/kura"
server=${KURA_DEV_SERVER:-http://127.0.0.1:58080}
issuer=https://localhost:59443/application/o/kura/
work=$(mktemp -d)
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }
sql() { podman exec kura-dev-postgres psql -U postgres -d kura -tAc "$1"; }
declare -A tokens
as() { # as <home> <kura args…>: kura with that home and its token
  local who=$1; shift
  KURA_CLI_HOME="$work/$who" KURA_ACCESS_TOKEN="${tokens[$who]}" "$kura" "$@"
}
cleanup() {
  sql "UPDATE device SET revoked_at = now() WHERE revoked_at IS NULL AND name LIKE 'authz-%-$$'" >/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

curl -sf "$server/readyz" >/dev/null || fail "no server on $server: task dev:run"
cargo build -q -p kura-cli
# Separate sessions: revoking one device ends its session (decision 35).
for who in alice bob1 bob2; do
  user=${who%[0-9]}
  tokens[$who]=$("$root/dev/token.sh" "$user")
  as "$who" login --server "$server" --issuer "$issuer" --device-name "authz-$who-$$" >/dev/null
done
device() { jq -r .device "$work/$1/config.json"; }
vault() { sql "SELECT id FROM vault WHERE name = '$1'"; }
notes=$(vault notes)

# 3. A scoped token used from outside its addresses: 403, an audit event,
# last_used_at unchanged; from inside, it reads.
for range in 10.99.0.0/16 127.0.0.0/8; do
  out=$(as alice token create notes --expires 1d --cidr "$range")
  key=$(awk 'NF==1 && /^ *kura_/{print $1}' <<<"$out")
  [ -n "$key" ] || fail "3: no token printed: $out"
  id=$(sed -E 's/^kura_([0-9a-f]{8})([0-9a-f]{4})([0-9a-f]{4})([0-9a-f]{4})([0-9a-f]{12})_.*/\1-\2-\3-\4-\5/' <<<"$key")
  status=$(curl -s -o "$work/body" -w '%{http_code}' -H "authorization: Bearer $key" "$server/v1/vaults/$notes/changes?since=0")
  if [ "$range" = 10.99.0.0/16 ]; then
    if [ "$status" != 403 ] || ! jq -e '.type == "/v1/problems/address_not_allowed"' "$work/body" >/dev/null; then
      fail "3: $status $(cat "$work/body")"
    fi
    [ -z "$(sql "SELECT last_used_at FROM scoped_token WHERE id = '$id'")" ] || fail "3: last_used_at moved"
    if [ -n "${KURA_DEV_LOG:-}" ]; then
      grep -q '"problem":"address_not_allowed"' "$KURA_DEV_LOG" || fail "3: no audit event in $KURA_DEV_LOG"
    fi
  else
    [ "$status" = 200 ] || fail "3: from inside its range: $status $(cat "$work/body")"
    [ -n "$(sql "SELECT last_used_at FROM scoped_token WHERE id = '$id'")" ] || fail "3: last_used_at not set"
  fi
  as alice token revoke notes "$id" >/dev/null
done
pass "3: scoped token refused outside its CIDR (403 address_not_allowed), last_used_at unchanged"

# 4. Another tenant's vault and a vault that does not exist: the same 404, byte for byte.
call() { curl -s -D - -o "$work/$1" -H "authorization: Bearer ${tokens[alice]}" -H "x-kura-device: $(device alice)" \
  "$server/v1/vaults/$2/changes?since=0" | grep -iv '^date:' | tr -d '\r' >"$work/$1.headers"; }
call foreign "$(vault elsewhere)"
call missing "$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid)"
cmp -s "$work/foreign" "$work/missing" || fail "4: bodies differ: $(cat "$work/foreign") / $(cat "$work/missing")"
cmp -s "$work/foreign.headers" "$work/missing.headers" || fail "4: headers differ"
grep -q '^HTTP/1.1 404' "$work/foreign.headers" || fail "4: $(head -1 "$work/foreign.headers")"
pass "4: another tenant's vault answers 404, byte-identical to a missing one"

# 5. bob revokes his first device from his second: the first is refused, the second goes on.
as bob2 devices revoke "authz-bob1-$$" >/dev/null
set +e
as bob1 devices >/dev/null 2>"$work/err"
code=$?
set -e
if [ "$code" != 2 ] || ! grep -q device_revoked "$work/err"; then
  fail "5: exit $code: $(cat "$work/err")"
fi
as bob2 devices >/dev/null || fail "5: the other device was refused"
pass "5: the revoked device gets 401 device_revoked, the other device of the user succeeds"
