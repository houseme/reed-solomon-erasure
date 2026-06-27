#![cfg(feature = "std")]

#[path = "../benches/common/mod.rs"]
mod bench_common;
mod common;

use std::io::Cursor;
use std::time::Instant;
use std::{fs, path::PathBuf};

use rustfs_erasure_codec::galois_8::ReedSolomon;
use rustfs_erasure_codec::stream::StreamOptions;

use self::bench_common::{
    ARTIFACT_SCHEMA_VERSION, BenchCase, Operation, backend, backend_id, backend_kind,
    backend_override, benchmark_metrics_enabled, derived_seed, features, git_revision,
    make_data_shards, target_triple,
};
use self::common::{assert_backend_override_honored_if_strict, override_honored};

#[derive(Clone, Copy, Debug)]
enum StreamOperation {
    Encode,
    Verify,
    Reconstruct,
}

impl StreamOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Encode => "encode_stream",
            Self::Verify => "verify_stream",
            Self::Reconstruct => "reconstruct_stream",
        }
    }

    fn seed_operation(self) -> Operation {
        match self {
            Self::Encode => Operation::Encode,
            Self::Verify => Operation::Verify,
            Self::Reconstruct => Operation::Reconstruct,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct StreamCase {
    case: BenchCase,
    block_size: usize,
}

struct StreamResult {
    operation: &'static str,
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    logical_data_bytes: usize,
    case_label: &'static str,
    seed: u64,
    stream_block_size: usize,
    stream_io_backend: &'static str,
    stream_io_mode: &'static str,
    blocks_per_iter: usize,
    throughput_mb_s: f64,
    ns_per_iter: f64,
    ns_per_block: f64,
}

const QUICK_STREAM_CASES: &[StreamCase] = &[
    StreamCase {
        case: BenchCase {
            data_shards: 4,
            parity_shards: 2,
            shard_size: 64 * 1024,
            label: "4x2_64k",
        },
        block_size: 64 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 4,
            parity_shards: 2,
            shard_size: 64 * 1024,
            label: "4x2_64k",
        },
        block_size: 256 * 1024,
    },
];

const FAST_STREAM_CASES: &[StreamCase] = &[
    StreamCase {
        case: BenchCase {
            data_shards: 4,
            parity_shards: 2,
            shard_size: 64 * 1024,
            label: "4x2_64k",
        },
        block_size: 64 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 4,
            parity_shards: 2,
            shard_size: 1024 * 1024,
            label: "4x2_1m",
        },
        block_size: 256 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 10,
            parity_shards: 4,
            shard_size: 64 * 1024,
            label: "10x4_64k",
        },
        block_size: 64 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 10,
            parity_shards: 4,
            shard_size: 1024 * 1024,
            label: "10x4_1m",
        },
        block_size: 1024 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 10,
            parity_shards: 4,
            shard_size: 16 * 1024 * 1024,
            label: "10x4_16m",
        },
        block_size: 4 * 1024 * 1024,
    },
];

const EXTENDED_STREAM_CASES: &[StreamCase] = &[
    StreamCase {
        case: BenchCase {
            data_shards: 4,
            parity_shards: 2,
            shard_size: 64 * 1024,
            label: "4x2_64k",
        },
        block_size: 64 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 4,
            parity_shards: 2,
            shard_size: 1024 * 1024,
            label: "4x2_1m",
        },
        block_size: 256 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 10,
            parity_shards: 4,
            shard_size: 64 * 1024,
            label: "10x4_64k",
        },
        block_size: 64 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 10,
            parity_shards: 4,
            shard_size: 1024 * 1024,
            label: "10x4_1m",
        },
        block_size: 1024 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 16,
            parity_shards: 4,
            shard_size: 1024 * 1024,
            label: "16x4_1m",
        },
        block_size: 1024 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 32,
            parity_shards: 16,
            shard_size: 1024 * 1024,
            label: "32x16_1m",
        },
        block_size: 1024 * 1024,
    },
    StreamCase {
        case: BenchCase {
            data_shards: 64,
            parity_shards: 20,
            shard_size: 1024 * 1024,
            label: "64x20_1m",
        },
        block_size: 1024 * 1024,
    },
];

