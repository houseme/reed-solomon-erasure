#[path = "../benches/common/mod.rs"]
mod bench_common;
mod common;

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use reed_solomon_erasure::galois_8::ReedSolomon;
use reed_solomon_erasure::{CodecFamily, CodecOptions};

use self::bench_common::{
    ARTIFACT_SCHEMA_VERSION, BenchCase, FAST_SMOKE_CASES, Operation, QUICK_SMOKE_CASES,
    SMOKE_CASES, backend, backend_id, backend_kind, backend_override, benchmark_metrics_enabled,
    derived_seed, features, git_revision, make_full_shards, target_triple,
};
use self::common::{assert_backend_override_honored_if_strict, override_honored};

struct SmokeResult {
    operation: &'static str,
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    seed: u64,
    throughput_mb_s: f64,
    ns_per_iter: f64,
}

struct UpdateCompareResult {
    operation: &'static str,
    changed_shards: usize,
    throughput_mb_s: f64,
    ns_per_iter: f64,
    speedup_vs_encode: f64,
}

struct DecodeIdxCompareResult {
    operation: &'static str,
    throughput_mb_s: f64,
    ns_per_iter: f64,
    speedup_vs_reconstruct_some: f64,
}

struct LeopardSetupResult {
    operation: &'static str,
    throughput_mb_s: f64,
    ns_per_iter: f64,
    setup_rows: usize,
    setup_cols: usize,
}

struct LeopardEncodeResult {
    operation: &'static str,
    throughput_mb_s: f64,
    ns_per_iter: f64,
}

struct LeopardEncodeAbResult {
    variant: &'static str,
    throughput_mb_s: f64,
    ns_per_iter: f64,
}

struct LeopardEncodeProfileResult {
    throughput_mb_s: f64,
    ns_per_iter: f64,
    encode_calls: usize,
    encode_chunks: usize,
    encode_full_groups: usize,
    encode_remainder_groups: usize,
    encode_later_group_calls: usize,
    fft_stage_calls: usize,
    ifft_stage_calls: usize,
    first_group_ifft_calls: usize,
    later_group_ifft_calls: usize,
    remainder_group_ifft_calls: usize,
    first_group_input_copy_bytes: usize,
    later_group_input_copy_bytes: usize,
    remainder_group_input_copy_bytes: usize,
    first_group_zero_fill_bytes: usize,
    later_group_zero_fill_bytes: usize,
    remainder_group_zero_fill_bytes: usize,
    later_group_xor_bytes: usize,
    remainder_group_xor_bytes: usize,
    output_writeback_calls: usize,
    input_copy_bytes: usize,
    zero_fill_bytes: usize,
    xor_bytes: usize,
    output_writeback_bytes: usize,
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
            Operation::LeopardSetup => {
                let codec = ReedSolomon::with_options(
                    case.data_shards,
                    case.parity_shards,
                    CodecOptions {
                        codec_family: CodecFamily::LeopardGF8,
                        ..CodecOptions::default()
                    },
                )
                .unwrap();
                let _ = codec.leopard_setup_matrix_shape();
            }
            Operation::LeopardEncode => {
                let codec = ReedSolomon::with_options(
                    case.data_shards,
                    case.parity_shards,
                    CodecOptions {
                        codec_family: CodecFamily::LeopardGF8,
                        ..CodecOptions::default()
                    },
                )
                .unwrap();
                let mut shards =
                    make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
                codec.encode(&mut shards).unwrap();
            }
            Operation::Update => {
                let mut shards =
                    make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
                rs.encode(&mut shards).unwrap();
                let old_data = shards[..case.data_shards].to_vec();
                let mut parity = shards[case.data_shards..].to_vec();
                let mut updated = old_data.clone();
                if case.data_shards > 0 && case.shard_size > 0 {
                    updated[0][0] ^= 0x5a;
                }
                let old_refs = old_data.iter().collect::<Vec<_>>();
                let changes = (0..case.data_shards)
                    .map(|idx| if idx == 0 { Some(&updated[0]) } else { None })
                    .collect::<Vec<_>>();
                let mut parity_refs = parity.iter_mut().collect::<Vec<_>>();
                rs.update(&old_refs, &changes, &mut parity_refs).unwrap();
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

fn smoke_profile() -> &'static str {
    std::env::var("RSE_SMOKE_PROFILE")
        .ok()
        .as_deref()
        .map(|value| match value {
            "extended" => "extended",
            "fast" => "fast",
            "quick" => "quick",
            _ => "fast",
        })
        .unwrap_or("fast")
}

fn smoke_cases() -> &'static [BenchCase] {
    match smoke_profile() {
        "extended" => SMOKE_CASES,
        "fast" => FAST_SMOKE_CASES,
        _ => QUICK_SMOKE_CASES,
    }
}

fn smoke_iterations() -> usize {
    std::env::var("RSE_SMOKE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| match smoke_profile() {
            "extended" => 3,
            "fast" => 1,
            _ => 1,
        })
}

