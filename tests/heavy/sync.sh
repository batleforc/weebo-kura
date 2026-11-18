#!/usr/bin/env bash
# RFC 0001 §6.P against the local stack (task dev:stack, then task dev:run):
# the real `kura` binary, Authentik, PostgreSQL, OpenBao and RustFS.
# Phase 3 observations: 2 (a viewer's push is refused and stores nothing),
# 7 (two offline edits of one paragraph: both versions kept as files),
# 11 (a dot-path note is refused by the server and reaches no device).
# Phase 4: 1 (a change reaches a watching device within 5 s). Phase 5: 6 (a
# device past the horizon resyncs, and a note deleted meanwhile is gone).
# Phase 9: `kura verify`, the migration check of §13.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
kura="$root/target/debug/kura"
server=${KURA_DEV_SERVER:-http://127.0.0.1:58080}
issuer=https://localhost:59443/application/o/kura/
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }
sql() { podman exec kura-dev-postgres psql -U postgres -d kura -tAc "$1"; }
as() { # as <who> <kura args…>: runs kura with that user's home and a fresh token
  local who=$1; shift
  KURA_CLI_HOME="$work/$who" KURA_ACCESS_TOKEN="${tokens[$who]}" "$kura" "$@"
}

curl -sf "$server/readyz" >/dev/null || fail "no server on $server: task dev:run"
cargo build -q -p kura-cli
declare -A tokens=([alice]=$("$root/dev/token.sh" alice) [bob]=$("$root/dev/token.sh" bob))
as alice login --server "$server" --issuer "$issuer" --device-name "heavy-a-$$" >/dev/null
as bob login --server "$server" --issuer "$issuer" --device-name "heavy-b-$$" >/dev/null
# The devices of this run are revoked at the end: 20 live devices per user (§4.4).
cleanup() {
  as alice devices revoke "heavy-a-$$" >/dev/null 2>&1 || true
  as bob devices revoke "heavy-b-$$" >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

# 7. Two devices edit the same paragraph offline.
mkdir -p "$work/a" "$work/b"
note="heavy-$$.md"
printf 'shared paragraph\n' > "$work/a/$note"
as alice attach notes "$work/a" --yes >/dev/null
as bob attach notes "$work/b" --yes >/dev/null
[ -f "$work/b/$note" ] || fail "7: bob did not receive $note"
printf 'shared paragraph, alice version\n' > "$work/a/$note"
printf 'shared paragraph, bob version\n' > "$work/b/$note"
as alice sync notes >/dev/null
out=$(as bob sync notes)
grep -q "^conflict: " <<<"$out" || fail "7: no conflict reported: $out"
as alice sync notes >/dev/null
for d in a b; do
  grep -rqs "alice version" "$work/$d" --include='*.md' || fail "7: $d lost alice's text"
  grep -rqs "bob version" "$work/$d" --include='*.md' || fail "7: $d lost bob's text"
done
pass "7: conflicted, both versions on both devices"
as alice verify notes >"$work/verify.log" || fail "§13: the server and alice's folder differ: $(cat "$work/verify.log")"
pass "§13: kura verify finds the folder equal to the server"

# RFC 0007 §6.P: folders as entries.
d="heavy-dirs-$$"
mkdir -p "$work/a/$d/Empty" "$work/a/$d/Kept"
out=$(as alice sync notes)
grep -q "folders created or removed" <<<"$out" || fail "rfc0007-1: no folder pushed: $out"
as bob sync notes >/dev/null
[ -d "$work/b/$d/Empty" ] && [ -d "$work/b/$d/Kept" ] || fail "rfc0007-1: bob did not get the empty folders"
pass "rfc0007-1: an empty folder made by alice reaches bob"
printf 'bob keeps this\n' > "$work/b/$d/Kept/mine.md"
rmdir "$work/a/$d/Empty" "$work/a/$d/Kept"
as alice sync notes >/dev/null
as bob sync notes >/dev/null
[ ! -e "$work/b/$d/Empty" ] || fail "rfc0007-2: the removed folder stayed on bob"
[ -f "$work/b/$d/Kept/mine.md" ] || fail "rfc0007-3: bob's note went with the folder"
as alice sync notes >/dev/null
[ -f "$work/a/$d/Kept/mine.md" ] || fail "rfc0007-3: bob's note did not reach alice"
pass "rfc0007-2/3: a removed folder leaves bob; one holding his new note stays, and the note syncs"
mkdir -p "$work/a/$d/Probe"
as alice sync notes >/dev/null
notes_id=$(sql "SELECT id FROM vault WHERE namespace = 'dev' AND name = 'notes' AND disabled_at IS NULL")
dev_b=$(jq -r .device "$work/bob/config.json")
old=$(curl -sf "$server/v1/vaults/$notes_id/changes?since=0" -H "authorization: Bearer ${tokens[bob]}" -H "x-kura-device: $dev_b")
new=$(curl -sf "$server/v1/vaults/$notes_id/changes?since=0&folders=true" -H "authorization: Bearer ${tokens[bob]}" -H "x-kura-device: $dev_b")
[ "$(jq '[.items[] | select(.folder == true)] | length' <<<"$old")" = 0 ] || fail "rfc0007-4: an older client was sent folders"
[ "$(jq --arg p "$d/Probe" '[.items[] | select(.folder == true and .path == $p)] | length' <<<"$new")" = 1 ] || fail "rfc0007-4: the folder is missing with folders=true"
pass "rfc0007-4: folders only to a client that asks (an older client sees none)"
key_out=$(as alice token create notes --name "heavy-dirs-$$" --read "$d/" --write "$d/w/" --expires 1d --cidr 127.0.0.0/8)
key=$(awk 'NF==1 && /^ *kura_/{print $1}' <<<"$key_out")
[ -n "$key" ] || fail "rfc0007-5: no token issued: $key_out"
uuid7() { python3 -I -c 'import os,time; t=int(time.time()*1000); b=bytearray(t.to_bytes(6,"big")+os.urandom(10)); b[6]=(b[6]&0x0f)|0x70; b[8]=(b[8]&0x3f)|0x80; h=b.hex(); print(f"{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}")'; }
commit() { curl -s -o /dev/null -w '%{http_code}' -X POST "$server/v1/vaults/$notes_id/commits" -H "authorization: Bearer $key" -H 'content-type: application/json' \
  -d "{\"idempotencyKey\":\"$(uuid7)\",\"operations\":[{\"op\":\"folder\",\"entry\":\"$(uuid7)\",\"path\":\"$1\"}]}"; }
[ "$(commit "$d/elsewhere")" = 403 ] || fail "rfc0007-5: a token made a folder outside its write folders"
[ "$(commit "$d/w/inside")" = 200 ] || fail "rfc0007-5: a token could not make a folder in its write folder"
sql "UPDATE scoped_token SET revoked_at = now() WHERE revoked_at IS NULL AND name = 'heavy-dirs-$$'" >/dev/null
pass "rfc0007-5: a token makes folders only where it writes notes (403 elsewhere)"

# 1. A change reaches a watching device within 5 s, through the socket.
as bob watch notes >"$work/watch.log" 2>&1 &
watcher=$!
sleep 2
printf 'pushed by alice %s\n' "$$" > "$work/a/live-$$.md"
start=$(date +%s%N)
as alice sync notes >/dev/null
arrived=""
for _ in $(seq 50); do
  [ -f "$work/b/live-$$.md" ] && { arrived=yes; break; }
  sleep 0.1
done
kill "$watcher" 2>/dev/null || true
wait "$watcher" 2>/dev/null || true
[ -n "$arrived" ] || fail "1: no change on the watching device within 5 s: $(cat "$work/watch.log")"
pass "1: change on the watching device after $(( ($(date +%s%N) - start) / 1000000 )) ms"

# 2. A viewer's push: exit 3, and no chunk stored.
mkdir -p "$work/s"
as bob attach shared "$work/s" --yes >/dev/null
before=$(sql "SELECT count(*) FROM chunk")
printf 'viewer text %s\n' "$$" > "$work/s/refused.md"
set +e
as bob sync shared >/dev/null 2>"$work/err"
code=$?
set -e
[ "$code" = 3 ] || fail "2: exit $code, expected 3: $(cat "$work/err")"
grep -q "insufficient_role" "$work/err" || fail "2: $(cat "$work/err")"
[ "$(sql "SELECT count(*) FROM chunk")" = "$before" ] || fail "2: a chunk was stored"
pass "2: viewer refused with exit 3, nothing stored"

# 11. A dot-path note: refused by the server, received by no one.
device=$(jq -r .device "$work/alice/config.json")
vault=$(jq -r '.vaults.notes.id' "$work/alice/config.json")
status=$(curl -s -o "$work/body" -w '%{http_code}' -X POST "$server/v1/vaults/$vault/commits" \
  -H "authorization: Bearer ${tokens[alice]}" -H "x-kura-device: $device" -H 'content-type: application/json' \
  -d '{"idempotencyKey":"heavy-11","operations":[{"op":"put","entry":"019a0000-0000-7000-8000-000000000000","path":".obsidian/plugins/x/main.js","blob":"'"$(printf '0%.0s' {1..64})"'","size":1}]}')
if [ "$status" != 422 ] || ! jq -e '.type == "/v1/problems/invalid_path"' "$work/body" >/dev/null; then
  fail "11: $status $(cat "$work/body")"
fi
mkdir -p "$work/a/.obsidian/plugins/x" && echo 'evil()' > "$work/a/.obsidian/plugins/x/main.js"
as alice sync notes >/dev/null
as bob sync notes >/dev/null
[ ! -e "$work/b/.obsidian/plugins/x/main.js" ] || fail "11: the plugin reached bob"
pass "11: dot-path refused by the server (422 invalid_path), not synced by the client"

# 6. Bob is away past deletedEntryTtl: the deletion is purged, his next sync
# gets 410, resyncs, and the note is gone on both devices (in his .trash).
away="away-$$.md"
printf 'deleted while bob is away\n' > "$work/a/$away"
as alice sync notes >/dev/null
as bob sync notes >/dev/null
[ -f "$work/b/$away" ] || fail "6: bob did not receive $away"
rm "$work/a/$away"
as alice sync notes >/dev/null
printf 'after the deletion\n' > "$work/a/after-$$.md"
as alice sync notes >/dev/null
sql "UPDATE revision SET created_at = now() - interval '100 days' WHERE entry_id IN (SELECT entry_id FROM revision WHERE path = '$away')" >/dev/null
(cd "$root" && KURA_VAULT_TOKEN=kura-dev SSL_CERT_FILE=dev/certs/ca.pem cargo run -q -p kura-server -- maintain --config dev/server.yaml) >"$work/maintain" 2>&1 \
  || fail "6: maintenance pass: $(cat "$work/maintain")"
[ "$(sql "SELECT count(*) FROM revision WHERE path = '$away'")" = 0 ] || fail "6: the deletion was not purged: $(cat "$work/maintain")"
out=$(as bob sync notes --yes)
grep -q "^resync: notes: this device was offline longer" <<<"$out" || fail "6: no resync: $out"
[ ! -e "$work/b/$away" ] || fail "6: $away is still on bob's device"
[ -f "$work/b/.trash/$away" ] || fail "6: $away is not in bob's .trash"
[ -f "$work/b/after-$$.md" ] || fail "6: bob missed a later note"
as alice sync notes >/dev/null
[ ! -e "$work/a/$away" ] || fail "6: $away came back to alice"
pass "6: 410 cursor_expired, full resync, the deleted note gone on both devices"

# D34 with Authentik's refresh: a real sign-in, commands on refreshed tokens
# (no `sid`, the session's `auth_time`), then revocation refuses them (exit 2).
home="$work/real"
SSL_CERT_FILE="$root/dev/certs/ca.pem" KURA_CLI_HOME="$home" "$kura" login --server "$server" --issuer "$issuer" --device-name "heavy-real" >"$work/login" 2>&1 &
login=$!
code=""
for _ in $(seq 20); do
  code=$(awk 'NF==1 && /^[[:space:]]+[0-9A-Z-]+$/{print $1}' "$work/login")
  [ -n "$code" ] && break
  sleep 1
done
[ -n "$code" ] || fail "D34: no device code shown: $(cat "$work/login")"
"$root/dev/approve.sh" "$code" bob
wait "$login" || fail "D34: login failed: $(cat "$work/login")"
real() { SSL_CERT_FILE="$root/dev/certs/ca.pem" KURA_CLI_HOME="$home" "$kura" "$@"; }
real devices >/dev/null || fail "D34: a refreshed token was refused"
real devices >/dev/null || fail "D34: a second refresh was refused"
real devices revoke heavy-real >/dev/null
set +e
real devices >/dev/null 2>&1
code=$?
set -e
[ "$code" = 2 ] || fail "D34: exit $code after revocation, expected 2"
pass "D34: real sign-in, refreshed tokens accepted, refused after revocation"
