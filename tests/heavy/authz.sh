#!/usr/bin/env bash
# RFC 0001 §6.P against the local stack (task dev:stack, then task dev:run),
# the authorization observations: 3 (a scoped token from outside its
# addresses), 4 (another tenant's vault looks missing), 5 (a revoked device
# is refused while another device of the same user goes on). RFC 0002
# observations 2 (a prefix token's chunk batch) and 3 (another member's
# configuration, unseen in a batch).
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

# RFC 0002 observation 2: a token under agents/ asks chunks/batch for a chunk
# inside and one outside: one found, one missing, one object fetched.
mkdir -p "$work/notes/agents" "$work/notes/private"
printf 'inside %s\n' "$$" > "$work/notes/agents/in-$$.md"
printf 'outside %s\n' "$$" > "$work/notes/private/out-$$.md"
as alice attach notes "$work/notes" --yes >/dev/null
out=$(as alice token create notes --prefix agents/ --expires 1d --cidr 127.0.0.0/8)
key=$(awk 'NF==1 && /^ *kura_/{print $1}' <<<"$out")
dev_curl() { curl -sf -H "authorization: Bearer ${tokens[alice]}" -H "x-kura-device: $(device alice)" "$@"; }
chunk_of() { # the first chunk of the live entry at this path
  local blob
  blob=$(sql "SELECT encode(r.blob_hash, 'hex') FROM entry e JOIN revision r ON r.id = e.head_revision WHERE e.vault_id = '$notes' AND e.path = '$1' AND NOT e.deleted")
  dev_curl "$server/v1/vaults/$notes/blobs/$blob" | jq -r '.chunks[0]'
}
in_chunk=$(chunk_of "agents/in-$$.md"); out_chunk=$(chunk_of "private/out-$$.md")
found() { curl -sf "${KURA_DEV_METRICS:-http://127.0.0.1:59090}/metrics" | awk '/^kura_batch_items_total\{outcome="found",route="chunks"\}/{print $2}'; }
before=$(found); before=${before:-0}
curl -sf -o "$work/records" -H "authorization: Bearer $key" -H 'content-type: application/json' \
  -d "{\"chunks\":[\"$in_chunk\",\"$out_chunk\"]}" "$server/v1/vaults/$notes/chunks/batch" || fail "rfc0002-2: the batch was refused"
statuses=$(python3 -I -c '
import sys
b = open(sys.argv[1], "rb").read()
out = []
while b:
    n = int.from_bytes(b[33:37], "big")
    out.append(b[:32].hex() + ":" + str(b[32]))
    b = b[37 + n:]
print(" ".join(out))' "$work/records")
[ "$statuses" = "$in_chunk:0 $out_chunk:1" ] || fail "rfc0002-2: records $statuses"
after=$(found)
[ "$(( ${after%.*} - ${before%.*} ))" = 1 ] || fail "rfc0002-2: $before -> $after objects fetched"
as alice token revoke notes "$(sed -E 's/^kura_([0-9a-f]{8})([0-9a-f]{4})([0-9a-f]{4})([0-9a-f]{4})([0-9a-f]{12})_.*/\1-\2-\3-\4-\5/' <<<"$key")" >/dev/null
pass "rfc0002-2: a prefix token's batch serves the chunk inside, answers missing outside, fetches one object"

# RFC 0002 observation 3: bob asks chunks/batch for a chunk of alice's own
# .obsidian (vault `obsidian`, configSync): the record is byte for byte the
# one a random hash gets; alice herself gets it.
mkdir -p "$work/obs/.obsidian/snippets"
printf '.alice-%s { color: red; }\n' "$$" > "$work/obs/.obsidian/snippets/alice-$$.css"
as alice attach obsidian "$work/obs" --yes >/dev/null
obsv=$(vault obsidian)
cfg_blob=$(sql "SELECT encode(r.blob_hash, 'hex') FROM entry e JOIN revision r ON r.id = e.head_revision WHERE e.vault_id = '$obsv' AND e.namespace = 'config:desktop' AND e.path = 'snippets/alice-$$.css' AND NOT e.deleted")
[ -n "$cfg_blob" ] || fail "rfc0002-3: alice's snippet was not pushed to her configuration"
cfg_chunk=$(sql "SELECT encode(chunk_hash, 'hex') FROM blob_chunk WHERE vault_id = '$obsv' AND blob_hash = decode('$cfg_blob', 'hex') AND position = 0")
random=$(printf 'nobody %s' "$$" | sha256sum | cut -c1-64)
batch_as() { # batch_as <who> <hash>…: the record headers (status and length) of a chunks/batch answer
  local who=$1; shift
  local list; list=$(printf '"%s",' "$@"); list="[${list%,}]"
  curl -sf -o "$work/rec-$who" -H "authorization: Bearer ${tokens[$who]}" -H "x-kura-device: $(device "$who")" \
    -H 'content-type: application/json' -d "{\"chunks\":$list}" "$server/v1/vaults/$obsv/chunks/batch" || fail "rfc0002-3: $who's batch refused"
  python3 -I -c '
import sys
b = open(sys.argv[1], "rb").read()
out = []
while b:
    n = int.from_bytes(b[33:37], "big")
    out.append(str(b[32]) + "/" + str(n))
    b = b[37 + n:]
print(" ".join(out))' "$work/rec-$who"
}
theirs=$(batch_as bob1 "$cfg_chunk" "$random")
[ "$theirs" = "1/0 1/0" ] || fail "rfc0002-3: bob got $theirs for alice's chunk and a random one"
mine=$(batch_as alice "$cfg_chunk")
[ "${mine%%/*}" = 0 ] || fail "rfc0002-3: alice did not get her own chunk: $mine"
# Alice's real Obsidian syncs this configuration too: the snippet goes again.
rm "$work/obs/.obsidian/snippets/alice-$$.css"
as alice sync obsidian >/dev/null
left=$(sql "SELECT count(*) FROM entry WHERE vault_id = '$obsv' AND path = 'snippets/alice-$$.css' AND NOT deleted")
[ "$left" = 0 ] || fail "rfc0002-3: the test snippet is still in alice's configuration"
pass "rfc0002-3: another member's batch gets alice's configuration chunk as an unknown hash; alice gets it"

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
