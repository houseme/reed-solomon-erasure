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
else
  echo
  echo "[release-check] skipping simd-accel tests (RUN_SIMD_ACCEL_TESTS=0)"
fi
