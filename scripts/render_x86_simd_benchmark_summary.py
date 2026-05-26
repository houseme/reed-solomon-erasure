#!/usr/bin/env python3
import argparse
import json
import pathlib


def load_json(path: pathlib.Path):
    with path.open() as f:
        return json.load(f)


def top_rows(rankings, op, limit=3):
    return rankings.get(op, [])[:limit]


def format_rows(rows):
    lines = []
    for idx, row in enumerate(rows, start=1):
        lines.append(
            f"{idx}. `{row['backend_override']}`: `{row['throughput_mb_s']:.4f} MB/s`"
        )
    if not lines:
        lines.append("1. 待补充")
    return "\n".join(lines)


def format_priority(rows):
    if not rows:
        return "1. 待补充"
    return "\n".join(
        f"{idx}. `{backend}`" for idx, backend in enumerate(rows, start=1)
    )


def render_summary(json_path: pathlib.Path, machine_slug: str, date_utc: str):
    data = load_json(json_path)
    rankings = data.get("rankings_10x4_1m", {})
    raw_priority = data.get("recommended_default_priority", {}).get("priority_order", [])
    policy_priority = data.get("policy_eligible_default_priority", {}).get(
        "priority_order", []
    )

    return f"""# x86_64 SIMD Benchmark Summary ({date_utc}, {machine_slug})

## 范围

本摘要对应以下实测产物：

1. [benchmarks/x86_64-simd/{json_path.name}](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/{json_path.name})
2. `target/benchmark-smoke/smoke-results-release-*.csv`
3. `cargo bench --bench galois_backend --features 'std simd-accel'` 的当前 Criterion 输出

机器环境：

1. 机器标识：`{machine_slug}`
2. 测试日期：`{date_utc}`
3. 详细 `lscpu` 信息已包含在 machine JSON 中

## 10x4_1m Release Smoke 排名

`encode`

{format_rows(top_rows(rankings, "encode"))}

`verify`

{format_rows(top_rows(rankings, "verify"))}

`reconstruct`

{format_rows(top_rows(rankings, "reconstruct"))}

`reconstruct_data`

{format_rows(top_rows(rankings, "reconstruct_data"))}

## 综合打分结果

### Raw Benchmark Ranking

{format_priority(raw_priority)}

### Policy Eligible Default Priority

{format_priority(policy_priority)}

## 结论模板

1. 当前默认自动策略是否应调整：待补充
2. `GFNI` 是否仍保持 `override-only`：待补充
3. 与已有 `AMD EPYC 9V45` 结果是否一致：待补充
4. 是否需要更多机器样本：待补充
"""


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--machine-json", required=True)
    parser.add_argument("--machine-slug", required=True)
    parser.add_argument("--date", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    json_path = pathlib.Path(args.machine_json)
    output_path = pathlib.Path(args.output)
    output_path.write_text(
        render_summary(json_path, args.machine_slug, args.date),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
