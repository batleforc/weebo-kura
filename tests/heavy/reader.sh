#!/usr/bin/env bash
# RFC 0005 §6.P against the local stack (task dev:stack, task ui:build, then
# task dev:run): in the tenant `jdr` (search on), bob (jdr-players) is a
# viewer of `campaign` restricted to Sessions/ and Resumes/, alice (jdr-mj)
# owns it (dev/config/tenants.yaml). Clients: curl, the kura binary and, for
# the pages, Chromium driven by Playwright (ui/e2e/reader.spec.ts).
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
cleanup() {
  sql "UPDATE device SET revoked_at = now() WHERE revoked_at IS NULL AND name LIKE 'reader-%-$$'" >/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

curl -sf "$server/readyz" >/dev/null || fail "no server on $server: task dev:run"
curl -sf "$server/app/frame-csp.txt" >/dev/null || fail "no console on $server/app: task ui:build"
cargo build -q -p kura-cli
declare -A tokens
as() { local who=$1; shift; KURA_CLI_HOME="$work/$who" KURA_ACCESS_TOKEN="${tokens[$who]}" "$kura" "$@"; }
for who in alice bob; do
  tokens[$who]=$("$root/dev/token.sh" "$who")
  as "$who" login --server "$server" --issuer "$issuer" --device-name "reader-$who-$$" >/dev/null
done
device() { jq -r .device "$work/$1/config.json"; }
api() { # api <who> <path> [curl args…]: status, body in $work/body
  local who=$1 path=$2; shift 2
  curl -s -o "$work/body" -w '%{http_code}' -H "authorization: Bearer ${tokens[$who]}" -H "x-kura-device: $(device "$who")" "$@" "$server/v1$path"
}
vault=$(sql "SELECT id FROM vault WHERE namespace = 'jdr' AND name = 'campaign' AND disabled_at IS NULL")
[ -n "$vault" ] || fail "no vault campaign: restart task dev:run after updating dev/config"

# The campaign, written by alice; a hostile note under Sessions/.
mkdir -p "$work/c/Sessions" "$work/c/Resumes" "$work/c/MJ"
cat > "$work/c/Sessions/s12-$$.md" <<'MD'
# Session 12

<img src=x onerror=fetch('/x')>

![remote](https://example.org/p.png)

See [[Session 11]] and [[MJ/secret]].
MD
printf 'recap twelve %s\n' "$$" > "$work/c/Resumes/s12-$$.md"
printf 'xyzzy the villain %s\n' "$$" > "$work/c/MJ/secret-$$.md"
as alice attach campaign "$work/c" --yes >/dev/null
entry() { sql "SELECT id FROM entry WHERE vault_id = '$vault' AND path = '$1' AND NOT deleted"; }
note=$(entry "Sessions/s12-$$.md"); hidden=$(entry "MJ/secret-$$.md")
[ -n "$note" ] && [ -n "$hidden" ] || fail "the campaign notes were not pushed"

# 2. bob's listing has nothing of MJ/; the hidden blob answers as a random one.
[ "$(api bob "/vaults/$vault/entries")" = 200 ] || fail "2: $(cat "$work/body")"
! jq -e '.items[] | select(.path | startswith("MJ/"))' "$work/body" >/dev/null || fail "2: MJ/ listed to bob"
blob=$(sql "SELECT encode(r.blob_hash, 'hex') FROM entry e JOIN revision r ON r.id = e.head_revision WHERE e.id = '$hidden'")
random=$(printf 'nobody %s' "$$" | sha256sum | cut -c1-64)
a=$(api bob "/vaults/$vault/blobs/$blob"); ba=$(cat "$work/body")
b=$(api bob "/vaults/$vault/blobs/$random"); bb=$(cat "$work/body")
[ "$a" = 404 ] && [ "$ba" = "$bb" ] || fail "2: hidden $a $ba, random $b $bb"
pass "2: bob lists no MJ/ entry, and its blob answers 404 exactly as a random hash"

# 3. Search: alice finds the hidden word once it is indexed; bob, never.
hits() { api "$1" "/vaults/$vault/search" -H 'content-type: application/json' -d '{"query":"xyzzy"}' >/dev/null; jq '.results | length' "$work/body"; }
for _ in $(seq 60); do [ "$(hits alice)" = 1 ] && break; sleep 1; done
[ "$(hits alice)" = 1 ] || fail "3: alice's search found $(cat "$work/body") after 60 s"
[ "$(api bob "/vaults/$vault/search" -H 'content-type: application/json' -d '{"query":"xyzzy"}')" = 200 ] || fail "3: $(cat "$work/body")"
[ "$(jq '.results | length' "$work/body")" = 0 ] || fail "3: bob found the hidden word: $(cat "$work/body")"
pass "3: alice finds xyzzy once indexed; bob's search answers 200 with no hit"

# 4. bob's kura sync of an empty folder brings the shared folders only.
as bob attach campaign "$work/bob-vault" --yes >/dev/null
[ -f "$work/bob-vault/Sessions/s12-$$.md" ] && [ -f "$work/bob-vault/Resumes/s12-$$.md" ] || fail "4: shared notes missing"
[ -z "$(find "$work/bob-vault" -path '*MJ*')" ] || fail "4: MJ/ reached bob's folder"
pass "4: bob's sync brings Sessions/ and Resumes/, nothing of MJ/"

# 7. Anonymous: refused.
[ "$(curl -s -o "$work/body" -w '%{http_code}' "$server/v1/vaults")" = 401 ] || fail "7: anonymous answered $(cat "$work/body")"
pass "7: an anonymous GET /v1/vaults is 401"

# 1, 6 and the page half of 5, in Chromium, as bob signed in to the console.
refresh=$(bash "$root/tests/heavy/console.sh" --refresh-token bob)
[ -n "$refresh" ] || fail "no kura-web refresh token for bob"
(cd "$root/ui" && KURA_E2E_REFRESH=$refresh KURA_E2E_VAULT=$vault KURA_E2E_NOTE=$note KURA_E2E_HIDDEN=$hidden \
  KURA_DEV_SERVER=$server pnpm exec playwright test e2e/reader.spec.ts --reporter=line) || fail "1, 6: the reader's browser proof failed"
pass "1, 6: in Chromium bob reads Sessions/ and Resumes/; the hostile note ran nothing and the frame cannot reach storage"

# 5. alice moves the note under MJ/: bob loses it and its history.
mv "$work/c/Sessions/s12-$$.md" "$work/c/MJ/s12-$$.md"
as alice sync campaign >/dev/null
[ "$(api bob "/vaults/$vault/entries/$note/revisions")" = 404 ] || fail "5: the moved note's history is still bob's"
pass "5: a note moved under MJ/ is gone for bob, history included"
echo "reader.sh: RFC 0005 §6.P holds on $server"
