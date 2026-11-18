#!/usr/bin/env bash
# RFC 0004 §6.P against the local stack (task dev:stack, then task dev:run):
# a bot holding a scoped token with folder rules, through the real `kura`
# binary, and `curl` for the requests the CLI refuses to send. Each run
# creates its own vault `keys-<pid>` through the API (deleted at the end), so
# it does not depend on what earlier runs stored.
# Observation 6 rewrites dev/config/tenants.yaml and puts it back.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
kura="$root/target/debug/kura"
server=${KURA_DEV_SERVER:-http://127.0.0.1:58080}
metrics=${KURA_DEV_METRICS:-http://127.0.0.1:59090}
issuer=https://localhost:59443/application/o/kura/
config="$root/dev/config/tenants.yaml"
work=$(mktemp -d)
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }
sql() { podman exec kura-dev-postgres psql -U postgres -d kura -tAc "$1"; }
cleanup() {
  [ -f "$work/tenants.yaml" ] && cp "$work/tenants.yaml" "$config"
  sql "UPDATE scoped_token SET revoked_at = now() WHERE revoked_at IS NULL AND name = 'recap-$$'" >/dev/null || true
  if [ -n "${vault_id:-}" ]; then
    curl -s -o /dev/null -X DELETE -H "authorization: Bearer $gm_token" -H "x-kura-device: $(jq -r .device "$work/gm-home/config.json")" \
      "$server/v1/vaults/$vault_id" || true
  fi
  sql "UPDATE device SET revoked_at = now() WHERE revoked_at IS NULL AND name = 'keys-gm-$$'" >/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

curl -sf "$server/readyz" >/dev/null || fail "no server on $server: task dev:run"
grep -q 'scopedTokens: { write: true }' "$config" || fail "dev/config/tenants.yaml: scopedTokens.write is not on"
cargo build -q -p kura-cli
gm() { KURA_CLI_HOME="$work/gm-home" KURA_ACCESS_TOKEN="$gm_token" "$kura" "$@"; }
gm_token=$("$root/dev/token.sh" alice)
gm login --server "$server" --issuer "$issuer" --device-name "keys-gm-$$" >/dev/null
# A vault of its own: alice owns the dev tenant.
vault=keys-$$
vault_id=$(curl -sf -H "authorization: Bearer $gm_token" -H "x-kura-device: $(jq -r .device "$work/gm-home/config.json")" \
  -H 'content-type: application/json' -d "{\"tenant\":\"dev\",\"name\":\"$vault\"}" "$server/v1/vaults" | jq -r .id)
[ -n "$vault_id" ] && [ "$vault_id" != null ] || fail "could not create the vault $vault"
notes=$vault_id
S="Sessions-$$"; P="PNJ-$$"; M="MJ-$$"; R="Resumes-$$"

# The campaign, pushed by the game master.
mkdir -p "$work/gm/$S" "$work/gm/$P" "$work/gm/$M"
printf 'session one %s\n' "$$" > "$work/gm/$S/1.md"
printf 'an elf %s\n' "$$" > "$work/gm/$P/elf.md"
printf 'the villain %s\n' "$$" > "$work/gm/$M/secret.md"
gm attach "$vault" "$work/gm" --yes >/dev/null

out=$(gm token create "$vault" --name "recap-$$" --read "$S/" --read "$P/" --write "$R/" --expires 1d --cidr 127.0.0.0/8)
key=$(awk 'NF==1 && /^ *kura_/{print $1}' <<<"$out")
[ -n "$key" ] || fail "no token printed: $out"
id=$(sed -E 's/^kura_([0-9a-f]{8})([0-9a-f]{4})([0-9a-f]{4})([0-9a-f]{4})([0-9a-f]{12})_.*/\1-\2-\3-\4-\5/' <<<"$key")
grep -q "write  $vault/$R/" <<<"$out" || fail "the write rule is not shown: $out"
# The bot: a token and the server's address, no login.
bot() { KURA_CLI_HOME="$work/bot-home" KURA_SERVER="$server" KURA_TOKEN="$key" "$kura" "$@"; }
key_curl() { curl -s -o "$work/body" -w '%{http_code}' -H "authorization: Bearer $key" -H 'content-type: application/json' "$@"; }
head_seq() { sql "SELECT head_seq FROM vault WHERE id = '$notes'"; }
refused() { curl -sf "$metrics/metrics" | awk '/^kura_token_writes_total\{outcome="refused"\}/{print $2}' | grep . || echo 0; }

# 4. An empty folder attached with the token gets the readable folders only.
bot attach "$vault" "$work/bot" --yes >/dev/null
[ -f "$work/bot/$S/1.md" ] && [ -f "$work/bot/$P/elf.md" ] || fail "4: readable folders not downloaded"
[ -z "$(find "$work/bot" -path "*$M*")" ] || fail "4: $M was downloaded"
[ "$(find "$work/bot" -type f -not -path '*/.kura/*' -not -name '.*' | wc -l)" = 2 ] || fail "4: more than the two readable files: $(find "$work/bot" -type f)"
pass "4: a token's attach downloads its readable folders only"

# 1. A recap under the write folder: pushed, pulled by the game master,
# attributed to the key.
mkdir -p "$work/bot/$R"
printf 'recap one %s\n' "$$" > "$work/bot/$R/1.md"
bot sync >/dev/null || fail "1: the bot's push was refused"
gm sync "$vault" >/dev/null
[ "$(cat "$work/gm/$R/1.md" 2>/dev/null)" = "recap one $$" ] || fail "1: the game master did not get the recap"
by=$(sql "SELECT user_sub FROM revision WHERE vault_id = '$notes' AND path = '$R/1.md' ORDER BY seq DESC LIMIT 1")
[ "$by" = "key:$id" ] || fail "1: revision attributed to $by"
pass "1: the bot's recap is pushed, pulled and attributed to key:$id"

# 2. curl commits a put outside the write folders: 403, nothing applied.
blob=$(sql "SELECT encode(blob_hash, 'hex') FROM revision WHERE vault_id = '$notes' AND path = '$R/1.md' ORDER BY seq DESC LIMIT 1")
size=$(wc -c < "$work/bot/$R/1.md")
entry=$(python3 -I -c 'import os, time; t = int(time.time() * 1000); b = bytearray(t.to_bytes(6, "big") + os.urandom(10)); b[6] = 0x70 | (b[6] & 0x0f); b[8] = 0x80 | (b[8] & 0x3f); h = b.hex(); print("-".join([h[:8], h[8:12], h[12:16], h[16:20], h[20:]]))')
before=$(head_seq)
status=$(key_curl -d "{\"idempotencyKey\":\"keys-$$\",\"operations\":[{\"op\":\"put\",\"entry\":\"$entry\",\"path\":\"$M/x.md\",\"blob\":\"$blob\",\"size\":$size}]}" "$server/v1/vaults/$notes/commits")
if [ "$status" != 403 ] || ! jq -e '.type == "/v1/problems/insufficient_role" and .detail == "operations[0]: outside the token'"'"'s write folders"' "$work/body" >/dev/null; then
  fail "2: $status $(cat "$work/body")"
fi
[ "$(head_seq)" = "$before" ] || fail "2: head_seq moved"
[ -z "$(sql "SELECT 1 FROM revision WHERE vault_id = '$notes' AND path = '$M/x.md'")" ] || fail "2: a revision names $M/x.md"
pass "2: a commit outside the write folders is 403 insufficient_role, nothing applied"

# 3. A manifest naming the chunk of a note outside the token's folders is
# refused exactly as one naming a chunk stored nowhere; the chunk is 404.
secret_blob=$(sql "SELECT encode(blob_hash, 'hex') FROM revision WHERE vault_id = '$notes' AND path = '$M/secret.md' ORDER BY seq DESC LIMIT 1")
secret_chunk=$(sql "SELECT encode(chunk_hash, 'hex') FROM blob_chunk WHERE vault_id = '$notes' AND blob_hash = decode('$secret_blob', 'hex') AND position = 0")
secret_size=$(wc -c < "$work/gm/$M/secret.md")
[ -n "$secret_chunk" ] || fail "3: no chunk for $M/secret.md"
# The manifest of the secret note is its real one; the other names a chunk
# nobody stored, under an address that is not its own: both must stop at
# possession, before the address is even compared.
random=$(printf 'nowhere %s' "$$" | sha256sum | cut -c1-64)
status=$(key_curl -X PUT -d "{\"size\":$secret_size,\"chunks\":[\"$secret_chunk\"]}" "$server/v1/vaults/$notes/blobs/$secret_blob")
jq -c '{type, detail}' "$work/body" > "$work/outside"
[ "$status" = 422 ] || fail "3: $status $(cat "$work/body")"
grep -q 'content not uploaded by this token' "$work/outside" || fail "3: $(cat "$work/outside")"
status=$(key_curl "$server/v1/vaults/$notes/chunks/$secret_chunk")
[ "$status" = 404 ] || fail "3: the outside chunk is served: $status"
status=$(key_curl -d "{\"hashes\":[\"$secret_chunk\",\"$random\"]}" "$server/v1/vaults/$notes/chunks/missing")
[ "$status" = 200 ] && [ "$(jq '.missing | length' "$work/body")" = 2 ] || fail "3: chunks/missing tells the two apart: $(cat "$work/body")"
pass "3: content outside the token's folders cannot be named or probed (422, 404, both missing)"

# 5. A file moved out of the write folder: the CLI refuses before sending.
before=$(head_seq); counted=$(refused)
mkdir -p "$work/bot/$M"
mv "$work/bot/$R/1.md" "$work/bot/$M/1.md"
set +e; bot sync > "$work/out" 2>&1; code=$?; set -e
[ "$code" = 3 ] || fail "5: exit $code: $(cat "$work/out")"
grep -q "is outside this token's write folders ($R/)" "$work/out" || fail "5: $(cat "$work/out")"
grep -q 'Nothing was pushed' "$work/out" || fail "5: $(cat "$work/out")"
[ "$(head_seq)" = "$before" ] && [ "$(refused)" = "$counted" ] || fail "5: the server saw the refused change"
mv "$work/bot/$M/1.md" "$work/bot/$R/1.md"; rmdir "$work/bot/$M"
pass "5: a change outside the write folders is refused by the CLI, exit 3, nothing sent"

# 6. scopedTokens.write off: the token's writes stop, its reads go on.
writes() { key_curl -d '{"hashes":[]}' "$server/v1/vaults/$notes/chunks/missing"; }
wait_for() { for _ in $(seq 45); do [ "$(writes)" = "$1" ] && return 0; sleep 1; done; return 1; }
cp "$config" "$work/tenants.yaml"
sed -i 's/scopedTokens: { write: true }/scopedTokens: { write: false }/' "$config"
wait_for 403 || fail "6: the token still writes 45 s after the switch went off"
printf 'recap two %s\n' "$$" > "$work/bot/$R/2.md"
set +e; bot sync > "$work/out" 2>&1; code=$?; set -e
[ "$code" = 3 ] || fail "6: exit $code with the switch off: $(cat "$work/out")"
[ "$(key_curl "$server/v1/vaults/$notes/changes?since=0")" = 200 ] || fail "6: the token no longer reads"
cp "$work/tenants.yaml" "$config"; rm "$work/tenants.yaml"
wait_for 200 || fail "6: the token does not write again 45 s after the switch came back"
bot sync >/dev/null || fail "6: the push kept by the refusal did not go through"
pass "6: with scopedTokens.write off the token's push exits 3 and its reads go on; on again, the kept push lands"
echo "keys.sh: RFC 0004 §6.P holds on $server"
