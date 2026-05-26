#!/usr/bin/env python3
import argparse
import json
import pathlib
from typing import Dict, Iterable


def load_results(path: pathlib.Path) -> Dict[str, dict]:
    with path.open() as f:
        rows = json.load(f)
    return {row["scenario"]: row for row in rows}


def parse_key_value(items: Iterable[str], flag: str) -> Dict[str, float]:
    values: Dict[str, float] = {}
    for item in items:
        if "=" not in item:
            raise SystemExit(f"{flag} expects SCENARIO=VALUE, got: {item}")
        key, value = item.split("=", 1)
        try:
            values[key] = float(value)
        except ValueError as exc:
            raise SystemExit(f"{flag} expects numeric VALUE, got: {item}") from exc
    return values


def regression_ratio(current: float, baseline: float) -> float:
    if baseline <= 0:
        return 0.0
    return max(0.0, (baseline - current) / baseline)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--current", required=True)
    parser.add_argument(
        "--require-scenario",
        action="append",
        default=[],
        help="Require that this hotspot scenario exists in both baseline and current results.",
    )
    parser.add_argument(
        "--metric",
        choices=("candidate_mb_s", "baseline_mb_s", "speedup"),
        default="candidate_mb_s",
        help="Metric compared against baseline for regression detection.",
    )
    parser.add_argument(
        "--max-regression",
        type=float,
        default=0.12,
        help="Allowed fractional regression vs baseline for the selected metric.",
    )
    parser.add_argument(
        "--scenario-max-regression",
        action="append",
        default=[],
        help="Per-scenario regression threshold in SCENARIO=VALUE form.",
    )
    parser.add_argument(
        "--min-speedup",
        action="append",
        default=[],
        help="Per-scenario minimum accepted current speedup in SCENARIO=VALUE form.",
    )
    args = parser.parse_args()

    baseline = load_results(pathlib.Path(args.baseline))
    current = load_results(pathlib.Path(args.current))
    scenario_max_regression = parse_key_value(
        args.scenario_max_regression, "--scenario-max-regression"
    )
    min_speedup = parse_key_value(args.min_speedup, "--min-speedup")

    required_scenarios = set(args.require_scenario)
    required_scenarios.update(scenario_max_regression)
    required_scenarios.update(min_speedup)

    missing = sorted(
        scenario
        for scenario in required_scenarios
        if scenario not in baseline or scenario not in current
    )
    if missing:
        raise SystemExit(
            "missing required hotspot scenarios: " + ", ".join(missing)
        )

    failures = []
    for scenario in sorted(required_scenarios):
        baseline_row = baseline[scenario]
        current_row = current[scenario]

        baseline_metric = float(baseline_row[args.metric])
        current_metric = float(current_row[args.metric])
        allowed_regression = scenario_max_regression.get(
            scenario, args.max_regression
        )
        actual_regression = regression_ratio(current_metric, baseline_metric)
        if actual_regression > allowed_regression:
            failures.append(
                (
                    scenario,
                    f"{args.metric} regressed by {actual_regression:.4f}, "
                    f"allowed {allowed_regression:.4f}",
                )
            )

        if scenario in min_speedup:
            speedup = float(current_row["speedup"])
            if speedup < min_speedup[scenario]:
                failures.append(
                    (
                        scenario,
                        f"speedup {speedup:.4f} is below floor {min_speedup[scenario]:.4f}",
                    )
                )

    if failures:
        details = "\n".join(f"- {scenario}: {message}" for scenario, message in failures)
        raise SystemExit(f"reconstruction hotspot gate failed:\n{details}")

    print(
        "reconstruction hotspot gate passed for "
        + ", ".join(sorted(required_scenarios))
    )


if __name__ == "__main__":
    main()
