#!/usr/bin/env python3
import argparse
import json
import pathlib


def load_json(path: pathlib.Path):
    with path.open() as f:
        return json.load(f)


def render_entry(data: dict, machine_slug: str, date_utc: str) -> str:
    raw_priority = data.get("recommended_default_priority", {}).get("priority_order", [])
    policy_priority = data.get("policy_eligible_default_priority", {}).get(
        "priority_order", []
    )

    raw_lines = "\n".join(
        f"{idx}. `{backend}`" for idx, backend in enumerate(raw_priority, start=1)
    ) or "1. 待补充"
    policy_lines = "\n".join(
        f"{idx}. `{backend}`" for idx, backend in enumerate(policy_priority, start=1)
    ) or "1. 待补充"

    return f"""## {date_utc} {machine_slug}

### 机器

1. 机器标识：`{machine_slug}`
2. 日期：`{date_utc}`
3. 对应 JSON：`benchmarks/x86_64-simd/{date_utc}-{machine_slug}.json`

### Raw Benchmark Ranking

{raw_lines}

### Policy Eligible Default Priority

{policy_lines}

### 待补结论

1. 当前默认自动策略是否应调整：待补充
2. `GFNI` 是否仍保持 `override-only`：待补充
3. 是否与已有机器结论一致：待补充
4. 是否需要更多机器样本：待补充
"""


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--machine-json", required=True)
    parser.add_argument("--machine-slug", required=True)
    parser.add_argument("--date", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    data = load_json(pathlib.Path(args.machine_json))
    pathlib.Path(args.output).write_text(
        render_entry(data, args.machine_slug, args.date),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
