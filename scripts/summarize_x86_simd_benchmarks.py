#!/usr/bin/env python3
import argparse
import csv
import json
import os
import pathlib
import platform
import subprocess
from typing import Dict, List


def load_json(path: pathlib.Path):
    with path.open() as f:
        return json.load(f)


def load_csv(path: pathlib.Path):
    with path.open() as f:
        return list(csv.DictReader(f))


def collect_criterion(root: pathlib.Path) -> List[Dict]:
    criterion_dir = root / "target" / "criterion"
    rows = []
    for path in sorted(criterion_dir.glob("**/new/estimates.json")):
        rel = path.relative_to(criterion_dir)
        parts = rel.parts
        if len(parts) < 3:
            continue
        bench_name = parts[0]
        length = parts[1]
        data = load_json(path)
        rows.append(
            {
                "benchmark": bench_name,
                "length": length,
                "mean_ns": data["mean"]["point_estimate"],
                "lower_ns": data["mean"]["confidence_interval"]["lower_bound"],
                "upper_ns": data["mean"]["confidence_interval"]["upper_bound"],
            }
        )
    return rows


def collect_release_smoke(root: pathlib.Path) -> Dict[str, List[Dict]]:
    smoke_dir = root / "target" / "benchmark-smoke"
    out = {}
    for path in sorted(smoke_dir.glob("smoke-results-release-*.csv")):
        out[path.name] = load_csv(path)
    return out


def backend_rankings(machine_json: Dict) -> Dict[str, List[Dict]]:
    rankings = {}
    smoke = machine_json["release_smoke"]
    focus_case = {
        "data_shards": "10",
        "parity_shards": "4",
        "shard_size": "1048576",
    }
    for op in ["encode", "verify", "reconstruct", "reconstruct_data"]:
        rows = []
        for file_name, records in smoke.items():
            for record in records:
                if (
                    record["operation"] == op
                    and record["data_shards"] == focus_case["data_shards"]
                    and record["parity_shards"] == focus_case["parity_shards"]
                    and record["shard_size"] == focus_case["shard_size"]
                ):
                    rows.append(
                        {
                            "backend": record["backend"],
                            "backend_override": record["backend_override"],
                            "throughput_mb_s": float(record["throughput_mb_s"]),
                            "source": file_name,
                        }
                    )
        rankings[op] = sorted(rows, key=lambda item: item["throughput_mb_s"], reverse=True)
    return rankings


def write_machine_json(root: pathlib.Path, out_json: pathlib.Path, machine_slug: str, date_utc: str):
    report = {
        "date_utc": date_utc,
        "machine_slug": machine_slug,
        "hostname": platform.node(),
        "arch": platform.machine(),
        "lscpu": subprocess.check_output(["lscpu"], text=True),
        "criterion_galois_backend": collect_criterion(root),
        "release_smoke": collect_release_smoke(root),
    }
    report["rankings_10x4_1m"] = backend_rankings(report)
    out_json.parent.mkdir(parents=True, exist_ok=True)
    out_json.write_text(json.dumps(report, indent=2))


def print_summary(root: pathlib.Path):
    bench_dir = root / "benchmarks" / "x86_64-simd"
    rows = []
    for path in sorted(bench_dir.glob("*.json")):
        data = load_json(path)
        rankings = data.get("rankings_10x4_1m", {})
        top = {op: (rankings.get(op) or [{}])[0] for op in ["encode", "verify", "reconstruct", "reconstruct_data"]}
        rows.append(
            {
                "file": path.name,
                "encode": top["encode"].get("backend_override", "n/a"),
                "verify": top["verify"].get("backend_override", "n/a"),
                "reconstruct": top["reconstruct"].get("backend_override", "n/a"),
                "reconstruct_data": top["reconstruct_data"].get("backend_override", "n/a"),
            }
        )
    print(json.dumps(rows, indent=2))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--machine-json")
    parser.add_argument("--machine-slug")
    parser.add_argument("--date")
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()

    root = pathlib.Path(args.root).resolve()

    if args.summary:
        print_summary(root)
        return

    if not args.machine_json or not args.machine_slug or not args.date:
        raise SystemExit("--machine-json, --machine-slug and --date are required unless --summary is used")

    write_machine_json(root, pathlib.Path(args.machine_json), args.machine_slug, args.date)


if __name__ == "__main__":
    main()
