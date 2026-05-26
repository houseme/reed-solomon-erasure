#!/usr/bin/env bash

set -euo pipefail

VALIDATION_PROFILE="${VALIDATION_PROFILE:-fast}"

run() {
  echo
  echo "[release-check] $*"
  "$@"
}

run_smoke_profile() {
  local profile="$1"
  shift
  run env RSE_SMOKE_PROFILE="${profile}" "$@"
}

run_fast_checks() {
  run cargo check --tests
  run cargo test --test selftest
  run_smoke_profile fast cargo test --test golden_vectors --test benchmark_smoke
  run cargo test --no-default-features
  run cargo test --features std
}

run_extended_checks() {
  if [[ -n "${RSE_SMOKE_BASELINE:-}" ]]; then
    run python3 scripts/check_benchmark_regression.py \
      --baseline "${RSE_SMOKE_BASELINE}" \
      --current target/benchmark-smoke/smoke-results.json \
      --require-case encode:4:2:65536 \
      --require-case encode:10:4:1048576 \
      --require-case verify:10:4:1048576 \
      --require-case reconstruct:10:4:1048576 \
      --require-case reconstruct_data:10:4:1048576
  else
    echo
    echo "[release-check] skipping benchmark regression gate (set RSE_SMOKE_BASELINE=/path/to/smoke-results.json)"
  fi

  if [[ "${RUN_BACKEND_CONSISTENCY:-0}" == "1" ]]; then
    run bash scripts/check_backend_consistency.sh
  else
    echo
    echo "[release-check] skipping backend consistency sweep (set RUN_BACKEND_CONSISTENCY=1)"
  fi

  if [[ "${RUN_SIMD_ACCEL_TESTS:-1}" == "1" ]]; then
    run cargo test --features "std simd-accel"
    if [[ "$(uname -m)" == "x86_64" && "${RUN_X86_SIMD_OVERRIDE_MATRIX:-1}" == "1" ]]; then
      for backend in auto scalar rust-ssse3 simd-c rust-avx2 rust-avx512 rust-gfni-avx2 rust-gfni-avx512; do
        run env RSE_BACKEND_OVERRIDE="${backend}" RSE_STRICT_BACKEND_OVERRIDE=1 RSE_SMOKE_PROFILE=extended \
          cargo test --release --features "std simd-accel" --test benchmark_smoke \
            benchmark_smoke_matrix_runs_and_exports_results -- --nocapture
      done
    fi
  else
    echo
    echo "[release-check] skipping simd-accel tests (RUN_SIMD_ACCEL_TESTS=0)"
  fi
}

run_fast_checks

if [[ "${VALIDATION_PROFILE}" == "extended" ]]; then
  run_extended_checks
else
  echo
  echo "[release-check] fast profile complete; skip extended checks (set VALIDATION_PROFILE=extended)"
fi
