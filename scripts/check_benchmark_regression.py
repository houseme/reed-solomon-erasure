#!/usr/bin/env python3
import argparse
import csv
import json
import pathlib
import sys

OPERATIONS = (
    "encode",
    "verify",
    "verify_with_buffer",
    "reconstruct",
    "reconstruct_data",
)
CASE_KEY = ("data_shards", "parity_shards", "shard_size")
DEFAULT_THRESHOLDS = {
    "encode": 0.10,
    "verify": 0.12,
    "verify_with_buffer": 0.12,
    "reconstruct": 0.15,
    "reconstruct_data": 0.15,
}


def load_rows(path: pathlib.Path):
    if path.suffix == ".json":
        return json.loads(path.read_text())
    if path.suffix == ".csv":
        with path.open() as f:
            return list(csv.DictReader(f))
    raise SystemExit(f"unsupported file type: {path}")


def normalize_record(record):
    normalized = dict(record)
    normalized["operation"] = str(record["operation"])
    normalized["throughput_mb_s"] = float(record["throughput_mb_s"])
    normalized["ns_per_iter"] = float(record["ns_per_iter"])
    for key in CASE_KEY:
        normalized[key] = str(record[key])
    return normalized


def index_records(rows):
    indexed = {}
    for row in rows:
        record = normalize_record(row)
        key = (
            record["operation"],
            record["data_shards"],
            record["parity_shards"],
            record["shard_size"],
        )
        indexed[key] = record
    return indexed


def parse_threshold_overrides(values):
    thresholds = dict(DEFAULT_THRESHOLDS)
    for value in values:
        if "=" not in value:
            raise SystemExit(f"invalid threshold override '{value}', expected op=value")
        op, raw = value.split("=", 1)
        op = op.strip()
        if op not in OPERATIONS:
            raise SystemExit(f"unsupported operation in threshold override: {op}")
        thresholds[op] = float(raw)
    return thresholds


def metric_value(record, metric):
    if metric == "throughput_mb_s":
        return record["throughput_mb_s"]
    if metric == "ns_per_iter":
        return record["ns_per_iter"]
    raise SystemExit(f"unsupported metric: {metric}")


def regression_ratio(baseline_value, current_value, metric):
    if baseline_value <= 0.0:
        return 0.0
    if metric == "throughput_mb_s":
        return max(0.0, (baseline_value - current_value) / baseline_value)
    if metric == "ns_per_iter":
        return max(0.0, (current_value - baseline_value) / baseline_value)
    raise SystemExit(f"unsupported metric: {metric}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--current", required=True)
    parser.add_argument(
        "--threshold",
        action="append",
        default=[],
        help="Override allowed regression ratio, e.g. reconstruct=0.18",
    )
    parser.add_argument(
        "--metric",
        choices=("throughput_mb_s", "ns_per_iter"),
        default="throughput_mb_s",
        help="Metric used for regression detection. Use ns_per_iter for latency-sensitive small-file checks.",
    )
    parser.add_argument(
        "--require-case",
        action="append",
        default=[],
        help="Require a specific case in operation:data:parity:shard_size format",
    )
    args = parser.parse_args()

    baseline_path = pathlib.Path(args.baseline)
    current_path = pathlib.Path(args.current)
    thresholds = parse_threshold_overrides(args.threshold)

    baseline = index_records(load_rows(baseline_path))
    current = index_records(load_rows(current_path))

    failures = []
    comparisons = []

    for key, current_record in sorted(current.items()):
        if key not in baseline:
            continue
        baseline_record = baseline[key]
        op = current_record["operation"]
        baseline_value = metric_value(baseline_record, args.metric)
        current_value = metric_value(current_record, args.metric)
        if baseline_value <= 0.0:
            continue
        ratio = regression_ratio(baseline_value, current_value, args.metric)
        comparisons.append(
            {
                "operation": op,
                "data_shards": key[1],
                "parity_shards": key[2],
                "shard_size": key[3],
                "metric": args.metric,
                "baseline_metric_value": baseline_value,
                "current_metric_value": current_value,
                "baseline_throughput_mb_s": baseline_record["throughput_mb_s"],
                "current_throughput_mb_s": current_record["throughput_mb_s"],
                "baseline_ns_per_iter": baseline_record["ns_per_iter"],
                "current_ns_per_iter": current_record["ns_per_iter"],
                "regression_ratio": ratio,
                "threshold": thresholds[op],
            }
        )
        if ratio > thresholds[op]:
            failures.append(comparisons[-1])

    for required in args.require_case:
        parts = required.split(":")
        if len(parts) != 4:
            raise SystemExit(
                f"invalid required case '{required}', expected operation:data:parity:shard_size"
            )
        key = tuple(parts)
        if key not in current:
            failures.append(
                {
                    "operation": parts[0],
                    "data_shards": parts[1],
                    "parity_shards": parts[2],
                    "shard_size": parts[3],
                    "error": "required case missing from current results",
                }
            )

    summary = {
        "baseline": str(baseline_path),
        "current": str(current_path),
        "metric": args.metric,
        "thresholds": thresholds,
        "comparisons": comparisons,
        "failures": failures,
    }
    print(json.dumps(summary, indent=2))

    if failures:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
