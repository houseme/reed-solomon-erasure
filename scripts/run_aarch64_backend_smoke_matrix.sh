#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

BACKENDS=(
  auto
  scalar
  rust-neon
)

mkdir -p target/benchmark-smoke

for backend in "${BACKENDS[@]}"; do
  echo "==> smoke: ${backend}"
  RSE_BACKEND_OVERRIDE="${backend}" \
  RSE_STRICT_BACKEND_OVERRIDE=1 \
    cargo test --release --features 'std simd-accel' --test benchmark_smoke \
      benchmark_smoke_matrix_runs_and_exports_results -- --ignored --nocapture
  cp target/benchmark-smoke/smoke-results.csv \
    "target/benchmark-smoke/smoke-results-release-${backend}.csv"
done
