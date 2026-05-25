#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found"
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 not found"
  exit 1
fi

DATE_UTC="$(date -u +%F)"
CPU_SLUG="${1:-}"
if [[ -z "${CPU_SLUG}" ]]; then
  CPU_SLUG="$(lscpu | awk -F: '/Model name:/ {gsub(/^ +/,\"\",$2); print $2; exit}' | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]\+/-/g; s/^-//; s/-$//')"
fi

OUT_DIR="benchmarks/x86_64-simd"
OUT_JSON="${OUT_DIR}/${DATE_UTC}-${CPU_SLUG}.json"

BACKENDS=(
  auto
  scalar
  simd-c
  rust-avx2
  rust-avx512
  rust-gfni-avx2
  rust-gfni-avx512
)

mkdir -p "${OUT_DIR}"
mkdir -p target/benchmark-smoke
mkdir -p target/criterion

run_smoke() {
  local backend="$1"
  local out_csv="target/benchmark-smoke/smoke-results-release-${backend}.csv"
  echo "==> release smoke: ${backend}"
  RSE_BACKEND_OVERRIDE="${backend}" \
    cargo test --release --features 'std simd-accel' --test benchmark_smoke \
      benchmark_smoke_matrix_runs_and_exports_results -- --nocapture
  cp target/benchmark-smoke/smoke-results.csv "${out_csv}"
}

run_bench() {
  local backend="$1"
  echo "==> criterion bench: ${backend}"
  RSE_BACKEND_OVERRIDE="${backend}" \
    cargo bench --bench galois_backend --features 'std simd-accel' -- \
      --sample-size 10 --warm-up-time 1 --measurement-time 1
}

for backend in "${BACKENDS[@]}"; do
  run_bench "${backend}"
  run_smoke "${backend}"
done

echo "==> writing ${OUT_JSON}"
python3 scripts/summarize_x86_simd_benchmarks.py \
  --root "${ROOT_DIR}" \
  --machine-json "${OUT_JSON}" \
  --machine-slug "${CPU_SLUG}" \
  --date "${DATE_UTC}"

echo "saved: ${OUT_JSON}"