fn stream_profile() -> &'static str {
    std::env::var("RSE_STREAM_PROFILE")
        .ok()
        .as_deref()
        .map(|value| match value {
            "extended" => "extended",
            "quick" => "quick",
            _ => "fast",
        })
        .unwrap_or("fast")
}

fn stream_cases() -> &'static [StreamCase] {
    match stream_profile() {
        "quick" => QUICK_STREAM_CASES,
        "extended" => EXTENDED_STREAM_CASES,
        _ => FAST_STREAM_CASES,
    }
}

fn stream_iterations() -> usize {
    std::env::var("RSE_STREAM_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| match stream_profile() {
            "extended" => 2,
            "quick" => 3,
            _ => 3,
        })
}

fn selected_stream_cases() -> Vec<StreamCase> {
    let cases = stream_cases();
    let Some(raw_filter) = std::env::var("RSE_STREAM_CASE_FILTER").ok() else {
        return cases.to_vec();
    };
    let wanted: Vec<&str> = raw_filter
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if wanted.is_empty() {
        return cases.to_vec();
    }

    cases
        .iter()
        .copied()
        .filter(|case| wanted.contains(&case.case.label))
        .collect()
}

fn make_encoded_shards(case: BenchCase, seed: u64) -> Vec<Vec<u8>> {
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let mut data = make_data_shards(seed, case.data_shards, case.shard_size);
    let mut parity = vec![vec![0u8; case.shard_size]; case.parity_shards];
    let data_refs: Vec<&[u8]> = data.iter().map(Vec::as_slice).collect();
    let mut parity_refs: Vec<&mut [u8]> = parity.iter_mut().map(Vec::as_mut_slice).collect();
    rs.encode_sep(&data_refs, &mut parity_refs).unwrap();
    data.extend(parity);
    data
}

