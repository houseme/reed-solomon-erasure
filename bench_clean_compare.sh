#!/usr/bin/env bash
set -euo pipefail

# Clean-build compare runner for backend A/B on this machine.
# Usage:
#   ./bench_clean_compare.sh galois_backend
#   ./bench_clean_compare.sh throughput_matrix

BENCH_NAME="${1:-galois_backend}"
FEATURES='std simd-accel'
COMMON_ARGS=(--sample-size 10 --warm-up-time 1 --measurement-time 1)

run_one() {
  local backend="$1"
  local log_path="$2"

  echo "==> backend=${backend}, bench=${BENCH_NAME}"
  cargo clean
  RSE_BACKEND_OVERRIDE="${backend}" \
    cargo bench --bench "${BENCH_NAME}" --features "${FEATURES}" -- "${COMMON_ARGS[@]}" \
    > "${log_path}" 2>&1
  echo "saved: ${log_path}"
}

if [[ "${BENCH_NAME}" != "galois_backend" && "${BENCH_NAME}" != "throughput_matrix" ]]; then
  echo "unsupported bench: ${BENCH_NAME}"
  echo "supported: galois_backend | throughput_matrix"
  exit 2
fi

run_one "rust-neon" "/tmp/${BENCH_NAME}-clean-rust-neon.log"
run_one "simd-c" "/tmp/${BENCH_NAME}-clean-simd-c.log"

echo "done"
