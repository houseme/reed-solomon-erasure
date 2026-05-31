#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 --machine-json FILE --machine-slug SLUG --date DATE --output FILE"
  exit 1
}

MACHINE_JSON=""
MACHINE_SLUG=""
DATE_UTC=""
OUTPUT=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --machine-json) MACHINE_JSON="$2"; shift 2 ;;
    --machine-slug) MACHINE_SLUG="$2"; shift 2 ;;
    --date) DATE_UTC="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    *) usage ;;
  esac
done

[[ -z "$MACHINE_JSON" || -z "$MACHINE_SLUG" || -z "$DATE_UTC" || -z "$OUTPUT" ]] && usage

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: jq is required but not installed" >&2
  exit 1
fi

format_rows() {
  local op="$1"
  local data
  data=$(jq -r --arg op "$op" '
    (.rankings_10x4_1m[$op] // [])[:3][] |
    "\(.backend_override)\t\(.throughput_mb_s)"
  ' "$MACHINE_JSON" 2>/dev/null || true)

  if [[ -z "$data" ]]; then
    echo "1. 待补充"
    return
  fi

  local idx=1
  while IFS=$'\t' read -r backend throughput; do
    printf '%d. `%s`: `%.4f MB/s`\n' "$idx" "$backend" "$throughput"
    idx=$((idx + 1))
  done <<< "$data"
}

format_priority() {
  local key="$1"
  local data
  data=$(jq -r --arg key "$key" '
    (.[$key].priority_order // [])[]?
  ' "$MACHINE_JSON" 2>/dev/null || true)

  if [[ -z "$data" ]]; then
    echo "1. 待补充"
    return
  fi

  local idx=1
  while IFS= read -r backend; do
    printf '%d. `%s`\n' "$idx" "$backend"
    idx=$((idx + 1))
  done <<< "$data"
}

ENCODE_ROWS=$(format_rows "encode")
VERIFY_ROWS=$(format_rows "verify")
RECONSTRUCT_ROWS=$(format_rows "reconstruct")
RECONSTRUCT_DATA_ROWS=$(format_rows "reconstruct_data")
RAW_PRIORITY=$(format_priority "recommended_default_priority")
POLICY_PRIORITY=$(format_priority "policy_eligible_default_priority")

JSON_NAME=$(basename "$MACHINE_JSON")

cat > "$OUTPUT" <<EOF
# x86_64 SIMD Benchmark Summary (${DATE_UTC}, ${MACHINE_SLUG})

## 范围

本摘要对应以下实测产物：

1. [benchmarks/x86_64-simd/${JSON_NAME}](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/${JSON_NAME})
2. \`target/benchmark-smoke/smoke-results-release-*.csv\`
3. \`cargo bench --bench galois_backend --features 'std simd-accel'\` 的当前 Criterion 输出

机器环境：

1. 机器标识：\`${MACHINE_SLUG}\`
2. 测试日期：\`${DATE_UTC}\`
3. 详细 \`lscpu\` 信息已包含在 machine JSON 中

## 10x4_1m Release Smoke 排名

\`encode\`

${ENCODE_ROWS}

\`verify\`

${VERIFY_ROWS}

\`reconstruct\`

${RECONSTRUCT_ROWS}

\`reconstruct_data\`

${RECONSTRUCT_DATA_ROWS}

## 综合打分结果

### Raw Benchmark Ranking

${RAW_PRIORITY}

### Policy Eligible Default Priority

${POLICY_PRIORITY}

## 结论模板

1. 当前默认自动策略是否应调整：待补充
2. \`GFNI\` 是否仍保持 \`override-only\`：待补充
3. 与已有 \`AMD EPYC 9V45\` 结果是否一致：待补充
4. 是否需要更多机器样本：待补充
EOF
