#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

DATE_UTC="${1:-$(date -u +%F)}"
CPU_SLUG="${2:-$(lscpu | awk -F: '/Model name:/ {gsub(/^ +/,"",$2); print $2; exit}' | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]\+/-/g; s/^-//; s/-$//')}"

JSON_PATH="benchmarks/x86_64-simd/${DATE_UTC}-${CPU_SLUG}.json"
SUMMARY_PATH="docs/x86_64-simd-benchmark-summary-${DATE_UTC}-${CPU_SLUG}.md"
LEDGER_DRAFT_PATH="docs/x86_64-simd-ledger-entry-${DATE_UTC}-${CPU_SLUG}.md"
LEDGER_PATH="docs/x86_64-simd-benchmark-ledger.md"

./scripts/run_x86_backend_smoke_matrix.sh "${DATE_UTC}" "${CPU_SLUG}"

bash scripts/render_x86_simd_benchmark_summary.sh \
  --machine-json "${JSON_PATH}" \
  --machine-slug "${CPU_SLUG}" \
  --date "${DATE_UTC}" \
  --output "${SUMMARY_PATH}"

bash scripts/render_x86_simd_ledger_entry.sh \
  --machine-json "${JSON_PATH}" \
  --machine-slug "${CPU_SLUG}" \
  --date "${DATE_UTC}" \
  --output "${LEDGER_DRAFT_PATH}"

echo "saved machine json: ${JSON_PATH}"
echo "saved summary: ${SUMMARY_PATH}"
echo "saved ledger draft: ${LEDGER_DRAFT_PATH}"
echo "next step: review ${LEDGER_DRAFT_PATH} and merge it into ${LEDGER_PATH}"
echo "merge hint: cat ${LEDGER_DRAFT_PATH} >> ${LEDGER_PATH}"
