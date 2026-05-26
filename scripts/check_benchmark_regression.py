#!/usr/bin/env python3
import argparse
import csv
import json
import pathlib
import sys

OPERATIONS = ("encode", "verify", "reconstruct", "reconstruct_data")
CASE_KEY = ("data_shards", "parity_shards", "shard_size")
DEFAULT_THRESHOLDS = {
    "encode": 0.10,
    "verify": 0.12,
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
        baseline_tp = baseline_record["throughput_mb_s"]
        current_tp = current_record["throughput_mb_s"]
        if baseline_tp <= 0.0:
            continue
        regression_ratio = max(0.0, (baseline_tp - current_tp) / baseline_tp)
        comparisons.append(
            {
                "operation": op,
                "data_shards": key[1],
                "parity_shards": key[2],
                "shard_size": key[3],
                "baseline_throughput_mb_s": baseline_tp,
                "current_throughput_mb_s": current_tp,
                "regression_ratio": regression_ratio,
                "threshold": thresholds[op],
            }
        )
        if regression_ratio > thresholds[op]:
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
