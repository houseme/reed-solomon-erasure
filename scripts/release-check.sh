#!/usr/bin/env bash

set -euo pipefail

run() {
  echo
  echo "[release-check] $*"
  "$@"
}

run cargo check --tests
run cargo test --test selftest
run cargo test --test golden_vectors --test benchmark_smoke
run cargo test --no-default-features
run cargo test --features std

if [[ "${RUN_SIMD_ACCEL_TESTS:-1}" == "1" ]]; then
  run cargo test --features "std simd-accel"
  if [[ "$(uname -m)" == "x86_64" && "${RUN_X86_SIMD_OVERRIDE_MATRIX:-1}" == "1" ]]; then
    for backend in auto scalar rust-ssse3 simd-c rust-avx2 rust-avx512 rust-gfni-avx2 rust-gfni-avx512; do
      run env RSE_BACKEND_OVERRIDE="${backend}" RSE_STRICT_BACKEND_OVERRIDE=1 \
        cargo test --release --features "std simd-accel" --test benchmark_smoke \
          benchmark_smoke_matrix_runs_and_exports_results -- --nocapture
    done
  fi
else
  echo
  echo "[release-check] skipping simd-accel tests (RUN_SIMD_ACCEL_TESTS=0)"
fi