fn run_stream_operation(
    stream_case: StreamCase,
    operation: StreamOperation,
    iterations: usize,
) -> StreamResult {
    let case = stream_case.case;
    let seed = derived_seed(operation.seed_operation(), case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let logical_data_bytes = case.data_shards * case.shard_size;
    let block_size = StreamOptions::new()
        .with_block_size(stream_case.block_size)
        .block_size;
    let blocks_per_iter = case.shard_size.div_ceil(block_size).max(1);
    let options = StreamOptions::new().with_block_size(stream_case.block_size);

    let start = Instant::now();
    match operation {
        StreamOperation::Encode => {
            let data = make_data_shards(seed, case.data_shards, case.shard_size);
            for _ in 0..iterations {
                let mut readers: Vec<&[u8]> = data.iter().map(Vec::as_slice).collect();
                let mut writers = vec![Vec::with_capacity(case.shard_size); case.parity_shards];
                rs.encode_stream(&mut readers, &mut writers, &options)
                    .unwrap();
            }
        }
        StreamOperation::Verify => {
            let shards = make_encoded_shards(case, seed);
            for _ in 0..iterations {
                let mut readers: Vec<&[u8]> = shards.iter().map(Vec::as_slice).collect();
                assert!(rs.verify_stream(&mut readers, &options).unwrap());
            }
        }
        StreamOperation::Reconstruct => {
            let shards = make_encoded_shards(case, seed);
            for _ in 0..iterations {
                let mut cursors: Vec<Cursor<Vec<u8>>> =
                    shards.iter().cloned().map(Cursor::new).collect();
                cursors[0] = Cursor::new(Vec::new());
                if case.parity_shards > 0 {
                    cursors[case.data_shards] = Cursor::new(Vec::new());
                }
                rs.reconstruct_stream(&mut cursors, &options).unwrap();
                assert_eq!(cursors[0].get_ref(), &shards[0]);
            }
        }
    }

    let elapsed = start.elapsed();
    let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
    let ns_per_block = ns_per_iter / blocks_per_iter as f64;
    let throughput_mb_s =
        logical_data_bytes as f64 / (1024.0 * 1024.0) / (ns_per_iter / 1_000_000_000.0);

    StreamResult {
        operation: operation.as_str(),
        data_shards: case.data_shards,
        parity_shards: case.parity_shards,
        shard_size: case.shard_size,
        logical_data_bytes,
        case_label: case.label,
        seed,
        stream_block_size: block_size,
        stream_io_backend: "memory",
        stream_io_mode: "current_parallel",
        blocks_per_iter,
        throughput_mb_s,
        ns_per_iter,
        ns_per_block,
    }
}

fn write_results(results: &[StreamResult]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let revision = git_revision();
    let target = target_triple();
    let features = features();
    let backend = backend();
    let backend_id = backend_id();
    let backend_kind = backend_kind();
    let backend_override = backend_override();
    let override_honored = override_honored();
    let metrics_enabled = benchmark_metrics_enabled();
    let profile = stream_profile();
    let iterations = stream_iterations();

    let json_path = dir.join("stream-path-results.json");
    let csv_path = dir.join("stream-path-results.csv");

    let mut json = String::from("[\n");
    for (i, result) in results.iter().enumerate() {
        let suffix = if i + 1 == results.len() { "\n" } else { ",\n" };
        json.push_str(&format!(
            "  {{\"schema_version\":{},\"artifact_kind\":\"stream-path-results\",\"git_revision\":\"{}\",\"target_triple\":\"{}\",\"features\":\"{}\",\"benchmark_metrics_enabled\":{},\"backend\":\"{}\",\"backend_id\":\"{}\",\"backend_kind\":\"{}\",\"backend_override\":\"{}\",\"override_honored\":{},\"profile\":\"{}\",\"iterations\":{},\"operation\":\"{}\",\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"logical_data_bytes\":{},\"case_label\":\"{}\",\"seed\":{},\"stream_block_size\":{},\"stream_io_backend\":\"{}\",\"stream_io_mode\":\"{}\",\"blocks_per_iter\":{},\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2},\"ns_per_block\":{:.2}}}{}",
            ARTIFACT_SCHEMA_VERSION,
            revision,
            target,
            features,
            metrics_enabled,
            backend,
            backend_id,
            backend_kind,
            backend_override,
            override_honored,
            profile,
            iterations,
            result.operation,
            result.data_shards,
            result.parity_shards,
            result.shard_size,
            result.logical_data_bytes,
            result.case_label,
            result.seed,
            result.stream_block_size,
            result.stream_io_backend,
            result.stream_io_mode,
            result.blocks_per_iter,
            result.throughput_mb_s,
            result.ns_per_iter,
            result.ns_per_block,
            suffix
        ));
    }
    json.push(']');
    fs::write(&json_path, json).unwrap();

    let mut csv = String::from(
        "schema_version,artifact_kind,git_revision,target_triple,features,benchmark_metrics_enabled,backend,backend_id,backend_kind,backend_override,override_honored,profile,iterations,operation,data_shards,parity_shards,shard_size,logical_data_bytes,case_label,seed,stream_block_size,stream_io_backend,stream_io_mode,blocks_per_iter,throughput_mb_s,ns_per_iter,ns_per_block\n",
    );
    for result in results {
        csv.push_str(&format!(
            "{},stream-path-results,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.4},{:.2},{:.2}\n",
            ARTIFACT_SCHEMA_VERSION,
            revision,
            target,
            features,
            metrics_enabled,
            backend,
            backend_id,
            backend_kind,
            backend_override,
            override_honored,
            profile,
            iterations,
            result.operation,
            result.data_shards,
            result.parity_shards,
            result.shard_size,
            result.logical_data_bytes,
            result.case_label,
            result.seed,
            result.stream_block_size,
            result.stream_io_backend,
            result.stream_io_mode,
            result.blocks_per_iter,
            result.throughput_mb_s,
            result.ns_per_iter,
            result.ns_per_block
        ));
    }
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

#[test]
#[ignore]
fn benchmark_stream_path_matrix_runs_and_exports_results() {
    assert_backend_override_honored_if_strict();

    let iterations = stream_iterations();
    let cases = selected_stream_cases();
    let mut results = Vec::new();

    for case in cases {
        results.push(run_stream_operation(
            case,
            StreamOperation::Encode,
            iterations,
        ));
        results.push(run_stream_operation(
            case,
            StreamOperation::Verify,
            iterations,
        ));
        if stream_profile() != "quick" {
            results.push(run_stream_operation(
                case,
                StreamOperation::Reconstruct,
                iterations,
            ));
        }
    }

    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    assert!(results.iter().all(|result| result.ns_per_block.is_finite()));
    write_results(&results);
}
