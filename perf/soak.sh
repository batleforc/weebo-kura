#!/usr/bin/env bash
# Soak (weebo-testing k6 reference): the commit scenario for SOAK (default
# 30m) while the dev server's resident memory is sampled every 30 s. The gate
# is growth: after the first 5 minutes, memory must not rise by more than 25%.
# usage: perf/soak.sh [duration]
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
duration=${1:-${SOAK:-30m}}
pid=$(pgrep -f 'target/.*/kura-server --config dev/server.yaml' | head -1)
[ -n "$pid" ] || { echo "no dev server: task dev:run"; exit 1; }
mkdir -p "$root/perf/results"
csv="$root/perf/results/soak-rss.csv"
echo "seconds,rss_kib" >"$csv"
start=$(date +%s)
(while kill -0 "$pid" 2>/dev/null; do
  echo "$(($(date +%s) - start)),$(awk '/VmRSS/{print $2}' "/proc/$pid/status")" >>"$csv"
  sleep 30
done) &
sampler=$!
trap 'kill $sampler 2>/dev/null || true' EXIT
PERF_DURATION=$duration "$root/perf/run.sh" "$root/perf/k6/scenarios/01_commit.js"
# The baseline is the first sample after the warm-up; the gate, the last one.
awk -F, 'NR > 1 && $1 >= 300 && !base { base = $2 } NR > 1 { last = $2 }
  END { if (!base) { print "soak too short for a gate (under 5 min)"; exit 0 }
        printf "rss after warm-up %d KiB, at the end %d KiB (%+.1f%%)\n", base, last, (last - base) * 100 / base
        exit (last > base * 1.25) }' "$csv"
