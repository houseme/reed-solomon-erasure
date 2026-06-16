mod common;

use std::convert::TryInto;
use std::fs::{self, File};
use std::hint::black_box;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use rustfs_erasure_codec::galois_8::{ReedSolomon, RustNeonProfileStats, rust_neon_profile_stats};
use rustfs_erasure_codec::{
    CodecFamily, CodecOptions, ReconstructionCacheStats, RuntimeProfileStats,
};

use self::common::{BenchCase, Operation, SMOKE_CASES, case_name, derived_seed, make_full_shards};

const ARTIFACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
struct ProfileRecord {
    operation: &'static str,
    case_label: &'static str,
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    policy_min_parallel_shard_bytes: usize,
    policy_min_bytes_per_job: usize,
    policy_max_jobs: usize,
    runtime: RuntimeProfileStats,
    cache: ReconstructionCacheStats,
    neon: RustNeonProfileStats,
}

static PROFILE_RECORDS: Mutex<Vec<ProfileRecord>> = Mutex::new(Vec::new());

fn benchmark_metrics_enabled() -> bool {
    cfg!(feature = "benchmark-metrics")
}

fn push_profile_record(
    operation: &'static str,
    case: BenchCase,
    rs: &ReedSolomon,
    neon: RustNeonProfileStats,
) {
    let policy = rs.effective_parallel_policy();
    let record = ProfileRecord {
        operation,
        case_label: case.label,
        data_shards: case.data_shards,
        parity_shards: case.parity_shards,
        shard_size: case.shard_size,
        policy_min_parallel_shard_bytes: policy.min_parallel_shard_bytes,
        policy_min_bytes_per_job: policy.min_bytes_per_job,
        policy_max_jobs: policy.max_jobs,
        runtime: rs.runtime_profile_stats(),
        cache: rs.reconstruction_cache_stats(),
        neon,
    };
    PROFILE_RECORDS
        .lock()
        .expect("profile mutex poisoned")
        .push(record);
}

