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

RAW_PRIORITY=$(format_priority "recommended_default_priority")
POLICY_PRIORITY=$(format_priority "policy_eligible_default_priority")

cat > "$OUTPUT" <<EOF
## ${DATE_UTC} ${MACHINE_SLUG}

### 机器

1. 机器标识：\`${MACHINE_SLUG}\`
2. 日期：\`${DATE_UTC}\`
3. 对应 JSON：\`benchmarks/x86_64-simd/${DATE_UTC}-${MACHINE_SLUG}.json\`

### Raw Benchmark Ranking

${RAW_PRIORITY}

### Policy Eligible Default Priority

${POLICY_PRIORITY}

### 待补结论

1. 当前默认自动策略是否应调整：待补充
2. \`GFNI\` 是否仍保持 \`override-only\`：待补充
3. 是否与已有机器结论一致：待补充
4. 是否需要更多机器样本：待补充
EOF