fn write_results(results: &[SmokeResult]) {
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
    let iterations = smoke_iterations();

    let json_path = dir.join("smoke-results.json");
    let csv_path = dir.join("smoke-results.csv");

    let mut json = String::from("[\n");
    for (i, result) in results.iter().enumerate() {
        let suffix = if i + 1 == results.len() { "\n" } else { ",\n" };
        json.push_str(&format!(
            "  {{\"schema_version\":{},\"artifact_kind\":\"smoke-results\",\"git_revision\":\"{}\",\"target_triple\":\"{}\",\"features\":\"{}\",\"benchmark_metrics_enabled\":{},\"backend\":\"{}\",\"backend_id\":\"{}\",\"backend_kind\":\"{}\",\"backend_override\":\"{}\",\"override_honored\":{},\"iterations\":{},\"operation\":\"{}\",\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"seed\":{},\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2}}}{}",
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
            iterations,
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
        "schema_version,artifact_kind,git_revision,target_triple,features,benchmark_metrics_enabled,backend,backend_id,backend_kind,backend_override,override_honored,iterations,operation,data_shards,parity_shards,shard_size,seed,throughput_mb_s,ns_per_iter\n",
    );
    for result in results {
        csv.push_str(&format!(
            "{},smoke-results,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.4},{:.2}\n",
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
            iterations,
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

fn run_update_compare(
    case: BenchCase,
    changed_shards: usize,
    iterations: usize,
) -> UpdateCompareResult {
    let seed = derived_seed(Operation::Update, case) ^ changed_shards as u64;
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let bytes = (case.shard_size * case.data_shards) as f64;

    let mut original =
        make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode(&mut original).unwrap();
    let old_data = original[..case.data_shards].to_vec();
    let old_refs = old_data.iter().collect::<Vec<_>>();
    let mut updated = old_data.clone();
    for idx in 0..changed_shards.min(case.data_shards) {
        if case.shard_size > 0 {
            updated[idx][0] ^= 0x5a;
        }
    }
    let changes = (0..case.data_shards)
        .map(|idx| {
            if idx < changed_shards {
                Some(&updated[idx])
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let encode_start = Instant::now();
    for _ in 0..iterations {
        let mut shards =
            make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
        rs.encode(&mut shards).unwrap();
    }
    let encode_elapsed = encode_start.elapsed();
    let encode_ns_per_iter = encode_elapsed.as_nanos() as f64 / iterations as f64;

    let update_start = Instant::now();
    for _ in 0..iterations {
        let mut parity = original[case.data_shards..].to_vec();
        let mut parity_refs = parity.iter_mut().collect::<Vec<_>>();
        rs.update(&old_refs, &changes, &mut parity_refs).unwrap();
    }
    let update_elapsed = update_start.elapsed();
    let update_ns_per_iter = update_elapsed.as_nanos() as f64 / iterations as f64;
    let throughput_mb_s = bytes / (1024.0 * 1024.0) / (update_ns_per_iter / 1_000_000_000.0);

    UpdateCompareResult {
        operation: "update",
        changed_shards,
        throughput_mb_s,
        ns_per_iter: update_ns_per_iter,
        speedup_vs_encode: encode_ns_per_iter / update_ns_per_iter,
    }
}

fn write_update_compare_results(case: BenchCase, results: &[UpdateCompareResult]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let stem = format!("update-vs-encode-{}", case.label);
    let json_path = dir.join(format!("{stem}.json"));
    let csv_path = dir.join(format!("{stem}.csv"));

    let mut json = String::from("[\n");
    for (i, result) in results.iter().enumerate() {
        let suffix = if i + 1 == results.len() { "\n" } else { ",\n" };
        json.push_str(&format!(
            "  {{\"schema_version\":{},\"artifact_kind\":\"update-vs-encode\",\"case\":\"{}\",\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"operation\":\"{}\",\"changed_shards\":{},\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2},\"speedup_vs_encode\":{:.4}}}{}",
            ARTIFACT_SCHEMA_VERSION,
            case.label,
            case.data_shards,
            case.parity_shards,
            case.shard_size,
            result.operation,
            result.changed_shards,
            result.throughput_mb_s,
            result.ns_per_iter,
            result.speedup_vs_encode,
            suffix
        ));
    }
    json.push(']');
    fs::write(&json_path, json).unwrap();

    let mut csv = String::from(
        "schema_version,artifact_kind,case,data_shards,parity_shards,shard_size,operation,changed_shards,throughput_mb_s,ns_per_iter,speedup_vs_encode\n",
    );
    for result in results {
        csv.push_str(&format!(
            "{},update-vs-encode,{},{},{},{},{},{},{:.4},{:.2},{:.4}\n",
            ARTIFACT_SCHEMA_VERSION,
            case.label,
            case.data_shards,
            case.parity_shards,
            case.shard_size,
            result.operation,
            result.changed_shards,
            result.throughput_mb_s,
            result.ns_per_iter,
            result.speedup_vs_encode
        ));
    }
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

fn run_decode_idx_compare(case: BenchCase, iterations: usize) -> DecodeIdxCompareResult {
    let seed = derived_seed(Operation::ReconstructData, case) ^ 0xD1u64;
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let useful_shards = 2usize;
    let bytes = (useful_shards * case.shard_size) as f64;

    let mut original =
        make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode(&mut original).unwrap();

    let missing = [0usize, 2usize];
    let mut required = vec![false; case.data_shards + case.parity_shards];
    required[missing[0]] = true;
    required[missing[1]] = true;

    let reconstruct_start = Instant::now();
    for _ in 0..iterations {
        let mut shards: Vec<Option<Vec<u8>>> = original.iter().cloned().map(Some).collect();
        shards[missing[0]] = None;
        shards[missing[1]] = None;
        rs.reconstruct_some(&mut shards, &required).unwrap();
    }
    let reconstruct_elapsed = reconstruct_start.elapsed();
    let reconstruct_ns_per_iter = reconstruct_elapsed.as_nanos() as f64 / iterations as f64;

    let expect_input = {
        let mut flags = vec![false; case.data_shards + case.parity_shards];
        for idx in 0..case.data_shards {
            if idx != missing[0] && idx != missing[1] {
                flags[idx] = true;
            }
        }
        let mut extras = 0usize;
        for idx in case.data_shards..(case.data_shards + case.parity_shards) {
            if extras < 2 {
                flags[idx] = true;
                extras += 1;
            }
        }
        flags
    };

    let decode_start = Instant::now();
    for _ in 0..iterations {
        let mut dst = vec![None; case.data_shards + case.parity_shards];
        dst[missing[0]] = Some(vec![0u8; case.shard_size]);
        dst[missing[1]] = Some(vec![0u8; case.shard_size]);

        let mut first_input = vec![None; case.data_shards + case.parity_shards];
        first_input[1] = Some(original[1].clone());
        first_input[3] = Some(original[3].clone());
        first_input[case.data_shards] = Some(original[case.data_shards].clone());
        rs.decode_idx(&mut dst, Some(&expect_input), &first_input)
            .unwrap();

        let mut second_input = vec![None; case.data_shards + case.parity_shards];
        for idx in 4..case.data_shards {
            second_input[idx] = Some(original[idx].clone());
        }
        second_input[case.data_shards + 1] = Some(original[case.data_shards + 1].clone());
        rs.decode_idx(&mut dst, Some(&expect_input), &second_input)
            .unwrap();
    }
    let decode_elapsed = decode_start.elapsed();
    let decode_ns_per_iter = decode_elapsed.as_nanos() as f64 / iterations as f64;
    let throughput_mb_s = bytes / (1024.0 * 1024.0) / (decode_ns_per_iter / 1_000_000_000.0);

    DecodeIdxCompareResult {
        operation: "decode_idx",
        throughput_mb_s,
        ns_per_iter: decode_ns_per_iter,
        speedup_vs_reconstruct_some: reconstruct_ns_per_iter / decode_ns_per_iter,
    }
}

fn write_decode_idx_compare_results(case: BenchCase, result: &DecodeIdxCompareResult) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let stem = format!("decode-idx-vs-reconstruct-some-{}", case.label);
    let json_path = dir.join(format!("{stem}.json"));
    let csv_path = dir.join(format!("{stem}.csv"));

    let json = format!(
        concat!(
            "{{\"schema_version\":{},\"artifact_kind\":\"decode-idx-vs-reconstruct-some\",\"case\":\"{}\",",
            "\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"operation\":\"{}\",",
            "\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2},\"speedup_vs_reconstruct_some\":{:.4}}}"
        ),
        ARTIFACT_SCHEMA_VERSION,
        case.label,
        case.data_shards,
        case.parity_shards,
        case.shard_size,
        result.operation,
        result.throughput_mb_s,
        result.ns_per_iter,
        result.speedup_vs_reconstruct_some
    );
    fs::write(&json_path, json).unwrap();

    let csv = format!(
        concat!(
            "schema_version,artifact_kind,case,data_shards,parity_shards,shard_size,operation,throughput_mb_s,ns_per_iter,speedup_vs_reconstruct_some\n",
            "{},decode-idx-vs-reconstruct-some,{},{},{},{},{},{:.4},{:.2},{:.4}\n"
        ),
        ARTIFACT_SCHEMA_VERSION,
        case.label,
        case.data_shards,
        case.parity_shards,
        case.shard_size,
        result.operation,
        result.throughput_mb_s,
        result.ns_per_iter,
        result.speedup_vs_reconstruct_some
    );
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

fn run_leopard_setup(case: BenchCase, iterations: usize) -> LeopardSetupResult {
    let bytes = (case.shard_size * case.data_shards) as f64;

    let start = Instant::now();
    let mut shape = (0usize, 0usize);
    for _ in 0..iterations {
        let codec = ReedSolomon::with_options(
            case.data_shards,
            case.parity_shards,
            CodecOptions {
                codec_family: CodecFamily::LeopardGF8,
                ..CodecOptions::default()
            },
        )
        .unwrap();
        shape = codec.leopard_setup_matrix_shape().unwrap_or((0, 0));
    }
    let elapsed = start.elapsed();
    let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
    let throughput_mb_s = bytes / (1024.0 * 1024.0) / (ns_per_iter / 1_000_000_000.0);

    LeopardSetupResult {
        operation: "leopard_setup",
        throughput_mb_s,
        ns_per_iter,
        setup_rows: shape.0,
        setup_cols: shape.1,
    }
}

fn write_leopard_setup_results(case: BenchCase, result: &LeopardSetupResult) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let stem = format!("leopard-setup-{}", case.label);
    let json_path = dir.join(format!("{stem}.json"));
    let csv_path = dir.join(format!("{stem}.csv"));

    let json = format!(
        concat!(
            "{{\"schema_version\":{},\"artifact_kind\":\"leopard-setup\",\"case\":\"{}\",",
            "\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"operation\":\"{}\",",
            "\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2},\"setup_rows\":{},\"setup_cols\":{}}}"
        ),
        ARTIFACT_SCHEMA_VERSION,
        case.label,
        case.data_shards,
        case.parity_shards,
        case.shard_size,
        result.operation,
        result.throughput_mb_s,
        result.ns_per_iter,
        result.setup_rows,
        result.setup_cols
    );
    fs::write(&json_path, json).unwrap();

    let csv = format!(
        concat!(
            "schema_version,artifact_kind,case,data_shards,parity_shards,shard_size,operation,throughput_mb_s,ns_per_iter,setup_rows,setup_cols\n",
            "{},leopard-setup,{},{},{},{},{},{:.4},{:.2},{},{}\n"
        ),
        ARTIFACT_SCHEMA_VERSION,
        case.label,
        case.data_shards,
        case.parity_shards,
        case.shard_size,
        result.operation,
        result.throughput_mb_s,
        result.ns_per_iter,
        result.setup_rows,
        result.setup_cols
    );
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

fn run_leopard_encode(case: BenchCase, iterations: usize) -> LeopardEncodeResult {
    let seed = derived_seed(Operation::LeopardEncode, case);
    let bytes = (case.shard_size * case.data_shards) as f64;
    let codec = ReedSolomon::with_options(
        case.data_shards,
        case.parity_shards,
        CodecOptions {
            codec_family: CodecFamily::LeopardGF8,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let start = Instant::now();
    for _ in 0..iterations {
        let mut shards =
            make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
        codec.encode_opt(&mut shards).unwrap();
    }
    let elapsed = start.elapsed();
    let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
    let throughput_mb_s = bytes / (1024.0 * 1024.0) / (ns_per_iter / 1_000_000_000.0);

    LeopardEncodeResult {
        operation: "leopard_encode",
        throughput_mb_s,
        ns_per_iter,
    }
}

fn run_leopard_encode_profile(case: BenchCase, iterations: usize) -> LeopardEncodeProfileResult {
    let seed = derived_seed(Operation::LeopardEncode, case);
    let bytes = (case.shard_size * case.data_shards) as f64;
    let codec = ReedSolomon::with_options(
        case.data_shards,
        case.parity_shards,
        CodecOptions {
            codec_family: CodecFamily::LeopardGF8,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    reed_solomon_erasure::reset_leopard_gf8_profile_stats();
    let start = Instant::now();
    for _ in 0..iterations {
        let mut shards =
            make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
        codec.encode_opt(&mut shards).unwrap();
    }
    let elapsed = start.elapsed();
    let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
    let throughput_mb_s = bytes / (1024.0 * 1024.0) / (ns_per_iter / 1_000_000_000.0);
    let stats = reed_solomon_erasure::leopard_gf8_profile_stats();

    LeopardEncodeProfileResult {
        throughput_mb_s,
        ns_per_iter,
        encode_calls: stats.encode_calls,
        encode_chunks: stats.encode_chunks,
        encode_full_groups: stats.encode_full_groups,
        encode_remainder_groups: stats.encode_remainder_groups,
        encode_later_group_calls: stats.encode_later_group_calls,
        fft_stage_calls: stats.fft_stage_calls,
        ifft_stage_calls: stats.ifft_stage_calls,
        first_group_ifft_calls: stats.first_group_ifft_calls,
        later_group_ifft_calls: stats.later_group_ifft_calls,
        remainder_group_ifft_calls: stats.remainder_group_ifft_calls,
        first_group_input_copy_bytes: stats.first_group_input_copy_bytes,
        later_group_input_copy_bytes: stats.later_group_input_copy_bytes,
        remainder_group_input_copy_bytes: stats.remainder_group_input_copy_bytes,
        first_group_zero_fill_bytes: stats.first_group_zero_fill_bytes,
        later_group_zero_fill_bytes: stats.later_group_zero_fill_bytes,
        remainder_group_zero_fill_bytes: stats.remainder_group_zero_fill_bytes,
        later_group_xor_bytes: stats.later_group_xor_bytes,
        remainder_group_xor_bytes: stats.remainder_group_xor_bytes,
        output_writeback_calls: stats.output_writeback_calls,
        input_copy_bytes: stats.input_copy_bytes,
        zero_fill_bytes: stats.zero_fill_bytes,
        xor_bytes: stats.xor_bytes,
        output_writeback_bytes: stats.output_writeback_bytes,
    }
}

fn write_leopard_encode_results(case: BenchCase, result: &LeopardEncodeResult) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let stem = format!("leopard-encode-{}", case.label);
    let json_path = dir.join(format!("{stem}.json"));
    let csv_path = dir.join(format!("{stem}.csv"));

    let json = format!(
        concat!(
            "{{\"schema_version\":{},\"artifact_kind\":\"leopard-encode\",\"case\":\"{}\",",
            "\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"operation\":\"{}\",",
            "\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2}}}"
        ),
        ARTIFACT_SCHEMA_VERSION,
        case.label,
        case.data_shards,
        case.parity_shards,
        case.shard_size,
        result.operation,
        result.throughput_mb_s,
        result.ns_per_iter
    );
    fs::write(&json_path, json).unwrap();

    let csv = format!(
        concat!(
            "schema_version,artifact_kind,case,data_shards,parity_shards,shard_size,operation,throughput_mb_s,ns_per_iter\n",
            "{},leopard-encode,{},{},{},{},{},{:.4},{:.2}\n"
        ),
        ARTIFACT_SCHEMA_VERSION,
        case.label,
        case.data_shards,
        case.parity_shards,
        case.shard_size,
        result.operation,
        result.throughput_mb_s,
        result.ns_per_iter
    );
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

fn write_leopard_encode_profile_result(case: BenchCase, result: &LeopardEncodeProfileResult) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let stem = format!("leopard-encode-profile-{}", case.label);
    let json_path = dir.join(format!("{stem}.json"));
    let csv_path = dir.join(format!("{stem}.csv"));

    let json = format!(
        concat!(
            "{{\"schema_version\":{},\"artifact_kind\":\"leopard-encode-profile\",\"case\":\"{}\",",
            "\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},",
            "\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2},",
            "\"encode_calls\":{},\"encode_chunks\":{},\"encode_full_groups\":{},\"encode_remainder_groups\":{},",
            "\"encode_later_group_calls\":{},\"fft_stage_calls\":{},\"ifft_stage_calls\":{},",
            "\"first_group_ifft_calls\":{},\"later_group_ifft_calls\":{},\"remainder_group_ifft_calls\":{},",
            "\"first_group_input_copy_bytes\":{},\"later_group_input_copy_bytes\":{},\"remainder_group_input_copy_bytes\":{},",
            "\"first_group_zero_fill_bytes\":{},\"later_group_zero_fill_bytes\":{},\"remainder_group_zero_fill_bytes\":{},",
            "\"later_group_xor_bytes\":{},\"remainder_group_xor_bytes\":{},\"output_writeback_calls\":{},",
            "\"input_copy_bytes\":{},\"zero_fill_bytes\":{},\"xor_bytes\":{},\"output_writeback_bytes\":{}}}"
        ),
        ARTIFACT_SCHEMA_VERSION,
        case.label,
        case.data_shards,
        case.parity_shards,
        case.shard_size,
        result.throughput_mb_s,
        result.ns_per_iter,
        result.encode_calls,
        result.encode_chunks,
        result.encode_full_groups,
        result.encode_remainder_groups,
        result.encode_later_group_calls,
        result.fft_stage_calls,
        result.ifft_stage_calls,
        result.first_group_ifft_calls,
        result.later_group_ifft_calls,
        result.remainder_group_ifft_calls,
        result.first_group_input_copy_bytes,
        result.later_group_input_copy_bytes,
        result.remainder_group_input_copy_bytes,
        result.first_group_zero_fill_bytes,
        result.later_group_zero_fill_bytes,
        result.remainder_group_zero_fill_bytes,
        result.later_group_xor_bytes,
        result.remainder_group_xor_bytes,
        result.output_writeback_calls,
        result.input_copy_bytes,
        result.zero_fill_bytes,
        result.xor_bytes,
        result.output_writeback_bytes
    );
    fs::write(&json_path, json).unwrap();

    let csv = [
        "schema_version,artifact_kind,case,data_shards,parity_shards,shard_size,throughput_mb_s,ns_per_iter,encode_calls,encode_chunks,encode_full_groups,encode_remainder_groups,encode_later_group_calls,fft_stage_calls,ifft_stage_calls,first_group_ifft_calls,later_group_ifft_calls,remainder_group_ifft_calls,first_group_input_copy_bytes,later_group_input_copy_bytes,remainder_group_input_copy_bytes,first_group_zero_fill_bytes,later_group_zero_fill_bytes,remainder_group_zero_fill_bytes,later_group_xor_bytes,remainder_group_xor_bytes,output_writeback_calls,input_copy_bytes,zero_fill_bytes,xor_bytes,output_writeback_bytes".to_string(),
        vec![
            ARTIFACT_SCHEMA_VERSION.to_string(),
            "leopard-encode-profile".to_string(),
            case.label.to_string(),
            case.data_shards.to_string(),
            case.parity_shards.to_string(),
            case.shard_size.to_string(),
            format!("{:.4}", result.throughput_mb_s),
            format!("{:.2}", result.ns_per_iter),
            result.encode_calls.to_string(),
            result.encode_chunks.to_string(),
            result.encode_full_groups.to_string(),
            result.encode_remainder_groups.to_string(),
            result.encode_later_group_calls.to_string(),
            result.fft_stage_calls.to_string(),
            result.ifft_stage_calls.to_string(),
            result.first_group_ifft_calls.to_string(),
            result.later_group_ifft_calls.to_string(),
            result.remainder_group_ifft_calls.to_string(),
            result.first_group_input_copy_bytes.to_string(),
            result.later_group_input_copy_bytes.to_string(),
            result.remainder_group_input_copy_bytes.to_string(),
            result.first_group_zero_fill_bytes.to_string(),
            result.later_group_zero_fill_bytes.to_string(),
            result.remainder_group_zero_fill_bytes.to_string(),
            result.later_group_xor_bytes.to_string(),
            result.remainder_group_xor_bytes.to_string(),
            result.output_writeback_calls.to_string(),
            result.input_copy_bytes.to_string(),
            result.zero_fill_bytes.to_string(),
            result.xor_bytes.to_string(),
            result.output_writeback_bytes.to_string(),
        ]
        .join(","),
    ]
    .join("\n")
        + "\n";
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

fn with_leopard_envs<R>(
    reuse_zero: bool,
    forward_tables: bool,
    xor_clone: bool,
    f: impl FnOnce() -> R,
) -> R {
    unsafe {
        if reuse_zero {
            std::env::set_var("RSE_LEOPARD_GF8_REUSE_ZERO", "1");
        } else {
            std::env::remove_var("RSE_LEOPARD_GF8_REUSE_ZERO");
        }
        if forward_tables {
            std::env::set_var("RSE_LEOPARD_GF8_FORWARD_TABLES", "1");
        } else {
            std::env::remove_var("RSE_LEOPARD_GF8_FORWARD_TABLES");
        }
        if xor_clone {
            std::env::set_var("RSE_LEOPARD_GF8_XOR_CLONE", "1");
        } else {
            std::env::remove_var("RSE_LEOPARD_GF8_XOR_CLONE");
        }
    }
    let result = f();
    unsafe {
        std::env::remove_var("RSE_LEOPARD_GF8_REUSE_ZERO");
        std::env::remove_var("RSE_LEOPARD_GF8_FORWARD_TABLES");
        std::env::remove_var("RSE_LEOPARD_GF8_XOR_CLONE");
    }
    result
}

fn run_leopard_encode_ab_variant(
    case: BenchCase,
    iterations: usize,
    variant: &'static str,
    reuse_zero: bool,
    forward_tables: bool,
    xor_clone: bool,
) -> LeopardEncodeAbResult {
    let seed = derived_seed(Operation::LeopardEncode, case);
    let bytes = (case.shard_size * case.data_shards) as f64;
    let codec = ReedSolomon::with_options(
        case.data_shards,
        case.parity_shards,
        CodecOptions {
            codec_family: CodecFamily::LeopardGF8,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let elapsed = with_leopard_envs(reuse_zero, forward_tables, xor_clone, || {
        let start = Instant::now();
        for _ in 0..iterations {
            let mut shards =
                make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
            codec.encode_opt(&mut shards).unwrap();
        }
        start.elapsed()
    });

    let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
    let throughput_mb_s = bytes / (1024.0 * 1024.0) / (ns_per_iter / 1_000_000_000.0);

    LeopardEncodeAbResult {
        variant,
        throughput_mb_s,
        ns_per_iter,
    }
}

fn write_leopard_encode_ab_results(case: BenchCase, results: &[LeopardEncodeAbResult]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let stem = format!("leopard-encode-ab-{}", case.label);
    let json_path = dir.join(format!("{stem}.json"));
    let csv_path = dir.join(format!("{stem}.csv"));

    let mut json = String::from("[\n");
    for (i, result) in results.iter().enumerate() {
        let suffix = if i + 1 == results.len() { "\n" } else { ",\n" };
        json.push_str(&format!(
            "  {{\"schema_version\":{},\"artifact_kind\":\"leopard-encode-ab\",\"case\":\"{}\",\"variant\":\"{}\",\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"throughput_mb_s\":{:.4},\"ns_per_iter\":{:.2}}}{}",
            ARTIFACT_SCHEMA_VERSION,
            case.label,
            result.variant,
            case.data_shards,
            case.parity_shards,
            case.shard_size,
            result.throughput_mb_s,
            result.ns_per_iter,
            suffix
        ));
    }
    json.push(']');
    fs::write(&json_path, json).unwrap();

    let mut csv = String::from(
        "schema_version,artifact_kind,case,variant,data_shards,parity_shards,shard_size,throughput_mb_s,ns_per_iter\n",
    );
    for result in results {
        csv.push_str(&format!(
            "{},leopard-encode-ab,{},{},{},{},{},{:.4},{:.2}\n",
            ARTIFACT_SCHEMA_VERSION,
            case.label,
            result.variant,
            case.data_shards,
            case.parity_shards,
            case.shard_size,
            result.throughput_mb_s,
            result.ns_per_iter
        ));
    }
    fs::write(&csv_path, csv).unwrap();
}

#[test]
fn benchmark_smoke_matrix_runs_and_exports_results() {
    assert_backend_override_honored_if_strict();
    let mut results = Vec::new();
    let iterations = smoke_iterations();
    for case in smoke_cases() {
        results.push(run_operation(*case, Operation::Encode, iterations));
        results.push(run_operation(*case, Operation::Update, iterations));
        results.push(run_operation(*case, Operation::Verify, iterations));
        results.push(run_operation(*case, Operation::Reconstruct, iterations));
        results.push(run_operation(*case, Operation::ReconstructData, iterations));
    }

    assert!(!results.is_empty());
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    write_results(&results);
}

#[test]
fn benchmark_update_vs_encode_10x4_1m_exports_results() {
    let case = FAST_SMOKE_CASES
        .iter()
        .copied()
        .find(|case| case.label == "10x4_1m")
        .expect("10x4_1m smoke case must exist");
    let iterations = smoke_iterations().max(2);
    let results = vec![
        run_update_compare(case, 1, iterations),
        run_update_compare(case, 2, iterations),
        run_update_compare(case, 3, iterations),
        run_update_compare(case, 4, iterations),
    ];
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    assert!(
        results
            .iter()
            .all(|result| result.speedup_vs_encode.is_finite())
    );
    write_update_compare_results(case, &results);
}

#[test]
fn benchmark_update_vs_encode_4x2_64k_exports_results() {
    let case = FAST_SMOKE_CASES
        .iter()
        .copied()
        .find(|case| case.label == "4x2_64k")
        .expect("4x2_64k smoke case must exist");
    let iterations = smoke_iterations().max(2);
    let results = vec![
        run_update_compare(case, 1, iterations),
        run_update_compare(case, 2, iterations),
        run_update_compare(case, 3, iterations),
        run_update_compare(case, 4, iterations),
    ];
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    assert!(
        results
            .iter()
            .all(|result| result.speedup_vs_encode.is_finite())
    );
    write_update_compare_results(case, &results);
}

#[test]
fn benchmark_update_vs_encode_32x16_1m_exports_results() {
    let case = SMOKE_CASES
        .iter()
        .copied()
        .find(|case| case.label == "32x16_1m")
        .expect("32x16_1m smoke case must exist");
    let iterations = smoke_iterations().max(2);
    let results = vec![
        run_update_compare(case, 1, iterations),
        run_update_compare(case, 2, iterations),
        run_update_compare(case, 3, iterations),
        run_update_compare(case, 4, iterations),
    ];
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    assert!(
        results
            .iter()
            .all(|result| result.speedup_vs_encode.is_finite())
    );
    write_update_compare_results(case, &results);
}

#[test]
fn benchmark_update_vs_encode_4x2_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "4x2_4m")
        .expect("4x2_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let results = vec![
        run_update_compare(case, 1, iterations),
        run_update_compare(case, 2, iterations),
        run_update_compare(case, 3, iterations),
        run_update_compare(case, 4, iterations),
    ];
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    assert!(
        results
            .iter()
            .all(|result| result.speedup_vs_encode.is_finite())
    );
    write_update_compare_results(case, &results);
}

#[test]
fn benchmark_update_vs_encode_10x4_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "10x4_4m")
        .expect("10x4_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let results = vec![
        run_update_compare(case, 1, iterations),
        run_update_compare(case, 2, iterations),
        run_update_compare(case, 3, iterations),
        run_update_compare(case, 4, iterations),
    ];
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    assert!(
        results
            .iter()
            .all(|result| result.speedup_vs_encode.is_finite())
    );
    write_update_compare_results(case, &results);
}

#[test]
fn benchmark_update_vs_encode_32x16_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "32x16_4m")
        .expect("32x16_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let results = vec![
        run_update_compare(case, 1, iterations),
        run_update_compare(case, 2, iterations),
        run_update_compare(case, 3, iterations),
        run_update_compare(case, 4, iterations),
    ];
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    assert!(
        results
            .iter()
            .all(|result| result.speedup_vs_encode.is_finite())
    );
    write_update_compare_results(case, &results);
}

#[test]
fn benchmark_decode_idx_vs_reconstruct_some_10x4_1m_exports_results() {
    let case = FAST_SMOKE_CASES
        .iter()
        .copied()
        .find(|case| case.label == "10x4_1m")
        .expect("10x4_1m smoke case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_decode_idx_compare(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    assert!(result.speedup_vs_reconstruct_some.is_finite());
    write_decode_idx_compare_results(case, &result);
}

#[test]
fn benchmark_decode_idx_vs_reconstruct_some_4x2_64k_exports_results() {
    let case = FAST_SMOKE_CASES
        .iter()
        .copied()
        .find(|case| case.label == "4x2_64k")
        .expect("4x2_64k smoke case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_decode_idx_compare(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    assert!(result.speedup_vs_reconstruct_some.is_finite());
    write_decode_idx_compare_results(case, &result);
}

#[test]
fn benchmark_decode_idx_vs_reconstruct_some_32x16_1m_exports_results() {
    let case = SMOKE_CASES
        .iter()
        .copied()
        .find(|case| case.label == "32x16_1m")
        .expect("32x16_1m smoke case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_decode_idx_compare(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    assert!(result.speedup_vs_reconstruct_some.is_finite());
    write_decode_idx_compare_results(case, &result);
}

#[test]
fn benchmark_decode_idx_vs_reconstruct_some_4x2_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "4x2_4m")
        .expect("4x2_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_decode_idx_compare(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    assert!(result.speedup_vs_reconstruct_some.is_finite());
    write_decode_idx_compare_results(case, &result);
}

#[test]
fn benchmark_decode_idx_vs_reconstruct_some_32x16_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "32x16_4m")
        .expect("32x16_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_decode_idx_compare(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    assert!(result.speedup_vs_reconstruct_some.is_finite());
    write_decode_idx_compare_results(case, &result);
}

#[test]
fn benchmark_leopard_setup_32x16_1m_exports_results() {
    let case = SMOKE_CASES
        .iter()
        .copied()
        .find(|case| case.label == "32x16_1m")
        .expect("32x16_1m smoke case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_setup(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    assert_eq!(48, result.setup_rows);
    assert_eq!(32, result.setup_cols);
    write_leopard_setup_results(case, &result);
}

#[test]
fn benchmark_leopard_setup_64x32_1m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "64x32_1m")
        .expect("64x32_1m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_setup(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    assert_eq!(96, result.setup_rows);
    assert_eq!(64, result.setup_cols);
    write_leopard_setup_results(case, &result);
}

#[test]
fn benchmark_leopard_setup_64x32_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "64x32_4m")
        .expect("64x32_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_setup(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    assert_eq!(96, result.setup_rows);
    assert_eq!(64, result.setup_cols);
    write_leopard_setup_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_64x32_1m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "64x32_1m")
        .expect("64x32_1m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_32x16_1m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "32x16_1m")
        .expect("32x16_1m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_32x16_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "32x16_4m")
        .expect("32x16_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_64x32_64k_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "64x32_64k")
        .expect("64x32_64k full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_64x32_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "64x32_4m")
        .expect("64x32_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_96x48_1m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "96x48_1m")
        .expect("96x48_1m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_profile_96x48_1m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "96x48_1m")
        .expect("96x48_1m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode_profile(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_profile_result(case, &result);
}

#[test]
fn benchmark_leopard_encode_96x48_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "96x48_4m")
        .expect("96x48_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_128x64_1m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "128x64_1m")
        .expect("128x64_1m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_profile_128x64_1m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "128x64_1m")
        .expect("128x64_1m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode_profile(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_profile_result(case, &result);
}

#[test]
fn benchmark_leopard_encode_128x64_4m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "128x64_4m")
        .expect("128x64_4m full case must exist");
    let iterations = smoke_iterations().max(2);
    let result = run_leopard_encode(case, iterations);
    assert!(result.throughput_mb_s.is_finite());
    write_leopard_encode_results(case, &result);
}

#[test]
fn benchmark_leopard_encode_ab_64x32_1m_exports_results() {
    let case = bench_common::FULL_CASES
        .iter()
        .copied()
        .find(|case| case.label == "64x32_1m")
        .expect("64x32_1m full case must exist");
    let iterations = smoke_iterations().max(2);
    let results = vec![
        run_leopard_encode_ab_variant(case, iterations, "baseline", false, false, false),
        run_leopard_encode_ab_variant(case, iterations, "reuse_zero_only", true, false, false),
        run_leopard_encode_ab_variant(case, iterations, "xor_clone_only", false, false, true),
    ];
    assert!(
        results
            .iter()
            .all(|result| result.throughput_mb_s.is_finite())
    );
    write_leopard_encode_ab_results(case, &results);
}

#[cfg(all(
    feature = "simd-accel",
    feature = "std",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[test]
fn benchmark_smoke_metadata_tracks_aarch64_scalar_and_neon_overrides() {
    use std::process::Command;

    // Child-process dispatch: each override is validated in a fresh process so
    // that the `Once`-cached ACTIVE_BACKEND is not poisoned by earlier tests.
    match std::env::var("RSE_BENCHMARK_SMOKE_CHILD_CHECK").as_deref() {
        Ok("aarch64-scalar-override") => {
            // Fresh process: RSE_BACKEND_OVERRIDE is already "scalar".
            assert_eq!("scalar-rust", backend());
            assert_eq!("ScalarRust", backend_id());
            assert_eq!("Scalar", backend_kind());
            assert!(override_honored());
            return;
        }
        Ok("aarch64-neon-override") => {
            // Fresh process: RSE_BACKEND_OVERRIDE is already "rust-neon".
            assert_eq!("rust-neon", backend());
            assert_eq!("RustNeon", backend_id());
            assert_eq!("RustSimd", backend_kind());
            assert!(override_honored());
            return;
        }
        _ => {}
    }

    let current_exe = std::env::current_exe().unwrap();

    // Validate scalar override in a child process (fresh `Once`).
    let scalar_output = Command::new(&current_exe)
        .env("RSE_BACKEND_OVERRIDE", "scalar")
        .env("RSE_STRICT_BACKEND_OVERRIDE", "1")
        .env("RSE_BENCHMARK_SMOKE_CHILD_CHECK", "aarch64-scalar-override")
        .arg("--exact")
        .arg("benchmark_smoke_metadata_tracks_aarch64_scalar_and_neon_overrides")
        .arg("--nocapture")
        .output()
        .unwrap();
    assert!(
        scalar_output.status.success(),
        "scalar override check failed: stdout={} stderr={}",
        String::from_utf8_lossy(&scalar_output.stdout),
        String::from_utf8_lossy(&scalar_output.stderr)
    );

    // Validate rust-neon override in a child process (fresh `Once`).
    let neon_output = Command::new(&current_exe)
        .env("RSE_BACKEND_OVERRIDE", "rust-neon")
        .env("RSE_STRICT_BACKEND_OVERRIDE", "1")
        .env("RSE_BENCHMARK_SMOKE_CHILD_CHECK", "aarch64-neon-override")
        .arg("--exact")
        .arg("benchmark_smoke_metadata_tracks_aarch64_scalar_and_neon_overrides")
        .arg("--nocapture")
        .output()
        .unwrap();
    assert!(
        neon_output.status.success(),
        "neon override check failed: stdout={} stderr={}",
        String::from_utf8_lossy(&neon_output.stdout),
        String::from_utf8_lossy(&neon_output.stderr)
    );

    // Also verify the env-var reads work in-process (the actual backend is
    // already cached by `Once`, so we only check `backend_override()`).
    unsafe {
        std::env::set_var("RSE_BACKEND_OVERRIDE", "scalar");
    }
    assert_eq!("scalar", backend_override());
    unsafe {
        std::env::set_var("RSE_BACKEND_OVERRIDE", "rust-neon");
    }
    assert_eq!("rust-neon", backend_override());
    unsafe {
        std::env::remove_var("RSE_BACKEND_OVERRIDE");
        std::env::remove_var("RSE_STRICT_BACKEND_OVERRIDE");
    }
}