fn write_profile_report() {
    if std::env::var_os("RSE_WRITE_PROFILE_REPORT").is_none() {
        return;
    }
    let mut path = std::env::var_os("RSE_PROFILE_REPORT_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/benchmark-smoke/throughput-profile-report.json"));
    if path.is_relative() {
        path = std::env::current_dir().expect("cwd available").join(path);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create report directory");
    }

    let records = PROFILE_RECORDS.lock().expect("profile mutex poisoned");
    let mut file = File::create(path).expect("create profile report file");
    writeln!(file, "[").expect("write report header");
    for (idx, item) in records.iter().enumerate() {
        let comma = if idx + 1 == records.len() { "" } else { "," };
        writeln!(
            file,
            concat!(
                "  {{\"schema_version\":{},\"artifact_kind\":\"throughput-profile-report\",\"benchmark_metrics_enabled\":{},\"operation\":\"{}\",\"case\":\"{}\",\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},",
                "\"policy_min_parallel_shard_bytes\":{},\"policy_min_bytes_per_job\":{},\"policy_max_jobs\":{},",
                "\"code_some_serial_calls\":{},\"code_some_parallel_calls\":{},\"code_some_total_bytes\":{},\"code_some_total_chunks\":{},",
                "\"code_some_small_output_chunk_parallel_calls\":{},\"code_some_small_output_chunk_parallel_outputs\":{},\"code_some_small_output_chunk_parallel_chunks\":{},",
                "\"code_single_serial_calls\":{},\"code_single_parallel_calls\":{},\"code_single_total_bytes\":{},\"code_single_total_chunks\":{},",
                "\"parallel_policy_calls\":{},\"parallel_policy_parallel\":{},\"parallel_policy_serial\":{},",
                "\"parallel_policy_total_jobs\":{},\"parallel_policy_total_chunk_len\":{},",
                "\"reconstruct_calls\":{},\"reconstruct_data_only_calls\":{},\"reconstruct_total_missing_data\":{},\"reconstruct_total_missing_parity\":{},\"reconstruct_all_present_fast_path\":{},",
                "\"reconstruct_data_stage_calls\":{},\"reconstruct_data_stage_bytes\":{},\"reconstruct_parity_stage_calls\":{},\"reconstruct_parity_stage_bytes\":{},",
                "\"neon_mul_calls\":{},\"neon_mul_xor_calls\":{},\"neon_total_bytes\":{},\"neon_vector_64b_chunks\":{},\"neon_vector_16b_chunks\":{},\"neon_tail_bytes\":{},\"neon_tail_calls\":{},\"neon_table_lookups\":{},",
                "\"cache_requests\":{},\"cache_hits\":{},\"cache_misses\":{},\"cache_inserts\":{},\"cache_evictions\":{},",
                "\"cache_hit_rate\":{:.6},\"cache_reuse_ratio\":{:.6},\"cache_miss_cost_per_request\":{:.6}}}{}"
            ),
            ARTIFACT_SCHEMA_VERSION,
            benchmark_metrics_enabled(),
            item.operation,
            item.case_label,
            item.data_shards,
            item.parity_shards,
            item.shard_size,
            item.policy_min_parallel_shard_bytes,
            item.policy_min_bytes_per_job,
            item.policy_max_jobs,
            item.runtime.code_some_serial_calls,
            item.runtime.code_some_parallel_calls,
            item.runtime.code_some_total_bytes,
            item.runtime.code_some_total_chunks,
            item.runtime.code_some_small_output_chunk_parallel_calls,
            item.runtime.code_some_small_output_chunk_parallel_outputs,
            item.runtime.code_some_small_output_chunk_parallel_chunks,
            item.runtime.code_single_serial_calls,
            item.runtime.code_single_parallel_calls,
            item.runtime.code_single_total_bytes,
            item.runtime.code_single_total_chunks,
            item.runtime.parallel_policy_calls,
            item.runtime.parallel_policy_parallel,
            item.runtime.parallel_policy_serial,
            item.runtime.parallel_policy_total_jobs,
            item.runtime.parallel_policy_total_chunk_len,
            item.runtime.reconstruct_calls,
            item.runtime.reconstruct_data_only_calls,
            item.runtime.reconstruct_total_missing_data,
            item.runtime.reconstruct_total_missing_parity,
            item.runtime.reconstruct_all_present_fast_path,
            item.runtime.reconstruct_data_stage_calls,
            item.runtime.reconstruct_data_stage_bytes,
            item.runtime.reconstruct_parity_stage_calls,
            item.runtime.reconstruct_parity_stage_bytes,
            item.neon.mul_calls,
            item.neon.mul_xor_calls,
            item.neon.total_bytes,
            item.neon.vector_64b_chunks,
            item.neon.vector_16b_chunks,
            item.neon.tail_bytes,
            item.neon.tail_calls,
            item.neon.table_lookups,
            item.cache.requests,
            item.cache.hits,
            item.cache.misses,
            item.cache.inserts,
            item.cache.evictions,
            item.cache.hit_rate(),
            item.cache.reuse_ratio(),
            item.cache.miss_cost_per_request(),
            comma
        )
        .expect("write profile row");
    }
    writeln!(file, "]").expect("write report footer");
}

fn bench_encode(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::Encode, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::Encode, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();

    let mut group = c.benchmark_group("throughput_matrix_encode");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    rs.reset_runtime_profile_stats();
    let neon_before = rust_neon_profile_stats();
    group.bench_function(name, |b| {
        let mut shards =
            make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
        b.iter(|| {
            rs.encode_opt(black_box(&mut shards)).unwrap();
        });
    });
    group.finish();
    let neon_after = rust_neon_profile_stats();
    push_profile_record("encode", case, &rs, neon_after.saturating_sub(neon_before));
}

fn bench_leopard_setup(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::LeopardSetup, case);
    let throughput = case.shard_size * case.data_shards;

    let mut group = c.benchmark_group("throughput_matrix_leopard_setup");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    group.bench_function(name, |b| {
        b.iter(|| {
            let codec = ReedSolomon::with_options(
                case.data_shards,
                case.parity_shards,
                CodecOptions {
                    codec_family: CodecFamily::LeopardGF8,
                    ..CodecOptions::default()
                },
            )
            .unwrap();
            black_box(codec.leopard_setup_matrix_shape());
        });
    });
    group.finish();
}

