#[path = "../benches/common/mod.rs"]
mod bench_common;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use reed_solomon_erasure::galois_8::ReedSolomon;

use self::bench_common::{derived_seed, make_full_shards, BenchCase, Operation, SMOKE_CASES};

struct SmokeResult {
    operation: &'static str,
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    seed: u64,
    throughput_mb_s: f64,
    ns_per_iter: f64,
}

fn git_revision() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn features() -> String {
    let mut enabled = Vec::new();
    if cfg!(feature = "std") {
        enabled.push("std");
    }
    if cfg!(feature = "simd-accel") {
        enabled.push("simd-accel");
    }
    if enabled.is_empty() {
        "none".to_string()
    } else {
        enabled.join("|")
    }
}

fn backend() -> &'static str {
    if cfg!(feature = "simd-accel") {
        "simd-c"
    } else {
        "scalar"
    }
}

fn target_triple() -> String {
    format!(
        "{}-{}-{}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        option_env!("CARGO_CFG_TARGET_ENV").unwrap_or("unknown"),
    )
}

fn run_operation(case: BenchCase, operation: Operation, iterations: usize) -> SmokeResult {
    let seed = derived_seed(operation, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let bytes = (case.shard_size * case.data_shards) as f64;

    let start = Instant::now();
    for _ in 0..iterations {
        match operation {
            Operation::Encode => {
                let mut shards =
                    make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
                rs.encode(&mut shards).unwrap();
            }
            Operation::Verify => {
                let mut shards =
                    make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
                rs.encode(&mut shards).unwrap();
                rs.verify(&shards).unwrap();
            }
            Operation::Reconstruct => {
                let mut shards =
                    make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
                rs.encode(&mut shards).unwrap();
                let mut shards: Vec<Option<Vec<u8>>> = shards.into_iter().map(Some).collect();
                shards[0] = None;
                shards[case.data_shards] = None;
                rs.reconstruct(&mut shards).unwrap();
            }
            Operation::ReconstructData => {
                let mut shards =
                    make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
                rs.encode(&mut shards).unwrap();
                let mut shards: Vec<Option<Vec<u8>>> = shards.into_iter().map(Some).collect();
                shards[0] = None;
                shards[1] = None;
                rs.reconstruct_data(&mut shards).unwrap();
            }
        }
    }
    let elapsed = start.elapsed();
    let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
    let throughput_mb_s = bytes / (1024.0 * 1024.0) / (ns_per_iter / 1_000_000_000.0);

    SmokeResult {
        operation: operation.as_str(),
        data_shards: case.data_shards,
        parity_shards: case.parity_shards,
        shard_size: case.shard_size,
        seed,
        throughput_mb_s,
        ns_per_iter,
    }
}

fn write_results(results: &[SmokeResult]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let revision = git_revision();
    let target = target_triple();
    let features = features();
    let backend = backend();

    let json_path = dir.join("smoke-results.json");
    let csv_path = dir.join("smoke-results.csv");

    let mut json = String::from("[\n");
    for (i, result) in results.iter().enumerate() {
        let suffix = if i + 1 == results.len() { "\n" } else { ",\n" };
        json.push_str(&format!(
            "  {{\"git_revision\":\"{}\",\"target_triple\":\"{}\",\"features\":\"{}\",\"backend\":\"{}\",\"operation\":\"{}\",\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"seed\":{},\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2}}}{}",
            revision,
            target,
            features,
            backend,
            result.operation,
            result.data_shards,
            result.parity_shards,
            result.shard_size,
            result.seed,
            result.throughput_mb_s,
            result.ns_per_iter,
            suffix
        ));
    }
    json.push(']');
    fs::write(&json_path, json).unwrap();

    let mut csv = String::from(
        "git_revision,target_triple,features,backend,operation,data_shards,parity_shards,shard_size,seed,throughput_mb_s,ns_per_iter\n",
    );
    for result in results {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.4},{:.2}\n",
            revision,
            target,
            features,
            backend,
            result.operation,
            result.data_shards,
            result.parity_shards,
            result.shard_size,
            result.seed,
            result.throughput_mb_s,
            result.ns_per_iter
        ));
    }
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

#[test]
fn benchmark_smoke_matrix_runs_and_exports_results() {
    let mut results = Vec::new();
    for case in SMOKE_CASES {
        results.push(run_operation(*case, Operation::Encode, 3));
        results.push(run_operation(*case, Operation::Verify, 3));
        results.push(run_operation(*case, Operation::Reconstruct, 3));
        results.push(run_operation(*case, Operation::ReconstructData, 3));
    }

    assert!(!results.is_empty());
    assert!(results.iter().all(|result| result.throughput_mb_s.is_finite()));
    write_results(&results);
}
