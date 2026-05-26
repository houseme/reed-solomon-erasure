#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

SMOKE_PROFILE="${SMOKE_PROFILE:-quick}"
SMOKE_ITERATIONS="${SMOKE_ITERATIONS:-1}"
SIMD_FEATURES="${SIMD_FEATURES:-std simd-accel}"

run() {
  echo
  echo "[serial-test-check] $*"
  "$@"
}

run cargo check --tests
run cargo test --test selftest -- --test-threads=1
run env RSE_SMOKE_PROFILE="${SMOKE_PROFILE}" RSE_SMOKE_ITERATIONS="${SMOKE_ITERATIONS}" \
  cargo test --test golden_vectors --test benchmark_smoke -- --test-threads=1
run cargo test --no-default-features -- --test-threads=1
run cargo test --features std -- --test-threads=1
run cargo test --features "${SIMD_FEATURES}" -- --test-threads=1