fn bench_leopard_encode(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::LeopardEncode, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::LeopardEncode, case);
    let rs = ReedSolomon::with_options(
        case.data_shards,
        case.parity_shards,
        CodecOptions {
            codec_family: CodecFamily::LeopardGF8,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let mut group = c.benchmark_group("throughput_matrix_leopard_encode");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    group.bench_function(name, |b| {
        let mut shards =
            make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
        b.iter(|| {
            rs.encode_opt(black_box(&mut shards)).unwrap();
        });
    });
    group.finish();
}

fn bench_verify(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::Verify, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::Verify, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let mut shards = make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode_opt(&mut shards).unwrap();

    let mut group = c.benchmark_group("throughput_matrix_verify");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    rs.reset_runtime_profile_stats();
    let neon_before = rust_neon_profile_stats();
    group.bench_function(name, |b| {
        b.iter(|| {
            rs.verify_opt(black_box(&shards)).unwrap();
        });
    });
    group.finish();
    let neon_after = rust_neon_profile_stats();
    push_profile_record("verify", case, &rs, neon_after.saturating_sub(neon_before));
}

fn bench_update(c: &mut Criterion, case: BenchCase, changed_indices: &[usize], label: &str) {
    let name = format!("{}_{}_{}", Operation::Update.as_str(), case.label, label);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::Update, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let mut original =
        make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode_opt(&mut original).unwrap();

    let mut updated_data = original[..case.data_shards].to_vec();
    for &idx in changed_indices {
        if idx < updated_data.len() && !updated_data[idx].is_empty() {
            updated_data[idx][0] ^= 0x5a;
        }
    }

    let mut group = c.benchmark_group("throughput_matrix_update");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    rs.reset_runtime_profile_stats();
    let neon_before = rust_neon_profile_stats();
    group.bench_function(name, |b| {
        let old_data = original[..case.data_shards].to_vec();
        let old_refs = old_data.iter().collect::<Vec<_>>();
        let changes = (0..case.data_shards)
            .map(|idx| {
                if changed_indices.contains(&idx) {
                    Some(&updated_data[idx])
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        b.iter(|| {
            let mut parity = original[case.data_shards..].to_vec();
            let mut parity_refs = parity.iter_mut().collect::<Vec<_>>();
            rs.update(
                black_box(&old_refs),
                black_box(&changes),
                black_box(&mut parity_refs),
            )
            .unwrap();
        });
    });
    group.finish();
    let neon_after = rust_neon_profile_stats();
    push_profile_record("update", case, &rs, neon_after.saturating_sub(neon_before));
}

fn bench_reconstruct(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::Reconstruct, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::Reconstruct, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let mut original =
        make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode_opt(&mut original).unwrap();

    let mut group = c.benchmark_group("throughput_matrix_reconstruct");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    rs.reset_runtime_profile_stats();
    let neon_before = rust_neon_profile_stats();
    group.bench_function(name, |b| {
        b.iter(|| {
            let mut shards: Vec<Option<Vec<u8>>> = original.iter().cloned().map(Some).collect();
            shards[0] = None;
            shards[case.data_shards] = None;
            rs.reconstruct_opt(black_box(&mut shards)).unwrap();
        });
    });
    group.finish();
    let neon_after = rust_neon_profile_stats();
    push_profile_record(
        "reconstruct",
        case,
        &rs,
        neon_after.saturating_sub(neon_before),
    );
}

fn bench_reconstruct_data(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::ReconstructData, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::ReconstructData, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let mut original =
        make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode_opt(&mut original).unwrap();

    let mut group = c.benchmark_group("throughput_matrix_reconstruct_data");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    rs.reset_runtime_profile_stats();
    let neon_before = rust_neon_profile_stats();
    group.bench_function(name, |b| {
        b.iter(|| {
            let mut shards: Vec<Option<Vec<u8>>> = original.iter().cloned().map(Some).collect();
            shards[0] = None;
            shards[1] = None;
            rs.reconstruct_data_opt(black_box(&mut shards)).unwrap();
        });
    });
    group.finish();
    let neon_after = rust_neon_profile_stats();
    push_profile_record(
        "reconstruct_data",
        case,
        &rs,
        neon_after.saturating_sub(neon_before),
    );
}

fn smoke_matrix(c: &mut Criterion) {
    for case in SMOKE_CASES {
        bench_encode(c, *case);
        bench_update(c, *case, &[0], "1_change");
        if case.data_shards >= 2 {
            bench_update(c, *case, &[0, 1], "2_changes");
        }
        bench_verify(c, *case);
        bench_reconstruct(c, *case);
        bench_reconstruct_data(c, *case);
    }
    for case in SMOKE_CASES {
        if case.data_shards + case.parity_shards <= 256 {
            bench_leopard_setup(c, *case);
            bench_leopard_encode(c, *case);
        }
    }
    write_profile_report();
}

criterion_group!(throughput_matrix_benches, smoke_matrix);
criterion_main!(throughput_matrix_benches);
