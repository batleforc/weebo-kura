#!/usr/bin/env bash
# RFC 0005 §6.P against the local stack (task dev:stack, task ui:build, then
# task dev:run): in the tenant `jdr` (search on), bob (jdr-players) is a
# viewer of `campaign` restricted to Sessions/ and Resumes/, alice (jdr-mj)
# owns it (dev/config/tenants.yaml). Clients: curl, the kura binary and, for
# the pages, Chromium driven by Playwright (ui/e2e/reader.spec.ts).
# KURA_DEV_LOG (default target/dev-server.log): the server's output, with its
# object-storage reads logged, for observation 2:
#   RUST_LOG=info,kura::object_read=debug task dev:run > target/dev-server.log 2>&1
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
log=${KURA_DEV_LOG:-$root/target/dev-server.log}
moved_spec="$root/ui/e2e/reader-moved-$$.spec.ts"
cleanup() {
  rm -f "$moved_spec"
  sql "UPDATE device SET revoked_at = now() WHERE revoked_at IS NULL AND name LIKE 'reader-%-$$'" >/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

curl -sf "$server/readyz" >/dev/null || fail "no server on $server: task dev:run"
curl -sf "$server/app/frame-csp.txt" >/dev/null || fail "no console on $server/app: task ui:build"
[ -f "$log" ] || fail "no server log at $log: start the server with RUST_LOG=info,kura::object_read=debug task dev:run > $log 2>&1, or set KURA_DEV_LOG"
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
# The refused bytes were never fetched: bob asks for the hidden note's chunk,
# then a visible one; only the visible one reaches object storage. The
# visible read proves the log records reads, so the absence means something.
chunk_of() { sql "SELECT encode(bc.chunk_hash, 'hex') FROM entry e JOIN revision r ON r.id = e.head_revision JOIN blob_chunk bc ON bc.vault_id = e.vault_id AND bc.blob_hash = r.blob_hash WHERE e.id = '$1' ORDER BY bc.position LIMIT 1"; }
hidden_chunk=$(chunk_of "$hidden"); shown_chunk=$(chunk_of "$note")
from=$(($(wc -l < "$log") + 1))
[ "$(api bob "/vaults/$vault/chunks/$hidden_chunk")" = 404 ] || fail "2: the hidden chunk answered $(cat "$work/body")"
[ "$(api bob "/vaults/$vault/chunks/$shown_chunk")" = 200 ] || fail "2: the shared chunk answered $(cat "$work/body")"
for _ in $(seq 10); do tail -n +"$from" "$log" | grep -q "\"chunk\":\"$shown_chunk\"" && break; sleep 0.5; done
tail -n +"$from" "$log" | grep -q "\"chunk\":\"$shown_chunk\"" || fail "2: $log records no object-storage read (RUST_LOG=info,kura::object_read=debug?)"
! tail -n +"$from" "$log" | grep -q "\"chunk\":\"$hidden_chunk\"" || fail "2: the hidden chunk was read from object storage"
pass "2: bob lists no MJ/ entry, its blob answers 404 exactly as a random hash, and its chunk is never read from object storage"

# 3. Search: alice finds the hidden word once it is indexed; bob, never.
# Earlier runs leave their own secret notes in the vault: count this run's.
hits() { api "$1" "/vaults/$vault/search" -H 'content-type: application/json' -d '{"query":"xyzzy"}' >/dev/null; jq --arg p "MJ/secret-$$.md" '[.results[] | select(.path == $p)] | length' "$work/body"; }
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
# The page half after the move, at the note's address under MJ/ (the same
# entry if the move kept it) and at its old one, in a fresh browser session.
moved=$(entry "MJ/s12-$$.md")
[ -n "$moved" ] || fail "5: the move did not reach the server"
cat > "$moved_spec" <<'TS'
import { expect, test } from "@playwright/test";

import { env, signOut } from "./helpers";

test("a note moved out of the shared folders says it does not exist", async ({ page }) => {
  await page.addInitScript((token) => {
    if (!sessionStorage.getItem("kura:refresh-token"))
      sessionStorage.setItem("kura:refresh-token", token);
  }, env("KURA_E2E_REFRESH"));
  for (const entry of [env("KURA_E2E_MOVED"), env("KURA_E2E_NOTE")]) {
    await page.goto(`/app/vaults/${env("KURA_E2E_VAULT")}/notes/${entry}`);
    await expect(page.locator("[data-test=not-found]")).toContainText(
      "does not exist, or is not shared with you",
    );
  }
  await signOut(page);
});
TS
refresh=$(bash "$root/tests/heavy/console.sh" --refresh-token bob)
[ -n "$refresh" ] || fail "5: no kura-web refresh token for bob"
(cd "$root/ui" && KURA_E2E_REFRESH=$refresh KURA_E2E_VAULT=$vault KURA_E2E_NOTE=$note KURA_E2E_MOVED=$moved \
  KURA_DEV_SERVER=$server pnpm exec playwright test "e2e/reader-moved-$$.spec.ts" --reporter=line) || fail "5: the moved note's page is not the not-found page"
pass "5: a note moved under MJ/ is gone for bob, history included, and its page says it does not exist"
echo "reader.sh: RFC 0005 §6.P holds on $server"
