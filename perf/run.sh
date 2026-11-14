#!/usr/bin/env bash
# Runs one k6 scenario against the dev stack (task dev:run): a token for bob
# (an editor of `notes`), the vault's id and a blob it already stores; the
# run's devices are revoked afterwards (20 live devices per user, §4.4).
# usage: perf/run.sh <scenario.js> [k6 args…]
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
sql() { podman exec kura-dev-postgres psql -U postgres -d kura -tAc "$1"; }
scenario=$1; shift
PERF_TOKEN=$("$root/dev/token.sh" bob)
PERF_VAULT=$(sql "SELECT id FROM vault WHERE name = 'notes'")
read -r PERF_BLOB PERF_SIZE < <(sql "SELECT encode(hash, 'hex'), size FROM blob WHERE vault_id = '$PERF_VAULT' ORDER BY size DESC LIMIT 1" | tr '|' ' ')
[ -n "$PERF_BLOB" ] || { echo "no blob in notes: run task test:heavy once"; exit 1; }
export PERF_TOKEN PERF_VAULT PERF_BLOB PERF_SIZE
mkdir -p "$root/perf/results"
# The devices of this run only: their names carry its id.
PERF_RUN="$$-$(date +%s)"
export PERF_RUN
trap 'sql "UPDATE device SET revoked_at = now() WHERE revoked_at IS NULL AND name LIKE '"'"'perf-$PERF_RUN-%'"'"'" >/dev/null' EXIT
k6 run --summary-export "$root/perf/results/$(basename "$scenario" .js).json" "$@" "$scenario"
