#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

DATE_UTC="${1:-$(date -u +%F)}"
CPU_SLUG="${2:-$(lscpu | awk -F: '/Model name:/ {gsub(/^ +/,"",$2); print $2; exit}' | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]\+/-/g; s/^-//; s/-$//')}"

BACKENDS=(
  auto
  scalar
  rust-ssse3
  simd-c
  rust-avx2
  rust-avx512
  rust-gfni-avx2
  rust-gfni-avx512
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

OUT_JSON="benchmarks/x86_64-simd/${DATE_UTC}-${CPU_SLUG}.json"
python3 scripts/summarize_x86_simd_benchmarks.py \
  --root "${ROOT_DIR}" \
  --machine-json "${OUT_JSON}" \
  --machine-slug "${CPU_SLUG}" \
  --date "${DATE_UTC}"

echo "saved: ${OUT_JSON}"
