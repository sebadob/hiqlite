#!/usr/bin/env bash
# Sweep Raft replication depth against openraft's `max_payload_entries`.
#
# Every cell runs the same binary. The depth is read from `HQL_RAFT_STREAM_DEPTH` inside the crate,
# so a single build covers the whole sweep. Each cell spawns one process per node, wipes its data
# dir at startup, and runs the client in a process of its own.
#
# Usage:
#   ./sweep.sh                              # 1000000 rows, concurrency 64, 1 rep, 128 256 512
#   ./sweep.sh 1000000 64 1 128 256 512    # the same, spelled out
#   DEPTHS="1 8 32" ./sweep.sh 1000000 64 2 128 256 512
#
# Knobs, all through the environment:
#   DEPTHS         depths to compare, space separated   (default "1 32")
#   NODE_THREADS   tokio threads per node process       (default 8)
#   CLIENT_THREADS tokio threads for the client         (default 4)
#   NODE_CPUS      core lists, one per node, space separated  (default "", Linux only)
#                  give each node whole physical cores, siblings included:
#                  NODE_CPUS="0-4,16-20 5-9,21-25 10-14,26-30" on a 16 core / 32 thread box
#                  siblings of a core: cat /sys/devices/system/cpu/cpu0/topology/thread_siblings_list
#   BENCH_ARGS     extra bench flags                    (default "", e.g. "--cache-only")
#   BENCH          binary to run                        (default target/release/bench)
#
# Nothing else should run while a sweep runs. A cell is only comparable when its nodes own their
# cores: pin them with NODE_CPUS on Linux, keep CLIENT_THREADS small, and do not run a sweep next
# to a build.
set -euo pipefail

rows="${1:-1000000}"
concurrency="${2:-64}"
reps="${3:-1}"
if [ "$#" -gt 3 ]; then
  shift 3
  payloads=("$@")
else
  payloads=(128 256 512)
fi

depths="${DEPTHS:-1 32}"
node_threads="${NODE_THREADS:-8}"
client_threads="${CLIENT_THREADS:-4}"
node_cpus="${NODE_CPUS:-}"
bench_args="${BENCH_ARGS:-}"

cd "$(dirname "$0")"
bench="${BENCH:-target/release/bench}"

echo "building once"
cargo build --release

stamp="$(date +%Y%m%d-%H%M%S)"
dir="sweep/${stamp}"
mkdir -p "$dir"
log="${dir}/sweep.log"
table="${dir}/table.txt"

{
  echo "# rows=${rows} concurrency=${concurrency} reps=${reps}"
  echo "# depths=${depths} node_threads=${node_threads} client_threads=${client_threads} node_cpus='${node_cpus}' bench_args='${bench_args}'"
  echo "# cores=$( (sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null) || echo '?') host=$(uname -srm)"
} > "$log"

for rep in $(seq 1 "$reps"); do
  for payload in "${payloads[@]}"; do
    for depth in $depths; do
      echo "=== rep=${rep} payload=${payload} depth=${depth} start=$(date -u +%H:%M:%S)" >> "$log"
      echo "rep=${rep} payload=${payload} depth=${depth}"
      # shellcheck disable=SC2086
      HQL_RAFT_STREAM_DEPTH="$depth" "$bench" cluster \
        --rows "$rows" \
        --concurrency "$concurrency" \
        --max-payload-entries "$payload" \
        --node-threads "$node_threads" \
        --client-threads "$client_threads" \
        --node-cpus "$node_cpus" \
        ${bench_args:+$bench_args} >> "$log" 2>&1
      sleep 3
    done
  done
done

awk \
  -v payloads="${payloads[*]}" \
  -v depths="$depths" \
  -v metrics="single INSERTs|cache PUTs" '
  /^=== rep=/ {
    payload = ""; depth = ""; idx = 0
    for (i = 1; i <= NF; i++) {
      if ($i ~ /^payload=/) { split($i, t, "="); payload = t[2] }
      if ($i ~ /^depth=/)   { split($i, t, "="); depth   = t[2] }
    }
    next
  }
  $2 == "ms" && $4 ~ /^[0-9]+$/ && $5 == "inserts" {
    idx++
    key = payload SUBSEP depth SUBSEP idx
    n[key]++
    sum[key] += $4
    if (!(key in mn) || $4 < mn[key]) mn[key] = $4
    if (!(key in mx) || $4 > mx[key]) mx[key] = $4
  }
  END {
    split(payloads, P, " ")
    split(depths, D, " ")
    split(metrics, M, "|")
    print ""
    for (i = 1; i in P; i++) {
      printf "max_payload_entries=%s\n", P[i]
      printf "  %-16s", "metric"
      for (j = 1; j in D; j++) printf " %14s", "depth " D[j]
      printf "   delta\n"
      for (k = 1; k in M; k++) {
        printf "  %-16s", M[k]
        first = ""; last = ""
        for (j = 1; j in D; j++) {
          key = P[i] SUBSEP D[j] SUBSEP k
          if (n[key] == 0) { printf " %14s", "-"; continue }
          avg = int(sum[key] / n[key] + 0.5)
          cell = (n[key] == 1) ? sprintf("%d", avg) : sprintf("%d [%d-%d]", avg, mn[key], mx[key])
          printf " %14s", cell
          if (first == "") first = avg
          last = avg
        }
        printf "  %+6.1f%%\n", (first > 0 ? (last - first) / first * 100 : 0)
      }
      print ""
    }
  }
' "$log" | tee "$table"

echo ""
echo "log:   $log"
echo "table: $table"
