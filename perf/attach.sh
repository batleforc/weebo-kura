#!/usr/bin/env bash
# Decision 42 (RFC 0001 §10.3): how long a new device takes to attach a
# 10,000-note vault, with the real `kura` CLI against the dev stack. Device A
# pushes the notes into a new vault, device B attaches it from nothing; above
# 5 minutes for B, the batch chunk download RFC opens. The vault is deleted
# afterwards (purged after its 30 days) and the devices revoked.
# usage: perf/attach.sh [notes]   (default 10000)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
count=${1:-10000}
server=${KURA_DEV_SERVER:-http://127.0.0.1:58080}
issuer=https://localhost:59443/application/o/kura/
kura="$root/target/release/kura"
work=$(mktemp -d)
sql() { podman exec kura-dev-postgres psql -U postgres -d kura -tAc "$1"; }
cleanup() {
  # As DELETE /v1/vaults/{v} does: disabled, purged after 30 days. In SQL:
  # the run's access token may have expired by now.
  if [ -n "${vault:-}" ]; then
    sql "UPDATE vault SET disabled_at = now(), delete_after = now() + interval '30 days' WHERE id = '$vault'" >/dev/null
  fi
  sql "UPDATE device SET revoked_at = now() WHERE revoked_at IS NULL AND name LIKE 'perf-attach-%'" >/dev/null
  rm -rf "$work"
}
trap cleanup EXIT

cargo build -q --release -p kura-cli
# Real sign-ins with refresh: an access token lives minutes, an attach longer.
as() { local who=$1; shift; SSL_CERT_FILE="$root/dev/certs/ca.pem" KURA_CLI_HOME="$work/$who" "$kura" "$@"; }
login() { # login <who> <device name>: the device code flow, approved by dev/approve.sh
  as "$1" login --server "$server" --issuer "$issuer" --device-name "$2" >"$work/login-$1" 2>&1 &
  local pid=$! code=""
  for _ in $(seq 30); do
    code=$(awk 'NF==1 && /^[[:space:]]+[0-9A-Z-]+$/{print $1}' "$work/login-$1")
    [ -n "$code" ] && break
    sleep 1
  done
  [ -n "$code" ] || { echo "no device code: $(cat "$work/login-$1")"; exit 1; }
  "$root/dev/approve.sh" "$code" alice
  wait "$pid" || { echo "login failed: $(cat "$work/login-$1")"; exit 1; }
}
login a "perf-attach-a-$$"
login b "perf-attach-b-$$"
token=$("$root/dev/token.sh" alice)
device=$(jq -r .device "$work/a/config.json")
name="perf-$(date +%s)"
vault=$(curl -sf -X POST "$server/v1/vaults" -H "authorization: Bearer $token" -H "x-kura-device: $device" \
  -H 'content-type: application/json' -d "{\"tenant\":\"dev\",\"name\":\"$name\"}" | jq -r .id)

# Notes of 1 to 4 KiB of text, in 100 folders, as a vault of that size has.
mkdir -p "$work/notes-a"
for i in $(seq 0 $((count - 1))); do
  d="$work/notes-a/folder-$((i % 100))"
  mkdir -p "$d"
  head -c $((768 + (i * 37) % 2304)) /dev/urandom | base64 -w 76 >"$d/note-$i.md"
done
bytes=$(du -sb "$work/notes-a" | cut -f1)

t0=$(date +%s)
as a attach "$name" "$work/notes-a" --yes >/dev/null
t1=$(date +%s)
as b attach "$name" "$work/notes-b" --yes >/dev/null
t2=$(date +%s)
got=$(find "$work/notes-b" -name '*.md' | wc -l)
[ "$got" = "$count" ] || { echo "FAIL: device B has $got of $count notes"; exit 1; }
push=$((t1 - t0)); attach=$((t2 - t1))
mkdir -p "$root/perf/results"
printf '{"notes":%d,"bytes":%d,"pushSeconds":%d,"attachSeconds":%d}\n' "$count" "$bytes" "$push" "$attach" | tee "$root/perf/results/attach.json"
if [ "$attach" -gt 300 ]; then
  echo "decision 42: the first attach took ${attach}s, above 5 minutes: the batch chunk download RFC opens"
else
  echo "decision 42: the first attach took ${attach}s, under 5 minutes: no batch endpoint"
fi
