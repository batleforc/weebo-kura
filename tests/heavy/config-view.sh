#!/usr/bin/env bash
# RFC 0008 §6.P: the console's view of a member's own Obsidian configuration,
# against the local stack (task dev:stack, task ui:build, task dev:run), on
# the vault `suite-config` — never `obsidian`, which holds real notes.
# The API observations run with curl; the page's with Playwright in Chromium
# (ui/e2e/config.spec.ts). `--api-only` skips the browser (a server on
# another port, which Authentik does not redirect to).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
kura="$root/target/debug/kura"
server=${KURA_DEV_SERVER:-http://127.0.0.1:58080}
ak=${KURA_DEV_AUTHENTIK:-https://localhost:59443}
issuer=$ak/application/o/kura/
work=$(mktemp -d)
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok: $*"; }
sql() { podman exec kura-dev-postgres psql -U postgres -d kura -tAc "$1"; }
declare -A tokens devices
as() { # as <who> <kura args…>
  local who=$1; shift
  KURA_CLI_HOME="$work/$who" KURA_ACCESS_TOKEN="${tokens[$who]}" "$kura" "$@"
}
cleanup() {
  # The test files leave alice's configuration, then every device made here.
  if [ -d "$work/obs/.obsidian" ]; then
    rm -rf "$work/obs/.obsidian/snippets/s-$$.css" "$work/obs/.obsidian/plugins/p-$$" "$work/obs/n-$$.md"
    as alice sync suite-config >/dev/null 2>&1 || true
  fi
  sql "UPDATE device SET revoked_at = now() WHERE revoked_at IS NULL AND name LIKE 'config-view-%-$$'" >/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

curl -sf "$server/readyz" >/dev/null || fail "no server on $server: task dev:run"
cargo build -q -p kura-cli
for who in alice bob; do
  tokens[$who]=$("$root/dev/token.sh" "$who")
done
# alice's CLI (desktop) and her console's device, a web device of the API.
as alice login --server "$server" --issuer "$issuer" --device-name "config-view-cli-$$" >/dev/null
web_device() { # web_device <who>: registers a web device, prints its id
  curl -sf -H "authorization: Bearer ${tokens[$1]}" -H 'content-type: application/json' \
    -d "{\"name\":\"config-view-web-$1-$$\",\"profile\":\"web\"}" "$server/v1/devices" | jq -r .id
}
devices[alice]=$(web_device alice)
devices[bob]=$(web_device bob)
v=$(sql "SELECT id FROM vault WHERE name = 'suite-config' AND disabled_at IS NULL")
[ -n "$v" ] || fail "no vault suite-config: restart task dev:run after updating dev/config"
api() { # api <who> <method> <path> [body]: status, body in $work/body
  local who=$1 method=$2 path=$3
  curl -s -o "$work/body" -w '%{http_code}' -X "$method" -H "authorization: Bearer ${tokens[$who]}" \
    -H "x-kura-device: ${devices[$who]}" -H 'content-type: application/json' \
    ${4:+-d "$4"} "$server/v1/vaults/$v$path"
}

# 1. alice's CLI syncs a snippet, a plugin's manifest and its settings.
mkdir -p "$work/obs/.obsidian/snippets" "$work/obs/.obsidian/plugins/p-$$"
printf '.probe-%s { color: red; }\n' "$$" > "$work/obs/.obsidian/snippets/s-$$.css"
printf '{"id":"p-%s","name":"Probe %s","version":"1.0.0"}\n' "$$" "$$" > "$work/obs/.obsidian/plugins/p-$$/manifest.json"
secret="{\"token\":\"s3cret-$$-$RANDOM\"}"
printf '%s' "$secret" > "$work/obs/.obsidian/plugins/p-$$/data.json"
as alice attach suite-config "$work/obs" --yes >/dev/null
hash_of() { sql "SELECT encode(r.blob_hash, 'hex') FROM entry e JOIN revision r ON r.id = e.head_revision WHERE e.vault_id = '$v' AND e.namespace = 'config:desktop' AND e.path = '$1' AND NOT e.deleted"; }
entry_of() { sql "SELECT id FROM entry WHERE vault_id = '$v' AND namespace = 'config:desktop' AND path = '$1' AND NOT deleted"; }
data_blob=$(hash_of "plugins/p-$$/data.json"); snippet_blob=$(hash_of "snippets/s-$$.css")
[ -n "$data_blob" ] && [ -n "$snippet_blob" ] || fail "1: alice's configuration was not pushed"
[ "$(api alice GET '/entries?space=config&profile=desktop')" = 200 ] || fail "1: listing refused: $(cat "$work/body")"
jq -e --arg p "snippets/s-$$.css" --arg b "$snippet_blob" '.items[] | select(.path == $p and .blob == $b)' "$work/body" >/dev/null \
  || fail "1: the snippet is not listed with its hash"
pass "1: alice's console lists her desktop configuration"

# 2. The settings are listed without their hash, in the list and the history.
jq -e --arg p "plugins/p-$$/data.json" '.items[] | select(.path == $p and (has("blob") | not))' "$work/body" >/dev/null \
  || fail "2: data.json listed with its hash: $(cat "$work/body")"
[ "$(api alice GET "/entries/$(entry_of "plugins/p-$$/data.json")/revisions")" = 200 ] || fail "2: history refused"
! grep -q "$data_blob" "$work/body" || fail "2: the history gives the console the hash of data.json"
pass "2: the console is told no hash of the plugin's settings"

# 3. The content as an unknown hash to the console; the CLI's device reads it.
random=$(printf 'nobody %s' "$$" | sha256sum | cut -c1-64)
a=$(api alice GET "/blobs/$data_blob"); ba=$(cat "$work/body")
b=$(api alice GET "/blobs/$random"); bb=$(cat "$work/body")
[ "$a" = 404 ] && [ "$ba" = "$bb" ] || fail "3: data.json to the console: $a $ba, random: $b $bb"
cli=$(jq -r .device "$work/alice/config.json")
st=$(curl -s -o /dev/null -w '%{http_code}' -H "authorization: Bearer ${tokens[alice]}" -H "x-kura-device: $cli" "$server/v1/vaults/$v/blobs/$data_blob")
[ "$st" = 200 ] || fail "3: alice's CLI got $st for her own data.json"
[ "$(api alice GET "/blobs/$snippet_blob")" = 200 ] || fail "3: the snippet refused to the console"
pass "3: data.json answers the console as a random hash, byte for byte; the CLI gets it; the snippet is served"

# 4. No content write from the console: a note on the hash, a restore.
head_before=$(sql "SELECT head_seq FROM vault WHERE id = '$v'")
size=$(printf '%s' "$secret" | wc -c)
st=$(api alice POST /commits "{\"idempotencyKey\":\"config-view-$$\",\"operations\":[{\"op\":\"put\",\"entry\":\"$(python3 -I -c 'import uuid,time,os;t=int(time.time()*1000);print(uuid.UUID(int=(t<<80)|(0x7<<76)|(int.from_bytes(os.urandom(2),"big")&0xfff)<<64|(0b10<<62)|(int.from_bytes(os.urandom(8),"big")>>2)))')\",\"path\":\"leak-$$.md\",\"blob\":\"$data_blob\",\"size\":$size}]}")
[ "$st" = 403 ] || fail "4: a note on the hash from the console: $st $(cat "$work/body")"
snippet=$(entry_of "snippets/s-$$.css")
rev=$(sql "SELECT head_revision FROM entry WHERE id = '$snippet'")
st=$(api alice POST "/entries/$snippet/restore" "{\"revision\":\"$rev\"}")
[ "$st" = 403 ] || fail "4: a restore from the console: $st $(cat "$work/body")"
[ "$(sql "SELECT head_seq FROM vault WHERE id = '$v'")" = "$head_before" ] || fail "4: the vault moved"
pass "4: the console's note on the hash and its restore are refused; the vault did not move"

# 5. bob's console sees none of alice's configuration.
[ "$(api bob GET '/entries?space=config&profile=desktop')" = 200 ] || fail "5: bob's listing: $(cat "$work/body")"
! grep -q "s-$$" "$work/body" || fail "5: bob sees alice's snippet"
pass "5: another member's console lists none of it"

# 6. A scoped token, as today.
key=$(as alice token create suite-config --name "config-view-$$" --expires 1d --cidr 127.0.0.0/8 2>/dev/null | grep -oE 'kura_[0-9a-f]{32}_[A-Za-z0-9_-]+' | head -1 || true)
if [ -n "$key" ]; then
  st=$(curl -s -o /dev/null -w '%{http_code}' -H "authorization: Bearer $key" "$server/v1/vaults/$v/entries?space=config&profile=desktop")
  as alice token revoke suite-config "$(sed -E 's/^kura_([0-9a-f]{8})([0-9a-f]{4})([0-9a-f]{4})([0-9a-f]{4})([0-9a-f]{12})_.*/\1-\2-\3-\4-\5/' <<<"$key")" >/dev/null
  [ "$st" = 403 ] || fail "6: a scoped token got $st"
  pass "6: a scoped token is refused the configuration"
else
  echo "note: 6 not run: alice cannot create a key on suite-config here"
fi

if [ "${1:-}" != --api-only ]; then
  (cd "$root/ui" && KURA_E2E_CONFIG=$v KURA_E2E_SNIPPET="snippets/s-$$.css" KURA_E2E_PLUGIN="Probe $$" \
    KURA_E2E_DATA_BLOB=$data_blob KURA_E2E_AUTHENTIK=$ak KURA_DEV_SERVER=$server \
    pnpm exec playwright test e2e/config.spec.ts --reporter=line) || fail "the page's browser proof failed"
  pass "1, 2 in Chromium: the page lists the plugin by name, shows the snippet's text, asks nothing of data.json"
fi
echo "config-view.sh: RFC 0008 §6.P holds on $server"
