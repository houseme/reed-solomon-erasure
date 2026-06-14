#!/usr/bin/env bash
set -euo pipefail

PROFILE="${RSE_SMALL_FILE_PROFILE:-fast}"
ITERATIONS="${RSE_SMALL_FILE_ITERATIONS:-}"
FEATURES="${RSE_SMALL_FILE_FEATURES:-std simd-accel}"

CMD=(
  cargo test --release
  --features "$FEATURES"
  --test benchmark_small_files
  benchmark_small_file_matrix_runs_and_exports_results
  --
  --ignored
  --nocapture
)

echo "==> Running small-file EC benchmark matrix"
echo "    profile: $PROFILE"
if [[ -n "$ITERATIONS" ]]; then
  echo "    iterations override: $ITERATIONS"
else
  echo "    iterations override: <default>"
fi
echo "    features: $FEATURES"

if [[ -n "$ITERATIONS" ]]; then
  RSE_SMALL_FILE_PROFILE="$PROFILE" \
  RSE_SMALL_FILE_ITERATIONS="$ITERATIONS" \
  "${CMD[@]}"
else
  RSE_SMALL_FILE_PROFILE="$PROFILE" \
  "${CMD[@]}"
fi

echo "==> Artifacts"
echo "    target/benchmark-smoke/small-file-results.json"
echo "    target/benchmark-smoke/small-file-results.csv"
