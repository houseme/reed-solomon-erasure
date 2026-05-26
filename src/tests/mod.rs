#![allow(dead_code)]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use super::{CodecOptions, Error, MatrixMode, SBSError, galois_8};
#[cfg(feature = "std")]
use super::{ParallelDecision, ParallelPolicy};
use rand::{self, RngExt, rng};

#[cfg(feature = "std")]
use std::fs;
#[cfg(feature = "std")]
use std::path::PathBuf;
#[cfg(feature = "std")]
use std::time::Instant;

mod galois_16;

type ReedSolomon = crate::ReedSolomon<galois_8::Field>;
type ShardByShard<'a> = crate::ShardByShard<'a, galois_8::Field>;

macro_rules! make_random_shards {
    ($per_shard:expr, $size:expr) => {{
        let mut shards = Vec::with_capacity(20);
        for _ in 0..$size {
            shards.push(vec![0; $per_shard]);
        }

        for s in shards.iter_mut() {
            fill_random(s);
        }

        shards
    }};
}

fn assert_eq_shards<T, U>(s1: &[T], s2: &[U])
where
    T: AsRef<[u8]>,
    U: AsRef<[u8]>,
{
    assert_eq!(s1.len(), s2.len());
    for i in 0..s1.len() {
        assert_eq!(s1[i].as_ref(), s2[i].as_ref());
    }
}

pub fn fill_random<T>(arr: &mut [T])
where
    rand::distr::StandardUniform: rand::distr::Distribution<T>,
{
    for a in arr.iter_mut() {
        *a = rand::random::<T>();
    }
}

fn shards_to_option_shards<T: Clone>(shards: &[Vec<T>]) -> Vec<Option<Vec<T>>> {
    let mut result = Vec::with_capacity(shards.len());

    for v in shards.iter() {
        let inner: Vec<T> = v.clone();
        result.push(Some(inner));
    }
    result
}

fn shards_into_option_shards<T>(shards: Vec<Vec<T>>) -> Vec<Option<Vec<T>>> {
    let mut result = Vec::with_capacity(shards.len());

    for v in shards {
        result.push(Some(v));
    }
    result
}

fn option_shards_to_shards<T: Clone>(shards: &[Option<Vec<T>>]) -> Vec<Vec<T>> {
    let mut result = Vec::with_capacity(shards.len());

    for (i, shard) in shards.iter().enumerate() {
        let shard = match shard {
            Some(x) => x,
            None => panic!("Missing shard, index : {}", i),
        };
        let inner: Vec<T> = shard.clone();
        result.push(inner);
    }
    result
}

fn option_shards_into_shards<T>(shards: Vec<Option<Vec<T>>>) -> Vec<Vec<T>> {
    let mut result = Vec::with_capacity(shards.len());

    for shard in shards {
        let shard = match shard {
            Some(x) => x,
            None => panic!("Missing shard"),
        };
        result.push(shard);
    }
    result
}

#[cfg(feature = "std")]
fn with_env_var<R>(key: &str, value: &str, f: impl FnOnce() -> R) -> R {
    // SAFETY: tests in this module set process-global env vars in a scoped manner
    // and restore them immediately after the assertion under test.
    unsafe {
        std::env::set_var(key, value);
    }
    let result = f();
    // SAFETY: paired cleanup for the scoped env var override above.
    unsafe {
        std::env::remove_var(key);
    }
    result
}

#[test]
fn test_no_data_shards() {
    assert_eq!(Error::TooFewDataShards, ReedSolomon::new(0, 1).unwrap_err());
}

#[test]
fn test_no_parity_shards() {
    assert_eq!(
        Error::TooFewParityShards,
        ReedSolomon::new(1, 0).unwrap_err()
    );
}

#[test]
fn test_too_many_shards() {
    assert_eq!(
        Error::TooManyShards,
        ReedSolomon::new(129, 128).unwrap_err()
    );
}

#[test]
fn test_shard_count() {
    let mut rng = rng();
    for _ in 0..10 {
        let data_shard_count = rng.random_range(1..128);
        let parity_shard_count = rng.random_range(1..128);

        let total_shard_count = data_shard_count + parity_shard_count;

        let r = ReedSolomon::new(data_shard_count, parity_shard_count).unwrap();

        assert_eq!(data_shard_count, r.data_shard_count());
        assert_eq!(parity_shard_count, r.parity_shard_count());
        assert_eq!(total_shard_count, r.total_shard_count());
    }
}

#[test]
fn test_codec_options_default_matches_new() {
    let r1 = ReedSolomon::new(10, 3).unwrap();
    let r2 = ReedSolomon::with_options(10, 3, CodecOptions::default()).unwrap();

    assert_eq!(r1, r2);
    assert_eq!(r1.data_shard_count(), r2.data_shard_count());
    assert_eq!(r1.parity_shard_count(), r2.parity_shard_count());
    assert_eq!(r1.total_shard_count(), r2.total_shard_count());
}

#[test]
fn test_codec_options_zero_cache_capacity_falls_back_to_default() {
    let options = CodecOptions {
        inversion_cache_capacity: 0,
        ..CodecOptions::default()
    };

    let r = ReedSolomon::with_options(10, 3, options).unwrap();

    assert_eq!(10, r.data_shard_count());
    assert_eq!(3, r.parity_shard_count());
    assert_eq!(
        ReedSolomon::recommended_inversion_cache_capacity(10, 3),
        r.inversion_cache_capacity()
    );
}

#[test]
fn test_recommended_inversion_cache_capacity_scales_with_workload() {
    let small = ReedSolomon::recommended_inversion_cache_capacity(4, 2);
    let medium = ReedSolomon::recommended_inversion_cache_capacity(10, 4);
    let large = ReedSolomon::recommended_inversion_cache_capacity(32, 16);

    assert_eq!(128, small);
    assert_eq!(128, medium);
    assert_eq!(2048, large);
    assert!(small <= medium);
    assert!(medium < large);
}

#[test]
fn test_codec_options_explicit_cache_capacity_is_preserved() {
    let options = CodecOptions {
        inversion_cache_capacity: 7,
        ..CodecOptions::default()
    };

    let r = ReedSolomon::with_options(10, 3, options).unwrap();

    assert_eq!(7, r.inversion_cache_capacity());
}

#[test]
fn test_codec_options_disable_inversion_cache_keeps_reconstruction_correct() {
    let options = CodecOptions {
        inversion_cache: false,
        ..CodecOptions::default()
    };
    let r = ReedSolomon::with_options(4, 2, options).unwrap();

    let mut shards = make_random_shards!(1024, 6);
    r.encode(&mut shards).unwrap();

    let original = shards.clone();
    let mut shards = shards_to_option_shards(&shards);
    shards[0] = None;
    shards[4] = None;

    r.reconstruct_data(&mut shards).unwrap();

    let reconstructed = option_shards_to_shards(&shards[0..4]);
    assert_eq!(original[0], reconstructed[0]);
}

#[test]
fn test_codec_options_accepts_non_default_matrix_mode_placeholder() {
    let options = CodecOptions {
        matrix_mode: MatrixMode::Cauchy,
        ..CodecOptions::default()
    };

    let r = ReedSolomon::with_options(3, 2, options).unwrap();

    assert_eq!(3, r.data_shard_count());
    assert_eq!(2, r.parity_shard_count());
}

#[test]
fn test_fast_one_parity_encode_uses_xor_parity() {
    let fast_rs = ReedSolomon::with_options(
        4,
        1,
        CodecOptions {
            fast_one_parity: true,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let mut shards = make_random_shards!(1024, 5);
    let expected_parity = {
        let mut parity = shards[0].clone();
        for shard in &shards[1..4] {
            for (dst, src) in parity.iter_mut().zip(shard.iter()) {
                *dst ^= *src;
            }
        }
        parity
    };

    fast_rs.encode(&mut shards).unwrap();

    assert_eq!(expected_parity, shards[4]);
}

#[test]
fn test_fast_one_parity_verify_matches_default_path() {
    let fast_rs = ReedSolomon::with_options(
        4,
        1,
        CodecOptions {
            fast_one_parity: true,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let mut shards = make_random_shards!(1024, 5);
    fast_rs.encode(&mut shards).unwrap();
    assert!(fast_rs.verify(&shards).unwrap());

    shards[4][7] ^= 0xff;
    assert!(!fast_rs.verify(&shards).unwrap());
}

#[test]
fn test_fast_one_parity_flag_does_not_change_multi_parity_behavior() {
    let regular = ReedSolomon::new(4, 2).unwrap();
    let configured = ReedSolomon::with_options(
        4,
        2,
        CodecOptions {
            fast_one_parity: true,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let mut expected = make_random_shards!(512, 6);
    let mut actual = expected.clone();

    regular.encode(&mut expected).unwrap();
    configured.encode(&mut actual).unwrap();

    assert_eq!(expected, actual);
}

#[test]
fn test_split_evenly_distributes_and_zero_pads() {
    let r = ReedSolomon::new(3, 2).unwrap();
    let shards = r.split(&[1u8, 2, 3, 4, 5]).unwrap();

    assert_eq!(shards.len(), 3);
    assert_eq!(shards[0], vec![1, 2]);
    assert_eq!(shards[1], vec![3, 4]);
    assert_eq!(shards[2], vec![5, 0]);
}

#[test]
fn test_split_empty_input_returns_empty_data_shards() {
    let r = ReedSolomon::new(3, 2).unwrap();
    let shards = r.split(&[] as &[u8]).unwrap();

    assert_eq!(shards.len(), 3);
    assert!(shards.iter().all(|shard| shard.is_empty()));
}

#[test]
fn test_join_truncates_padding_to_original_length() {
    let r = ReedSolomon::new(3, 2).unwrap();
    let shards = vec![vec![1u8, 2], vec![3u8, 4], vec![5u8, 0]];

    let joined = r.join(&shards, 5).unwrap();

    assert_eq!(joined, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_join_uses_only_data_shards() {
    let r = ReedSolomon::new(2, 1).unwrap();
    let joined = r.join(&[vec![1u8, 2], vec![3u8, 4]], 4).unwrap();

    assert_eq!(joined, vec![1, 2, 3, 4]);
}

#[test]
fn test_reconstruct_some_recovers_only_required_data_shard() {
    let r = ReedSolomon::new(4, 2).unwrap();
    let mut shards = make_random_shards!(1024, 6);
    r.encode(&mut shards).unwrap();
    let original = shards.clone();

    let mut shards = shards_to_option_shards(&shards);
    shards[1] = None;
    shards[4] = None;

    let mut required = vec![false; 6];
    required[1] = true;

    r.reconstruct_some(&mut shards, &required).unwrap();

    assert_eq!(shards[1].as_ref().unwrap(), &original[1]);
    assert!(shards[4].is_none());
}

#[test]
fn test_reconstruct_some_can_recover_required_parity_shard() {
    let r = ReedSolomon::new(4, 2).unwrap();
    let mut shards = make_random_shards!(1024, 6);
    r.encode(&mut shards).unwrap();
    let original = shards.clone();

    let mut shards = shards_to_option_shards(&shards);
    shards[1] = None;
    shards[4] = None;

    let mut required = vec![false; 6];
    required[1] = true;
    required[4] = true;

    r.reconstruct_some(&mut shards, &required).unwrap();

    assert_eq!(shards[1].as_ref().unwrap(), &original[1]);
    assert_eq!(shards[4].as_ref().unwrap(), &original[4]);
}

#[test]
fn test_reconstruct_some_rejects_invalid_required_flags_length() {
    let r = ReedSolomon::new(4, 2).unwrap();
    let mut shards = make_random_shards!(1024, 6);
    r.encode(&mut shards).unwrap();
    let mut shards = shards_to_option_shards(&shards);

    assert_eq!(
        Error::InvalidShardFlags,
        r.reconstruct_some(&mut shards, &[true, false]).unwrap_err()
    );
}

#[test]
fn test_reconstruct_some_recovers_only_requested_among_multiple_missing_data_shards() {
    let r = ReedSolomon::new(4, 2).unwrap();
    let mut shards = make_random_shards!(1024, 6);
    r.encode(&mut shards).unwrap();
    let original = shards.clone();

    let mut shards = shards_to_option_shards(&shards);
    shards[0] = None;
    shards[2] = None;

    let mut required = vec![false; 6];
    required[2] = true;

    r.reconstruct_some(&mut shards, &required).unwrap();

    assert!(shards[0].is_none());
    assert_eq!(shards[2].as_ref().unwrap(), &original[2]);
}

#[test]
fn test_reconstruct_some_allows_required_flag_for_present_shard() {
    let r = ReedSolomon::new(4, 2).unwrap();
    let mut shards = make_random_shards!(1024, 6);
    r.encode(&mut shards).unwrap();
    let original = shards.clone();

    let mut shards = shards_to_option_shards(&shards);
    shards[4] = None;

    let mut required = vec![false; 6];
    required[1] = true;

    r.reconstruct_some(&mut shards, &required).unwrap();

    assert_eq!(shards[1].as_ref().unwrap(), &original[1]);
    assert!(shards[4].is_none());
}

#[test]
fn test_code_chunk_len_small_shards_use_single_chunk() {
    let r = ReedSolomon::new(4, 2).unwrap();

    assert_eq!(8 * 1024, r.code_chunk_len(8 * 1024));
    assert_eq!(16 * 1024, r.code_chunk_len(16 * 1024));
}

#[test]
fn test_code_chunk_len_medium_shards_use_min_chunk() {
    let r = ReedSolomon::new(4, 2).unwrap();

    assert_eq!(16 * 1024, r.code_chunk_len(32 * 1024));
    assert_eq!(16 * 1024, r.code_chunk_len(64 * 1024));
}

#[test]
fn test_code_chunk_len_large_shards_use_default_chunk() {
    let r = ReedSolomon::new(4, 2).unwrap();

    assert_eq!(64 * 1024, r.code_chunk_len(512 * 1024));
    assert_eq!(64 * 1024, r.code_chunk_len(4 * 1024 * 1024));
}

#[test]
fn test_code_chunk_len_very_large_shards_use_large_chunk() {
    let r = ReedSolomon::new(4, 2).unwrap();

    assert_eq!(256 * 1024, r.code_chunk_len(8 * 1024 * 1024));
}

#[test]
fn test_code_chunk_len_parameterized_boundaries() {
    let r = ReedSolomon::new(4, 2).unwrap();
    let cases = [
        (1usize, 1usize),
        (16 * 1024, 16 * 1024),
        (16 * 1024 + 1, 16 * 1024),
        (64 * 1024, 16 * 1024),
        (64 * 1024 + 1, 64 * 1024),
        (4 * 1024 * 1024, 64 * 1024),
        (4 * 1024 * 1024 + 1, 256 * 1024),
        (8 * 1024 * 1024, 256 * 1024),
        (64 * 1024 * 1024, 256 * 1024),
    ];

    for (shard_len, expected_chunk) in cases {
        assert_eq!(expected_chunk, r.code_chunk_len(shard_len));
    }
}

#[cfg(feature = "std")]
#[test]
fn test_parallel_policy_keeps_small_shards_serial() {
    let policy = ParallelPolicy::default();

    assert_eq!(
        ParallelDecision {
            use_parallel: false,
            jobs: 1,
            chunk_len: 16 * 1024,
        },
        policy.decide(16 * 1024, 10, 4, 8)
    );
}

#[cfg(feature = "std")]
#[test]
fn test_parallel_policy_enables_large_shards_with_multiple_jobs() {
    let policy = ParallelPolicy::default();
    let decision = policy.decide(1024 * 1024, 10, 4, 8);

    assert!(decision.use_parallel);
    assert!(decision.jobs > 1);
    assert!(decision.chunk_len >= 16 * 1024);
    assert!(decision.chunk_len <= 256 * 1024);
}

#[cfg(feature = "std")]
#[test]
fn test_reed_solomon_parallel_policy_uses_available_parallelism() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let serial = r.parallel_policy_with(1024 * 1024, 4, 1);
    let parallel = r.parallel_policy_with(1024 * 1024, 4, 8);

    assert!(!serial.use_parallel);
    assert!(parallel.use_parallel);
    assert!(parallel.jobs > serial.jobs);
}

#[cfg(feature = "std")]
#[test]
fn test_reconstruct_parallel_policy_has_data_only_and_full_tiers() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let shard_len = 300 * 1024;
    let data_only = r.reconstruct_parallel_decision_with(shard_len, 2, 2, true, 8);
    let full = r.reconstruct_parallel_decision_with(shard_len, 2, 4, false, 8);

    assert!(!data_only.use_parallel);
    assert!(full.use_parallel);
}

#[cfg(feature = "std")]
#[test]
fn test_parallel_policy_creates_multiple_chunks_for_small_output_reconstruct_case() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let decision = r.parallel_policy_with(1024 * 1024, 2, 8);

    assert!(decision.use_parallel);
    assert_eq!(4, decision.jobs);
    assert_eq!(262144, decision.chunk_len);
}

#[cfg(feature = "std")]
#[test]
fn test_reconstruct_parallel_policy_respects_min_bytes_per_job_env() {
    let decision = with_env_var("RS_RECONSTRUCT_MIN_BYTES_PER_JOB", "65536", || {
        let r = ReedSolomon::new(10, 4).unwrap();
        r.reconstruct_parallel_decision_with(1024 * 1024, 2, 4, false, 8)
    });

    assert!(decision.use_parallel);
    assert_eq!(65536, decision.chunk_len);
}

#[cfg(all(feature = "std", target_arch = "aarch64"))]
#[test]
fn test_aarch64_reconstruct_parallel_policy_has_arch_specific_override() {
    // SAFETY: tests run in-process and we restore this env var before returning.
    unsafe {
        std::env::set_var("RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES", "131072");
        std::env::set_var("RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB", "131072");
        std::env::set_var("RS_AARCH64_RECONSTRUCT_MAX_JOBS", "4");
    }
    let r = ReedSolomon::new(10, 4).unwrap();
    let decision = r.reconstruct_parallel_decision_with(1024 * 1024, 2, 4, false, 8);
    // SAFETY: cleanup for process-global env var set above.
    unsafe {
        std::env::remove_var("RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES");
        std::env::remove_var("RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB");
        std::env::remove_var("RS_AARCH64_RECONSTRUCT_MAX_JOBS");
    }

    assert!(decision.use_parallel);
    assert_eq!(131072, decision.chunk_len);
    assert_eq!(4, decision.jobs);
}

#[cfg(all(feature = "std", target_arch = "aarch64"))]
#[test]
fn test_aarch64_reconstruct_stage_policies_allow_data_parity_split() {
    // SAFETY: tests run in-process and we restore these env vars before returning.
    unsafe {
        std::env::set_var("RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB", "65536");
        std::env::set_var("RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB", "262144");
    }
    let r = ReedSolomon::new(10, 4).unwrap();
    let (data_policy, parity_policy) = r.reconstruct_stage_policies_for_test(false);
    // SAFETY: cleanup for process-global env vars set above.
    unsafe {
        std::env::remove_var("RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB");
        std::env::remove_var("RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB");
    }

    assert_eq!(65536, data_policy.min_bytes_per_job);
    assert_eq!(262144, parity_policy.min_bytes_per_job);
}

#[cfg(all(feature = "std", not(target_arch = "aarch64")))]
#[test]
fn test_reconstruct_parallel_policy_default_arch_stays_on_default_chunk() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let decision = r.reconstruct_parallel_decision_with(1024 * 1024, 1, 2, false, 8);

    assert!(decision.use_parallel);
    assert_eq!(256 * 1024, decision.chunk_len);
}

#[cfg(feature = "std")]
#[test]
fn test_effective_parallel_policy_env_overrides() {
    let policy = with_env_var("RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES", "131072", || {
        with_env_var("RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB", "65536", || {
            with_env_var("RS_PARALLEL_POLICY_MAX_JOBS", "3", || {
                let r = ReedSolomon::new(10, 4).unwrap();
                r.effective_parallel_policy()
            })
        })
    });

    assert_eq!(131072, policy.min_parallel_shard_bytes);
    assert_eq!(65536, policy.min_bytes_per_job);
    assert_eq!(3, policy.max_jobs);
}

#[cfg(feature = "std")]
#[test]
fn test_parallel_policy_respects_env_max_jobs_cap() {
    let decision = with_env_var("RS_PARALLEL_POLICY_MAX_JOBS", "2", || {
        let r = ReedSolomon::new(10, 4).unwrap();
        r.parallel_policy_with(1024 * 1024, 16, 16)
    });

    assert!(decision.use_parallel);
    assert!(decision.jobs <= 2);
}

#[cfg(feature = "std")]
#[test]
fn test_parallel_policy_env_override_is_sampled_at_construction() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let policy = with_env_var("RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB", "65536", || {
        r.effective_parallel_policy()
    });

    assert_eq!(256 * 1024, policy.min_bytes_per_job);
}

#[cfg(feature = "std")]
struct ParallelHelperBenchResult {
    operation: &'static str,
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    policy_version: u32,
    policy_min_parallel_shard_bytes: usize,
    policy_min_bytes_per_job: usize,
    serial_mb_s: f64,
    parallel_mb_s: f64,
    speedup: f64,
}

#[cfg(feature = "std")]
struct ReconstructionHotspotBenchResult {
    scenario: &'static str,
    missing_pattern: String,
    required_pattern: String,
    baseline_operation: &'static str,
    candidate_operation: &'static str,
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    useful_shards: usize,
    baseline_mb_s: f64,
    candidate_mb_s: f64,
    speedup: f64,
}

#[cfg(feature = "std")]
fn shard_index_pattern(data_shards: usize, indices: &[usize]) -> String {
    if indices.is_empty() {
        return "-".to_string();
    }

    let mut parts = Vec::with_capacity(indices.len());
    for &index in indices {
        if index < data_shards {
            parts.push(format!("d{index}"));
        } else {
            parts.push(format!("p{}", index - data_shards));
        }
    }

    parts.join("|")
}

#[cfg(feature = "std")]
fn option_shards_with_missing(shards: &[Vec<u8>], missing_indices: &[usize]) -> Vec<Option<Vec<u8>>> {
    let mut working = shards_to_option_shards(shards);
    for &index in missing_indices {
        working[index] = None;
    }
    working
}

#[cfg(feature = "std")]
fn assert_required_reconstruction_matches(
    original: &[Vec<u8>],
    baseline: &[Option<Vec<u8>>],
    candidate: &[Option<Vec<u8>>],
    required_indices: &[usize],
    missing_indices: &[usize],
) {
    for &index in required_indices {
        assert_eq!(baseline[index].as_ref().unwrap(), &original[index]);
        assert_eq!(candidate[index].as_ref().unwrap(), &original[index]);
    }

    for &index in missing_indices {
        if !required_indices.contains(&index) {
            assert!(candidate[index].is_none());
        }
    }
}

#[cfg(feature = "std")]
fn bench_reconstruct_data_hotspot(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    scenario: &'static str,
    missing_indices: &[usize],
) -> ReconstructionHotspotBenchResult {
    let r = ReedSolomon::new(data_shards, parity_shards).unwrap();
    let iterations = 3usize;
    let missing_data_indices: Vec<usize> = missing_indices
        .iter()
        .copied()
        .filter(|&index| index < data_shards)
        .collect();
    let useful_shards = missing_data_indices.len().max(1);
    let bytes = (useful_shards * shard_size) as f64;

    let mut baseline_total = 0.0;
    let mut candidate_total = 0.0;

    for _ in 0..iterations {
        let mut shards = make_random_shards!(shard_size, data_shards + parity_shards);
        r.encode(&mut shards).unwrap();

        let mut baseline = option_shards_with_missing(&shards, missing_indices);
        let baseline_start = Instant::now();
        r.reconstruct(&mut baseline).unwrap();
        baseline_total += baseline_start.elapsed().as_secs_f64();

        let mut candidate = option_shards_with_missing(&shards, missing_indices);
        let candidate_start = Instant::now();
        r.reconstruct_data(&mut candidate).unwrap();
        candidate_total += candidate_start.elapsed().as_secs_f64();

        for &index in &missing_data_indices {
            assert_eq!(baseline[index].as_ref().unwrap(), &shards[index]);
            assert_eq!(candidate[index].as_ref().unwrap(), &shards[index]);
        }
        for &index in missing_indices {
            if index >= data_shards {
                assert!(candidate[index].is_none());
            }
        }
    }

    let baseline_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / baseline_total;
    let candidate_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / candidate_total;

    ReconstructionHotspotBenchResult {
        scenario,
        missing_pattern: shard_index_pattern(data_shards, missing_indices),
        required_pattern: shard_index_pattern(data_shards, &missing_data_indices),
        baseline_operation: "reconstruct",
        candidate_operation: "reconstruct_data",
        data_shards,
        parity_shards,
        shard_size,
        useful_shards,
        baseline_mb_s,
        candidate_mb_s,
        speedup: candidate_mb_s / baseline_mb_s,
    }
}

#[cfg(feature = "std")]
fn bench_reconstruct_some_hotspot(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    scenario: &'static str,
    missing_indices: &[usize],
    required_indices: &[usize],
) -> ReconstructionHotspotBenchResult {
    let r = ReedSolomon::new(data_shards, parity_shards).unwrap();
    let iterations = 3usize;
    let bytes = (required_indices.len().max(1) * shard_size) as f64;
    let mut required = vec![false; data_shards + parity_shards];
    for &index in required_indices {
        required[index] = true;
    }

    let mut baseline_total = 0.0;
    let mut candidate_total = 0.0;

    for _ in 0..iterations {
        let mut shards = make_random_shards!(shard_size, data_shards + parity_shards);
        r.encode(&mut shards).unwrap();

        let mut baseline = option_shards_with_missing(&shards, missing_indices);
        let baseline_start = Instant::now();
        r.reconstruct_data(&mut baseline).unwrap();
        baseline_total += baseline_start.elapsed().as_secs_f64();

        let mut candidate = option_shards_with_missing(&shards, missing_indices);
        let candidate_start = Instant::now();
        r.reconstruct_some(&mut candidate, &required).unwrap();
        candidate_total += candidate_start.elapsed().as_secs_f64();

        assert_required_reconstruction_matches(
            &shards,
            &baseline,
            &candidate,
            required_indices,
            missing_indices,
        );
    }

    let baseline_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / baseline_total;
    let candidate_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / candidate_total;

    ReconstructionHotspotBenchResult {
        scenario,
        missing_pattern: shard_index_pattern(data_shards, missing_indices),
        required_pattern: shard_index_pattern(data_shards, required_indices),
        baseline_operation: "reconstruct_data",
        candidate_operation: "reconstruct_some",
        data_shards,
        parity_shards,
        shard_size,
        useful_shards: required_indices.len(),
        baseline_mb_s,
        candidate_mb_s,
        speedup: candidate_mb_s / baseline_mb_s,
    }
}

#[cfg(feature = "std")]
fn bench_encode_sep_pair(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> ParallelHelperBenchResult {
    let r = ReedSolomon::new(data_shards, parity_shards).unwrap();
    let policy = r.effective_parallel_policy();
    let iterations = 3usize;
    let bytes = (data_shards * shard_size) as f64;

    let mut serial_total = 0.0;
    let mut parallel_total = 0.0;

    for _ in 0..iterations {
        let shards = make_random_shards!(shard_size, data_shards + parity_shards);

        let mut serial = shards.clone();
        let serial_start = Instant::now();
        {
            let (data, parity) = serial.split_at_mut(data_shards);
            r.encode_sep(data, parity).unwrap();
        }
        serial_total += serial_start.elapsed().as_secs_f64();

        let mut parallel = shards;
        let parallel_start = Instant::now();
        {
            let (data, parity) = parallel.split_at_mut(data_shards);
            r.encode_sep_par(data, parity).unwrap();
        }
        parallel_total += parallel_start.elapsed().as_secs_f64();

        assert_eq!(serial, parallel);
    }

    let serial_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / serial_total;
    let parallel_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / parallel_total;

    ParallelHelperBenchResult {
        operation: "encode_sep_vs_encode_sep_par",
        data_shards,
        parity_shards,
        shard_size,
        policy_version: r.parallel_policy_version(),
        policy_min_parallel_shard_bytes: policy.min_parallel_shard_bytes,
        policy_min_bytes_per_job: policy.min_bytes_per_job,
        serial_mb_s,
        parallel_mb_s,
        speedup: parallel_mb_s / serial_mb_s,
    }
}

#[cfg(feature = "std")]
fn bench_verify_with_buffer_pair(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> ParallelHelperBenchResult {
    let r = ReedSolomon::new(data_shards, parity_shards).unwrap();
    let policy = r.effective_parallel_policy();
    let iterations = 3usize;
    let bytes = (data_shards * shard_size) as f64;

    let mut serial_total = 0.0;
    let mut parallel_total = 0.0;

    for _ in 0..iterations {
        let mut shards = make_random_shards!(shard_size, data_shards + parity_shards);
        r.encode(&mut shards).unwrap();

        let mut serial_buffer = make_random_shards!(shard_size, parity_shards);
        let serial_start = Instant::now();
        let serial_ok = r.verify_with_buffer(&shards, &mut serial_buffer).unwrap();
        serial_total += serial_start.elapsed().as_secs_f64();

        let mut parallel_buffer = make_random_shards!(shard_size, parity_shards);
        let parallel_start = Instant::now();
        let parallel_ok = r
            .verify_with_buffer_par(&shards, &mut parallel_buffer)
            .unwrap();
        parallel_total += parallel_start.elapsed().as_secs_f64();

        assert_eq!(serial_ok, parallel_ok);
        assert_eq!(serial_buffer, parallel_buffer);
    }

    let serial_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / serial_total;
    let parallel_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / parallel_total;

    ParallelHelperBenchResult {
        operation: "verify_with_buffer_vs_verify_with_buffer_par",
        data_shards,
        parity_shards,
        shard_size,
        policy_version: r.parallel_policy_version(),
        policy_min_parallel_shard_bytes: policy.min_parallel_shard_bytes,
        policy_min_bytes_per_job: policy.min_bytes_per_job,
        serial_mb_s,
        parallel_mb_s,
        speedup: parallel_mb_s / serial_mb_s,
    }
}

#[cfg(feature = "std")]
fn bench_reconstruct_pair(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    data_only: bool,
) -> ParallelHelperBenchResult {
    let r = ReedSolomon::new(data_shards, parity_shards).unwrap();
    let policy = r.effective_parallel_policy();
    let iterations = 3usize;
    let bytes = (data_shards * shard_size) as f64;

    let mut serial_total = 0.0;
    let mut parallel_total = 0.0;

    for _ in 0..iterations {
        let mut shards = make_random_shards!(shard_size, data_shards + parity_shards);
        r.encode(&mut shards).unwrap();

        let mut serial = shards_to_option_shards(&shards);
        serial[0] = None;
        serial[data_shards] = None;

        let serial_start = Instant::now();
        if data_only {
            r.reconstruct_data(&mut serial).unwrap();
        } else {
            r.reconstruct(&mut serial).unwrap();
        }
        serial_total += serial_start.elapsed().as_secs_f64();

        let mut parallel = shards_to_option_shards(&shards);
        parallel[0] = None;
        parallel[data_shards] = None;

        let parallel_start = Instant::now();
        if data_only {
            r.reconstruct_data_opt(&mut parallel).unwrap();
        } else {
            r.reconstruct_opt(&mut parallel).unwrap();
        }
        parallel_total += parallel_start.elapsed().as_secs_f64();

        assert_eq!(serial, parallel);
    }

    let serial_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / serial_total;
    let parallel_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / parallel_total;

    ParallelHelperBenchResult {
        operation: if data_only {
            "reconstruct_data_vs_reconstruct_data_opt"
        } else {
            "reconstruct_vs_reconstruct_opt"
        },
        data_shards,
        parity_shards,
        shard_size,
        policy_version: r.parallel_policy_version(),
        policy_min_parallel_shard_bytes: policy.min_parallel_shard_bytes,
        policy_min_bytes_per_job: policy.min_bytes_per_job,
        serial_mb_s,
        parallel_mb_s,
        speedup: parallel_mb_s / serial_mb_s,
    }
}

#[cfg(feature = "std")]
fn bench_reconstruct_some_required_data_pair(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    required_count: usize,
) -> ParallelHelperBenchResult {
    let r = ReedSolomon::new(data_shards, parity_shards).unwrap();
    let policy = r.effective_parallel_policy();
    let iterations = 3usize;
    let bytes = (required_count * shard_size) as f64;

    let mut serial_total = 0.0;
    let mut optimized_total = 0.0;

    for _ in 0..iterations {
        let mut shards = make_random_shards!(shard_size, data_shards + parity_shards);
        r.encode(&mut shards).unwrap();

        let mut serial = shards_to_option_shards(&shards);
        for i in 0..required_count {
            serial[i * 2] = None;
        }

        let serial_start = Instant::now();
        r.reconstruct_data(&mut serial).unwrap();
        serial_total += serial_start.elapsed().as_secs_f64();

        let mut optimized = shards_to_option_shards(&shards);
        for i in 0..required_count {
            optimized[i * 2] = None;
        }
        let mut required = vec![false; data_shards + parity_shards];
        for i in 0..required_count {
            required[i * 2] = true;
        }

        let optimized_start = Instant::now();
        r.reconstruct_some(&mut optimized, &required).unwrap();
        optimized_total += optimized_start.elapsed().as_secs_f64();

        for i in 0..required_count {
            assert_eq!(serial[i * 2], optimized[i * 2]);
        }
    }

    let serial_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / serial_total;
    let parallel_mb_s = (bytes * iterations as f64) / (1024.0 * 1024.0) / optimized_total;

    ParallelHelperBenchResult {
        operation: match required_count {
            1 => "reconstruct_some_required_1_vs_reconstruct_data",
            2 => "reconstruct_some_required_2_vs_reconstruct_data",
            4 => "reconstruct_some_required_4_vs_reconstruct_data",
            _ => "reconstruct_some_required_n_vs_reconstruct_data",
        },
        data_shards,
        parity_shards,
        shard_size,
        policy_version: r.parallel_policy_version(),
        policy_min_parallel_shard_bytes: policy.min_parallel_shard_bytes,
        policy_min_bytes_per_job: policy.min_bytes_per_job,
        serial_mb_s,
        parallel_mb_s,
        speedup: parallel_mb_s / serial_mb_s,
    }
}

#[cfg(feature = "std")]
fn write_parallel_helper_bench_results(results: &[ParallelHelperBenchResult]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let json_path = dir.join("parallel-helper-results.json");
    let csv_path = dir.join("parallel-helper-results.csv");

    let mut json = String::from("[\n");
    for (i, result) in results.iter().enumerate() {
        let suffix = if i + 1 == results.len() { "\n" } else { ",\n" };
        json.push_str(&format!(
            "  {{\"operation\":\"{}\",\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"policy_version\":{},\"policy_min_parallel_shard_bytes\":{},\"policy_min_bytes_per_job\":{},\"serial_mb_s\":{:.4},\"parallel_mb_s\":{:.4},\"speedup\":{:.4}}}{}",
            result.operation,
            result.data_shards,
            result.parity_shards,
            result.shard_size,
            result.policy_version,
            result.policy_min_parallel_shard_bytes,
            result.policy_min_bytes_per_job,
            result.serial_mb_s,
            result.parallel_mb_s,
            result.speedup,
            suffix
        ));
    }
    json.push(']');
    fs::write(&json_path, json).unwrap();

    let mut csv = String::from(
        "operation,data_shards,parity_shards,shard_size,policy_version,policy_min_parallel_shard_bytes,policy_min_bytes_per_job,serial_mb_s,parallel_mb_s,speedup\n",
    );
    for result in results {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{:.4},{:.4},{:.4}\n",
            result.operation,
            result.data_shards,
            result.parity_shards,
            result.shard_size,
            result.policy_version,
            result.policy_min_parallel_shard_bytes,
            result.policy_min_bytes_per_job,
            result.serial_mb_s,
            result.parallel_mb_s,
            result.speedup
        ));
    }
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

#[cfg(feature = "std")]
fn write_reconstruction_hotspot_bench_results(results: &[ReconstructionHotspotBenchResult]) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();

    let json_path = dir.join("reconstruction-hotspot-results.json");
    let csv_path = dir.join("reconstruction-hotspot-results.csv");

    let mut json = String::from("[\n");
    for (i, result) in results.iter().enumerate() {
        let suffix = if i + 1 == results.len() { "\n" } else { ",\n" };
        json.push_str(&format!(
            "  {{\"scenario\":\"{}\",\"missing_pattern\":\"{}\",\"required_pattern\":\"{}\",\"baseline_operation\":\"{}\",\"candidate_operation\":\"{}\",\"data_shards\":{},\"parity_shards\":{},\"shard_size\":{},\"useful_shards\":{},\"baseline_mb_s\":{:.4},\"candidate_mb_s\":{:.4},\"speedup\":{:.4}}}{}",
            result.scenario,
            result.missing_pattern,
            result.required_pattern,
            result.baseline_operation,
            result.candidate_operation,
            result.data_shards,
            result.parity_shards,
            result.shard_size,
            result.useful_shards,
            result.baseline_mb_s,
            result.candidate_mb_s,
            result.speedup,
            suffix
        ));
    }
    json.push(']');
    fs::write(&json_path, json).unwrap();

    let mut csv = String::from(
        "scenario,missing_pattern,required_pattern,baseline_operation,candidate_operation,data_shards,parity_shards,shard_size,useful_shards,baseline_mb_s,candidate_mb_s,speedup\n",
    );
    for result in results {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.4},{:.4},{:.4}\n",
            result.scenario,
            result.missing_pattern,
            result.required_pattern,
            result.baseline_operation,
            result.candidate_operation,
            result.data_shards,
            result.parity_shards,
            result.shard_size,
            result.useful_shards,
            result.baseline_mb_s,
            result.candidate_mb_s,
            result.speedup
        ));
    }
    fs::write(&csv_path, csv).unwrap();

    assert!(json_path.exists());
    assert!(csv_path.exists());
}

#[cfg(feature = "std")]
#[test]
fn benchmark_parallel_helpers_quantify_gain() {
    let results = vec![
        bench_encode_sep_pair(10, 4, 1024 * 1024),
        bench_encode_sep_pair(32, 16, 1024 * 1024),
        bench_verify_with_buffer_pair(10, 4, 1024 * 1024),
        bench_verify_with_buffer_pair(32, 16, 1024 * 1024),
        bench_reconstruct_pair(10, 4, 1024 * 1024, false),
        bench_reconstruct_pair(10, 4, 1024 * 1024, true),
        bench_reconstruct_pair(32, 16, 1024 * 1024, false),
        bench_reconstruct_pair(32, 16, 1024 * 1024, true),
        bench_reconstruct_some_required_data_pair(10, 4, 1024 * 1024, 1),
        bench_reconstruct_some_required_data_pair(10, 4, 1024 * 1024, 2),
        bench_reconstruct_some_required_data_pair(10, 4, 1024 * 1024, 4),
        bench_reconstruct_some_required_data_pair(32, 16, 1024 * 1024, 1),
        bench_reconstruct_some_required_data_pair(32, 16, 1024 * 1024, 2),
        bench_reconstruct_some_required_data_pair(32, 16, 1024 * 1024, 4),
    ];

    assert!(results.iter().all(|result| result.serial_mb_s.is_finite()));
    assert!(
        results
            .iter()
            .all(|result| result.parallel_mb_s.is_finite())
    );
    assert!(results.iter().all(|result| result.speedup.is_finite()));

    write_parallel_helper_bench_results(&results);
}

#[cfg(feature = "std")]
#[test]
fn benchmark_reconstruction_hotspots() {
    let results = vec![
        bench_reconstruct_data_hotspot(10, 4, 1024 * 1024, "reconstruct_data_missing_1_data", &[0]),
        bench_reconstruct_data_hotspot(10, 4, 1024 * 1024, "reconstruct_data_missing_2_data", &[0, 2]),
        bench_reconstruct_data_hotspot(
            10,
            4,
            1024 * 1024,
            "reconstruct_data_missing_data_plus_parity",
            &[1, 10],
        ),
        bench_reconstruct_data_hotspot(
            32,
            16,
            1024 * 1024,
            "reconstruct_data_32x16_missing_2_data",
            &[0, 2],
        ),
        bench_reconstruct_some_hotspot(
            10,
            4,
            1024 * 1024,
            "reconstruct_some_required_1_of_2_missing_data",
            &[0, 2],
            &[2],
        ),
        bench_reconstruct_some_hotspot(
            10,
            4,
            1024 * 1024,
            "reconstruct_some_required_2_of_3_missing_data",
            &[0, 2, 4],
            &[0, 4],
        ),
        bench_reconstruct_some_hotspot(
            10,
            4,
            1024 * 1024,
            "reconstruct_some_required_data_and_skip_parity",
            &[1, 10],
            &[1],
        ),
        bench_reconstruct_some_hotspot(
            32,
            16,
            1024 * 1024,
            "reconstruct_some_32x16_required_2_of_4_missing_data",
            &[0, 2, 4, 6],
            &[2, 6],
        ),
    ];

    assert!(results.iter().all(|result| result.baseline_mb_s.is_finite()));
    assert!(results.iter().all(|result| result.candidate_mb_s.is_finite()));
    assert!(results.iter().all(|result| result.speedup.is_finite()));

    write_reconstruction_hotspot_bench_results(&results);
}

#[cfg(feature = "std")]
#[test]
fn test_reconstruction_cache_stats_track_hits_and_misses() {
    let r = ReedSolomon::with_options(
        8,
        5,
        CodecOptions {
            inversion_cache: true,
            inversion_cache_capacity: 16,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let mut shards = make_random_shards!(4096, 13);
    r.encode(&mut shards).unwrap();

    let mut first = shards_to_option_shards(&shards);
    first[0] = None;
    first[2] = None;
    r.reconstruct(&mut first).unwrap();

    let stats_after_first = r.reconstruction_cache_stats();
    assert_eq!(1, stats_after_first.requests);
    assert_eq!(0, stats_after_first.hits);
    assert_eq!(1, stats_after_first.misses);
    assert_eq!(1, stats_after_first.inserts);
    assert_eq!(0, stats_after_first.evictions);

    let mut second = shards_to_option_shards(&shards);
    second[0] = None;
    second[2] = None;
    r.reconstruct(&mut second).unwrap();

    let stats_after_second = r.reconstruction_cache_stats();
    assert_eq!(2, stats_after_second.requests);
    assert_eq!(1, stats_after_second.hits);
    assert_eq!(1, stats_after_second.misses);
    assert_eq!(1, stats_after_second.inserts);
    assert_eq!(0, stats_after_second.evictions);

    let analysis = stats_after_second.analysis();
    assert!((analysis.hit_rate - 0.5).abs() < f64::EPSILON);
    assert!((analysis.reuse_ratio - 1.0).abs() < f64::EPSILON);
    assert!((analysis.miss_cost_per_request - 0.5).abs() < f64::EPSILON);
}

#[cfg(feature = "std")]
#[test]
fn test_reconstruction_cache_stats_track_evictions() {
    let r = ReedSolomon::with_options(
        8,
        5,
        CodecOptions {
            inversion_cache: true,
            inversion_cache_capacity: 2,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let mut shards = make_random_shards!(4096, 13);
    r.encode(&mut shards).unwrap();

    for missing in &[(0usize, 1usize), (0, 2), (0, 3)] {
        let mut working = shards_to_option_shards(&shards);
        working[missing.0] = None;
        working[missing.1] = None;
        r.reconstruct_data(&mut working).unwrap();
    }

    let stats = r.reconstruction_cache_stats();
    assert!(stats.inserts >= 3);
    assert!(stats.evictions >= 1);
}

#[cfg(feature = "std")]
#[test]
fn benchmark_reconstruction_cache_patterns() {
    let r = ReedSolomon::with_options(
        10,
        4,
        CodecOptions {
            inversion_cache: true,
            inversion_cache_capacity: 64,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let mut shards = make_random_shards!(256 * 1024, 14);
    r.encode(&mut shards).unwrap();

    for _ in 0..5 {
        let mut repeated = shards_to_option_shards(&shards);
        repeated[0] = None;
        repeated[3] = None;
        r.reconstruct_data(&mut repeated).unwrap();
    }

    for offset in 0..5 {
        let mut varying = shards_to_option_shards(&shards);
        varying[offset] = None;
        varying[offset + 1] = None;
        r.reconstruct_data(&mut varying).unwrap();
    }

    let stats = r.reconstruction_cache_stats();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("reconstruction-cache-stats.json");

    let body = format!(
        "{{\"requests\":{},\"hits\":{},\"misses\":{},\"inserts\":{},\"evictions\":{},\"hit_rate\":{:.6},\"reuse_ratio\":{:.6},\"miss_cost_per_request\":{:.6}}}",
        stats.requests,
        stats.hits,
        stats.misses,
        stats.inserts,
        stats.evictions,
        stats.hit_rate(),
        stats.reuse_ratio(),
        stats.miss_cost_per_request()
    );
    fs::write(&path, body).unwrap();
    assert!(path.exists());
    assert!(stats.requests >= 10);
}

#[cfg(feature = "std")]
fn run_reconstruction_pattern(
    r: &ReedSolomon,
    shards: &[Vec<u8>],
    data_only: bool,
    missing_pairs: &[(usize, usize)],
) -> f64 {
    let start = Instant::now();
    for &(a, b) in missing_pairs {
        let mut working = shards_to_option_shards(shards);
        working[a] = None;
        working[b] = None;
        if data_only {
            r.reconstruct_data(&mut working).unwrap();
        } else {
            r.reconstruct(&mut working).unwrap();
        }
    }
    start.elapsed().as_secs_f64()
}

#[cfg(feature = "std")]
#[test]
fn benchmark_reconstruction_cache_layers() {
    let data_shards = 10usize;
    let parity_shards = 4usize;
    let shard_size = 1024 * 1024usize;

    let with_cache = ReedSolomon::with_options(
        data_shards,
        parity_shards,
        CodecOptions {
            inversion_cache: true,
            inversion_cache_capacity: 64,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let without_cache = ReedSolomon::with_options(
        data_shards,
        parity_shards,
        CodecOptions {
            inversion_cache: false,
            inversion_cache_capacity: 64,
            ..CodecOptions::default()
        },
    )
    .unwrap();

    let mut shards = make_random_shards!(shard_size, data_shards + parity_shards);
    with_cache.encode(&mut shards).unwrap();

    let repeated_pairs = vec![(0usize, 3usize); 6];
    let varying_pairs = vec![(0usize, 1usize), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)];

    let repeated_data_cached =
        run_reconstruction_pattern(&with_cache, &shards, true, &repeated_pairs);
    let repeated_data_uncached =
        run_reconstruction_pattern(&without_cache, &shards, true, &repeated_pairs);
    let varying_data_cached =
        run_reconstruction_pattern(&with_cache, &shards, true, &varying_pairs);
    let varying_data_uncached =
        run_reconstruction_pattern(&without_cache, &shards, true, &varying_pairs);

    let repeated_all_cached =
        run_reconstruction_pattern(&with_cache, &shards, false, &repeated_pairs);
    let repeated_all_uncached =
        run_reconstruction_pattern(&without_cache, &shards, false, &repeated_pairs);
    let varying_all_cached =
        run_reconstruction_pattern(&with_cache, &shards, false, &varying_pairs);
    let varying_all_uncached =
        run_reconstruction_pattern(&without_cache, &shards, false, &varying_pairs);

    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("reconstruction-cache-patterns.csv");

    let body = format!(
        "scenario,seconds\nrepeated_data_cached,{:.6}\nrepeated_data_uncached,{:.6}\nvarying_data_cached,{:.6}\nvarying_data_uncached,{:.6}\nrepeated_all_cached,{:.6}\nrepeated_all_uncached,{:.6}\nvarying_all_cached,{:.6}\nvarying_all_uncached,{:.6}\n",
        repeated_data_cached,
        repeated_data_uncached,
        varying_data_cached,
        varying_data_uncached,
        repeated_all_cached,
        repeated_all_uncached,
        varying_all_cached,
        varying_all_uncached,
    );
    fs::write(&path, body).unwrap();

    assert!(path.exists());
}

#[cfg(feature = "std")]
#[test]
fn test_encode_sep_par_matches_encode_sep() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let shards = make_random_shards!(256 * 1024, 14);
    let mut expected = shards.clone();
    let mut actual = shards.clone();

    let (expected_data, expected_parity) = expected.split_at_mut(10);
    r.encode_sep(expected_data, expected_parity).unwrap();

    let (actual_data, actual_parity) = actual.split_at_mut(10);
    r.encode_sep_par(actual_data, actual_parity).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_encode_single_sep_par_matches_encode_single_sep() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let shards = make_random_shards!(256 * 1024, 14);
    let (data, parity_src) = shards.split_at(10);
    let mut expected_parity = parity_src.to_vec();
    let mut actual_parity = parity_src.to_vec();

    for (i, shard) in data.iter().enumerate().take(10) {
        r.encode_single_sep(i, shard, &mut expected_parity)
            .unwrap();
        r.encode_single_sep_par(i, shard, &mut actual_parity)
            .unwrap();
    }

    assert_eq_shards(&expected_parity, &actual_parity);
}

#[cfg(feature = "std")]
#[test]
fn test_encode_single_opt_matches_encode_single() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut expected = make_random_shards!(256 * 1024, 14);
    let mut actual = expected.clone();

    for i in 0..10 {
        r.encode_single(i, &mut expected).unwrap();
        r.encode_single_opt(i, &mut actual).unwrap();
    }

    assert_eq_shards(&expected, &actual);
}

#[cfg(feature = "std")]
#[test]
fn test_encode_single_sep_opt_matches_encode_single_sep_for_small_shards() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let shards = make_random_shards!(8 * 1024, 14);
    let (data, parity_src) = shards.split_at(10);
    let mut expected_parity = parity_src.to_vec();
    let mut actual_parity = parity_src.to_vec();

    assert!(!r.parallel_policy(8 * 1024, 4).use_parallel);

    for (i, shard) in data.iter().enumerate().take(10) {
        r.encode_single_sep(i, shard, &mut expected_parity)
            .unwrap();
        r.encode_single_sep_opt(i, shard, &mut actual_parity)
            .unwrap();
    }

    assert_eq_shards(&expected_parity, &actual_parity);
}

#[cfg(feature = "std")]
#[test]
fn test_encode_single_sep_opt_matches_encode_single_sep_for_large_shards() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let shards = make_random_shards!(256 * 1024, 14);
    let (data, parity_src) = shards.split_at(10);
    let mut expected_parity = parity_src.to_vec();
    let mut actual_parity = parity_src.to_vec();

    assert!(r.parallel_policy(256 * 1024, 4).use_parallel);

    for (i, shard) in data.iter().enumerate().take(10) {
        r.encode_single_sep(i, shard, &mut expected_parity)
            .unwrap();
        r.encode_single_sep_opt(i, shard, &mut actual_parity)
            .unwrap();
    }

    assert_eq_shards(&expected_parity, &actual_parity);
}

#[cfg(feature = "std")]
#[test]
fn test_encode_single_opt_matches_encode_single_for_small_shards() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut expected = make_random_shards!(8 * 1024, 14);
    let mut actual = expected.clone();

    assert!(!r.parallel_policy(8 * 1024, 4).use_parallel);

    for i in 0..10 {
        r.encode_single(i, &mut expected).unwrap();
        r.encode_single_opt(i, &mut actual).unwrap();
    }

    assert_eq_shards(&expected, &actual);
}

#[cfg(feature = "std")]
#[test]
fn test_encode_single_opt_matches_encode_single_for_large_shards() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut expected = make_random_shards!(256 * 1024, 14);
    let mut actual = expected.clone();

    assert!(r.parallel_policy(256 * 1024, 4).use_parallel);

    for i in 0..10 {
        r.encode_single(i, &mut expected).unwrap();
        r.encode_single_opt(i, &mut actual).unwrap();
    }

    assert_eq_shards(&expected, &actual);
}

#[cfg(feature = "std")]
#[test]
fn test_encode_par_matches_encode() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut expected = make_random_shards!(256 * 1024, 14);
    let mut actual = expected.clone();

    r.encode(&mut expected).unwrap();
    r.encode_par(&mut actual).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_verify_with_buffer_par_matches_verify_with_buffer() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(256 * 1024, 14);
    r.encode(&mut shards).unwrap();

    let mut expected_buffer = make_random_shards!(256 * 1024, 4);
    let mut actual_buffer = expected_buffer.clone();

    let expected = r.verify_with_buffer(&shards, &mut expected_buffer).unwrap();
    let actual = r
        .verify_with_buffer_par(&shards, &mut actual_buffer)
        .unwrap();

    assert_eq!(expected, actual);
    assert_eq!(expected_buffer, actual_buffer);
}

#[cfg(feature = "std")]
#[test]
fn test_verify_par_matches_verify() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(256 * 1024, 14);
    r.encode(&mut shards).unwrap();

    let expected = r.verify(&shards).unwrap();
    let actual = r.verify_par(&shards).unwrap();

    assert_eq!(expected, actual);

    shards[13][15] ^= 0xff;

    let expected = r.verify(&shards).unwrap();
    let actual = r.verify_par(&shards).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_encode_opt_matches_encode() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut expected = make_random_shards!(256 * 1024, 14);
    let mut actual = expected.clone();

    r.encode(&mut expected).unwrap();
    r.encode_opt(&mut actual).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_encode_opt_matches_encode_for_small_shards() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut expected = make_random_shards!(8 * 1024, 14);
    let mut actual = expected.clone();

    assert!(!r.parallel_policy(8 * 1024, 4).use_parallel);

    r.encode(&mut expected).unwrap();
    r.encode_opt(&mut actual).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_verify_opt_matches_verify() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(256 * 1024, 14);
    r.encode(&mut shards).unwrap();

    let expected = r.verify(&shards).unwrap();
    let actual = r.verify_opt(&shards).unwrap();
    assert_eq!(expected, actual);

    shards[13][31] ^= 0xff;
    let expected = r.verify(&shards).unwrap();
    let actual = r.verify_opt(&shards).unwrap();
    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_verify_with_buffer_opt_matches_verify_with_buffer() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(256 * 1024, 14);
    r.encode(&mut shards).unwrap();

    let mut expected_buffer = make_random_shards!(256 * 1024, 4);
    let mut actual_buffer = expected_buffer.clone();

    let expected = r.verify_with_buffer(&shards, &mut expected_buffer).unwrap();
    let actual = r
        .verify_with_buffer_opt(&shards, &mut actual_buffer)
        .unwrap();

    assert_eq!(expected, actual);
    assert_eq!(expected_buffer, actual_buffer);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_reconstruct_data_opt_matches_reconstruct_data() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(256 * 1024, 14);
    r.encode(&mut shards).unwrap();

    let mut expected = shards_to_option_shards(&shards);
    expected[0] = None;
    expected[1] = None;

    let mut actual = expected.clone();

    r.reconstruct_data(&mut expected).unwrap();
    r.reconstruct_data_opt(&mut actual).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_reconstruct_data_opt_matches_reconstruct_data_for_small_shards() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(8 * 1024, 14);
    r.encode(&mut shards).unwrap();

    let mut expected = shards_to_option_shards(&shards);
    expected[0] = None;
    expected[1] = None;

    let mut actual = expected.clone();

    assert!(!r.parallel_policy(8 * 1024, 2).use_parallel);

    r.reconstruct_data(&mut expected).unwrap();
    r.reconstruct_data_opt(&mut actual).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_reconstruct_opt_matches_reconstruct() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(256 * 1024, 14);
    r.encode(&mut shards).unwrap();

    let mut expected = shards_to_option_shards(&shards);
    expected[0] = None;
    expected[12] = None;

    let mut actual = expected.clone();

    r.reconstruct(&mut expected).unwrap();
    r.reconstruct_opt(&mut actual).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_reconstruct_some_opt_matches_reconstruct_some_for_data_only() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(256 * 1024, 14);
    r.encode(&mut shards).unwrap();

    let mut expected = shards_to_option_shards(&shards);
    expected[0] = None;
    expected[2] = None;

    let mut actual = expected.clone();
    let mut required = vec![false; 14];
    required[2] = true;

    r.reconstruct_some(&mut expected, &required).unwrap();
    r.reconstruct_some_opt(&mut actual, &required).unwrap();

    assert_eq!(expected, actual);
}

#[cfg(feature = "std")]
#[test]
fn test_galois_8_reconstruct_some_opt_rejects_invalid_flags_length() {
    let r = ReedSolomon::new(10, 4).unwrap();
    let mut shards = make_random_shards!(256 * 1024, 14)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();

    assert_eq!(
        Error::InvalidShardFlags,
        r.reconstruct_some_opt(&mut shards, &[true, false])
            .unwrap_err()
    );
}

#[test]
fn test_reed_solomon_clone() {
    let r1 = ReedSolomon::new(10, 3).unwrap();
    let r2 = r1.clone();

    assert_eq!(r1, r2);
}

#[test]
fn test_encoding() {
    let per_shard = 50_000;

    let r = ReedSolomon::new(10, 3).unwrap();

    let mut shards = make_random_shards!(per_shard, 13);

    r.encode(&mut shards).unwrap();
    assert!(r.verify(&shards).unwrap());

    assert_eq!(
        Error::TooFewShards,
        r.encode(&mut shards[0..1]).unwrap_err()
    );

    let mut bad_shards = make_random_shards!(per_shard, 13);
    bad_shards[0] = vec![0u8];
    assert_eq!(
        Error::IncorrectShardSize,
        r.encode(&mut bad_shards).unwrap_err()
    );
}

#[test]
fn test_reconstruct_shards() {
    let per_shard = 100_000;

    let r = ReedSolomon::new(8, 5).unwrap();

    let mut shards = make_random_shards!(per_shard, 13);

    r.encode(&mut shards).unwrap();

    let master_copy = shards.clone();

    let mut shards = shards_to_option_shards(&shards);

    // Try to decode with all shards present
    r.reconstruct(&mut shards).unwrap();
    {
        let shards = option_shards_to_shards(&shards);
        assert!(r.verify(&shards).unwrap());
        assert_eq!(&shards, &master_copy);
    }

    // Try to decode with 10 shards
    shards[0] = None;
    shards[2] = None;
    //shards[4] = None;
    r.reconstruct(&mut shards).unwrap();
    {
        let shards = option_shards_to_shards(&shards);
        assert!(r.verify(&shards).unwrap());
        assert_eq!(&shards, &master_copy);
    }

    // Try to decode the same shards again to try to
    // trigger the usage of cached decode matrix
    shards[0] = None;
    shards[2] = None;
    //shards[4] = None;
    r.reconstruct(&mut shards).unwrap();
    {
        let shards = option_shards_to_shards(&shards);
        assert!(r.verify(&shards).unwrap());
        assert_eq!(&shards, &master_copy);
    }

    // Try to deocde with 6 data and 4 parity shards
    shards[0] = None;
    shards[2] = None;
    shards[12] = None;
    r.reconstruct(&mut shards).unwrap();
    {
        let shards = option_shards_to_shards(&shards);
        assert!(r.verify(&shards).unwrap());
        assert_eq!(&shards, &master_copy);
    }

    // Try to reconstruct data only
    shards[0] = None;
    shards[1] = None;
    shards[12] = None;
    r.reconstruct_data(&mut shards).unwrap();
    {
        let data_shards = option_shards_to_shards(&shards[0..8]);
        assert_eq!(master_copy[0], data_shards[0]);
        assert_eq!(master_copy[1], data_shards[1]);
        assert_eq!(None, shards[12]);
    }

    // Try to decode with 7 data and 1 parity shards
    shards[0] = None;
    shards[1] = None;
    shards[9] = None;
    shards[10] = None;
    shards[11] = None;
    shards[12] = None;
    assert_eq!(
        r.reconstruct(&mut shards).unwrap_err(),
        Error::TooFewShardsPresent
    );
}

#[test]
fn test_reconstruct() {
    let r = ReedSolomon::new(2, 2).unwrap();

    let mut shards: [[u8; 3]; 4] = [[0, 1, 2], [3, 4, 5], [200, 201, 203], [100, 101, 102]];

    {
        {
            let mut shard_refs: Vec<&mut [u8]> = Vec::with_capacity(3);

            for shard in shards.iter_mut() {
                shard_refs.push(shard);
            }

            r.encode(&mut shard_refs).unwrap();
        }

        let shard_refs: Vec<_> = shards.iter().map(|i| &i[..]).collect();
        assert!(r.verify(&shard_refs).unwrap());
    }

    {
        {
            let mut shard_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

            shard_refs[0][0] = 101;
            shard_refs[0][1] = 102;
            shard_refs[0][2] = 103;

            let shards_present = [false, true, true, true];

            let mut shards = shard_refs
                .into_iter()
                .zip(shards_present.iter().cloned())
                .collect::<Vec<_>>();

            r.reconstruct(&mut shards[..]).unwrap();
        }

        let shard_refs: Vec<_> = shards.iter().map(|i| &i[..]).collect();
        assert!(r.verify(&shard_refs).unwrap());
    }

    let expect: [[u8; 3]; 4] = [[0, 1, 2], [3, 4, 5], [6, 11, 12], [5, 14, 11]];
    assert_eq!(expect, shards);

    {
        {
            let mut shard_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

            shard_refs[0][0] = 201;
            shard_refs[0][1] = 202;
            shard_refs[0][2] = 203;

            shard_refs[2][0] = 101;
            shard_refs[2][1] = 102;
            shard_refs[2][2] = 103;

            let shards_present = [false, true, false, true];

            let mut shards = shard_refs
                .into_iter()
                .zip(shards_present.iter().cloned())
                .collect::<Vec<_>>();

            r.reconstruct_data(&mut shards[..]).unwrap();
        }

        let shard_refs = convert_2D_slices!(shards =>to_vec &[u8]);

        assert!(!r.verify(&shard_refs).unwrap());
    }

    let expect: [[u8; 3]; 4] = [[0, 1, 2], [3, 4, 5], [101, 102, 103], [5, 14, 11]];
    assert_eq!(expect, shards);

    {
        {
            let mut shard_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

            shard_refs[2][0] = 101;
            shard_refs[2][1] = 102;
            shard_refs[2][2] = 103;

            shard_refs[3][0] = 201;
            shard_refs[3][1] = 202;
            shard_refs[3][2] = 203;

            let shards_present = [true, true, false, false];

            let mut shards = shard_refs
                .into_iter()
                .zip(shards_present.iter().cloned())
                .collect::<Vec<_>>();

            r.reconstruct_data(&mut shards[..]).unwrap();
        }

        let shard_refs = convert_2D_slices!(shards =>to_vec &[u8]);

        assert!(!r.verify(&shard_refs).unwrap());
    }

    let expect: [[u8; 3]; 4] = [[0, 1, 2], [3, 4, 5], [101, 102, 103], [201, 202, 203]];
    assert_eq!(expect, shards);
}

quickcheck! {
    fn qc_encode_verify_reconstruct_verify(data: usize,
                                           parity: usize,
                                           corrupt: usize,
                                           size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let corrupt = corrupt % (parity + 1);

        let mut corrupt_pos_s = Vec::with_capacity(corrupt);
        for _ in 0..corrupt {
            let mut pos = rand::random_range(0..(data + parity));

            while corrupt_pos_s.iter().find(|&&x| x == pos).is_some() {
                pos = rand::random_range(0..(data + parity));
            }

            corrupt_pos_s.push(pos);
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [u8]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        let mut shards = expect.clone();

        // corrupt shards
        for &p in corrupt_pos_s.iter() {
            fill_random(&mut shards[p]);
        }
        let mut slice_present = vec![true; data + parity];
        for &p in corrupt_pos_s.iter() {
            slice_present[p] = false;
        }

        // reconstruct
        {
            let mut refs: Vec<_> = shards.iter_mut()
                .map(|i| &mut i[..])
                .zip(slice_present.iter().cloned())
                .collect();

            r.reconstruct(&mut refs[..]).unwrap();
        }

        ({
            let refs =
                convert_2D_slices!(expect =>to_vec &[u8]);

            r.verify(&refs).unwrap()
        })
            &&
            expect == shards
            &&
            ({
                let refs =
                    convert_2D_slices!(shards =>to_vec &[u8]);

                r.verify(&refs).unwrap()
            })
    }

    fn qc_encode_verify_reconstruct_verify_shards(data: usize,
                                                  parity: usize,
                                                  corrupt: usize,
                                                  size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let corrupt = corrupt % (parity + 1);

        let mut corrupt_pos_s = Vec::with_capacity(corrupt);
        for _ in 0..corrupt {
            let mut pos = rand::random_range(0..(data + parity));

            while corrupt_pos_s.iter().find(|&&x| x == pos).is_some() {
                pos = rand::random_range(0..(data + parity));
            }

            corrupt_pos_s.push(pos);
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        r.encode(&mut expect).unwrap();

        let expect = expect;

        let mut shards = shards_into_option_shards(expect.clone());

        // corrupt shards
        for &p in corrupt_pos_s.iter() {
            shards[p] = None;
        }

        // reconstruct
        r.reconstruct(&mut shards).unwrap();

        let shards = option_shards_into_shards(shards);

        r.verify(&expect).unwrap()
            && expect == shards
            && r.verify(&shards).unwrap()
    }

    fn qc_verify(data: usize,
                 parity: usize,
                 corrupt: usize,
                 size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let corrupt = corrupt % (parity + 1);

        let mut corrupt_pos_s = Vec::with_capacity(corrupt);
        for _ in 0..corrupt {
            let mut pos = rand::random_range(0..(data + parity));

            while corrupt_pos_s.iter().find(|&&x| x == pos).is_some() {
                pos = rand::random_range(0..(data + parity));
            }

            corrupt_pos_s.push(pos);
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [u8]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        let mut shards = expect.clone();

        // corrupt shards
        for &p in corrupt_pos_s.iter() {
            fill_random(&mut shards[p]);
        }

        ({
            let refs =
                convert_2D_slices!(expect =>to_vec &[u8]);

            r.verify(&refs).unwrap()
        })
            &&
            ((corrupt > 0 && expect != shards)
             || (corrupt == 0 && expect == shards))
            &&
            ({
                let refs =
                    convert_2D_slices!(shards =>to_vec &[u8]);

                (corrupt > 0 && !r.verify(&refs).unwrap())
                    || (corrupt == 0 && r.verify(&refs).unwrap())
            })
    }

    fn qc_verify_shards(data: usize,
                        parity: usize,
                        corrupt: usize,
                        size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let corrupt = corrupt % (parity + 1);

        let mut corrupt_pos_s = Vec::with_capacity(corrupt);
        for _ in 0..corrupt {
            let mut pos = rand::random_range(0..(data + parity));

            while corrupt_pos_s.iter().find(|&&x| x == pos).is_some() {
                pos = rand::random_range(0..(data + parity));
            }

            corrupt_pos_s.push(pos);
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        r.encode(&mut expect).unwrap();

        let expect = expect;

        let mut shards = expect.clone();

        // corrupt shards
        for &p in corrupt_pos_s.iter() {
            fill_random(&mut shards[p]);
        }

        r.verify(&expect).unwrap()
            &&
            ((corrupt > 0 && expect != shards)
             || (corrupt == 0 && expect == shards))
            &&
            ((corrupt > 0 && !r.verify(&shards).unwrap())
             || (corrupt == 0 && r.verify(&shards).unwrap()))
    }

    fn qc_encode_sep_same_as_encode(data: usize,
                                    parity: usize,
                                    size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [u8]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        {
            let (data, parity) = shards.split_at_mut(data);

            let data_refs =
                convert_2D_slices!(data =>to_mut_vec &[u8]);

            let mut parity_refs =
                convert_2D_slices!(parity =>to_mut_vec &mut [u8]);

            r.encode_sep(&data_refs, &mut parity_refs).unwrap();
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_sep_same_as_encode_shards(data: usize,
                                           parity: usize,
                                           size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        r.encode(&mut expect).unwrap();

        let expect = expect;

        {
            let (data, parity) = shards.split_at_mut(data);

            r.encode_sep(data, parity).unwrap();
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_single_same_as_encode(data: usize,
                                       parity: usize,
                                       size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [u8]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        {
            let mut refs =
                convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

            for i in 0..data {
                r.encode_single(i, &mut refs).unwrap();
            }
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_single_same_as_encode_shards(data: usize,
                                              parity: usize,
                                              size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        r.encode(&mut expect).unwrap();

        let expect = expect;

        for i in 0..data {
            r.encode_single(i, &mut shards).unwrap();
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_single_sep_same_as_encode(data: usize,
                                           parity: usize,
                                           size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [u8]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        {
            let (data_shards, parity_shards) = shards.split_at_mut(data);

            let data_refs =
                convert_2D_slices!(data_shards =>to_mut_vec &[u8]);

            let mut parity_refs =
                convert_2D_slices!(parity_shards =>to_mut_vec &mut [u8]);

            for (i, shard) in data_refs.iter().enumerate().take(data) {
                r.encode_single_sep(i, shard, &mut parity_refs).unwrap();
            }
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_single_sep_same_as_encode_shards(data: usize,
                                                  parity: usize,
                                                  size: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        r.encode(&mut expect).unwrap();

        let expect = expect;

        {
            let (data_shards, parity_shards) = shards.split_at_mut(data);

            for (i, shard) in data_shards.iter().enumerate().take(data) {
                r.encode_single_sep(i, shard, parity_shards).unwrap();
            }
        }

        let shards = shards;

        expect == shards
    }
}

#[test]
fn test_reconstruct_error_handling() {
    let r = ReedSolomon::new(2, 2).unwrap();

    let mut shards: [[u8; 3]; 4] = [[0, 1, 2], [3, 4, 5], [200, 201, 203], [100, 101, 102]];

    {
        let mut shard_refs: Vec<&mut [u8]> = Vec::with_capacity(3);

        for shard in shards.iter_mut() {
            shard_refs.push(shard);
        }

        r.encode(&mut shard_refs).unwrap();
    }

    {
        let mut shard_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

        shard_refs[0][0] = 101;
        shard_refs[0][1] = 102;
        shard_refs[0][2] = 103;

        let shards_present = [true, false, false, false];

        let mut shard_refs: Vec<_> = shard_refs
            .into_iter()
            .zip(shards_present.iter().cloned())
            .collect();

        assert_eq!(
            Error::TooFewShardsPresent,
            r.reconstruct(&mut shard_refs[..]).unwrap_err()
        );

        shard_refs[3].1 = true;
        r.reconstruct(&mut shard_refs).unwrap();
    }
}

#[test]
fn test_one_encode() {
    let r = ReedSolomon::new(5, 5).unwrap();

    let mut shards = shards!(
        [0, 1],
        [4, 5],
        [2, 3],
        [6, 7],
        [8, 9],
        [0, 0],
        [0, 0],
        [0, 0],
        [0, 0],
        [0, 0]
    );

    r.encode(&mut shards).unwrap();
    {
        assert_eq!(shards[5][0], 12);
        assert_eq!(shards[5][1], 13);
    }
    {
        assert_eq!(shards[6][0], 10);
        assert_eq!(shards[6][1], 11);
    }
    {
        assert_eq!(shards[7][0], 14);
        assert_eq!(shards[7][1], 15);
    }
    {
        assert_eq!(shards[8][0], 90);
        assert_eq!(shards[8][1], 91);
    }
    {
        assert_eq!(shards[9][0], 94);
        assert_eq!(shards[9][1], 95);
    }

    assert!(r.verify(&shards).unwrap());

    shards[8][0] += 1;
    assert!(!r.verify(&shards).unwrap());
}

#[test]
fn test_verify_too_few_shards() {
    let r = ReedSolomon::new(3, 2).unwrap();

    let shards = make_random_shards!(10, 4);

    assert_eq!(Error::TooFewShards, r.verify(&shards).unwrap_err());
}

#[test]
fn test_verify_shards_with_buffer_incorrect_buffer_sizes() {
    let r = ReedSolomon::new(3, 2).unwrap();

    {
        // Test too few slices in buffer
        let shards = make_random_shards!(100, 5);

        let mut buffer = vec![vec![0; 100]; 1];

        assert_eq!(
            Error::TooFewBufferShards,
            r.verify_with_buffer(&shards, &mut buffer).unwrap_err()
        );
    }
    {
        // Test too many slices in buffer
        let shards = make_random_shards!(100, 5);

        let mut buffer = vec![vec![0; 100]; 3];

        assert_eq!(
            Error::TooManyBufferShards,
            r.verify_with_buffer(&shards, &mut buffer).unwrap_err()
        );
    }
    {
        // Test correct number of slices in buffer
        let mut shards = make_random_shards!(100, 5);

        r.encode(&mut shards).unwrap();

        let mut buffer = vec![vec![0; 100]; 2];

        assert!(r.verify_with_buffer(&shards, &mut buffer).unwrap());
    }
    {
        // Test having first buffer being empty
        let shards = make_random_shards!(100, 5);

        let mut buffer = vec![vec![0; 100]; 2];
        buffer[0] = vec![];

        assert_eq!(
            Error::EmptyShard,
            r.verify_with_buffer(&shards, &mut buffer).unwrap_err()
        );
    }
    {
        // Test having shards of inconsistent length in buffer
        let shards = make_random_shards!(100, 5);

        let mut buffer = vec![vec![0; 100]; 2];
        buffer[1] = vec![0; 99];

        assert_eq!(
            Error::IncorrectShardSize,
            r.verify_with_buffer(&shards, &mut buffer).unwrap_err()
        );
    }
}

#[test]
fn test_verify_shards_with_buffer_gives_correct_parity_shards() {
    let r = ReedSolomon::new(10, 3).unwrap();

    for _ in 0..100 {
        let mut shards = make_random_shards!(100, 13);
        let shards_copy = shards.clone();

        r.encode(&mut shards).unwrap();

        {
            let mut buffer = make_random_shards!(100, 3);

            assert!(!r.verify_with_buffer(&shards_copy, &mut buffer).unwrap());

            assert_eq_shards(&shards[10..], &buffer);
        }
        {
            let mut buffer = make_random_shards!(100, 3);

            assert!(r.verify_with_buffer(&shards, &mut buffer).unwrap());

            assert_eq_shards(&shards[10..], &buffer);
        }
    }
}

#[test]
fn test_verify_with_buffer_gives_correct_parity_shards() {
    let r = ReedSolomon::new(10, 3).unwrap();

    for _ in 0..100 {
        let mut slices: [[u8; 100]; 13] = [[0; 100]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }
        let slices_copy = slices;

        {
            let mut slice_refs = convert_2D_slices!(slices=>to_mut_vec &mut [u8]);

            r.encode(&mut slice_refs).unwrap();
        }

        {
            let mut buffer: [[u8; 100]; 3] = [[0; 100]; 3];

            {
                let slice_copy_refs = convert_2D_slices!(slices_copy =>to_vec &[u8]);

                for slice in buffer.iter_mut() {
                    fill_random(slice);
                }

                let mut buffer_refs = convert_2D_slices!(buffer =>to_mut_vec &mut [u8]);

                assert!(
                    !r.verify_with_buffer(&slice_copy_refs, &mut buffer_refs)
                        .unwrap()
                );
            }

            for a in 0..3 {
                for b in 0..100 {
                    assert_eq!(slices[10 + a][b], buffer[a][b]);
                }
            }
        }

        {
            let mut buffer: [[u8; 100]; 3] = [[0; 100]; 3];

            {
                let slice_refs = convert_2D_slices!(slices=>to_vec &[u8]);

                for slice in buffer.iter_mut() {
                    fill_random(slice);
                }

                let mut buffer_refs = convert_2D_slices!(buffer =>to_mut_vec &mut [u8]);

                assert!(r.verify_with_buffer(&slice_refs, &mut buffer_refs).unwrap());
            }

            for a in 0..3 {
                for b in 0..100 {
                    assert_eq!(slices[10 + a][b], buffer[a][b]);
                }
            }
        }
    }
}

#[test]
fn test_slices_or_shards_count_check() {
    let r = ReedSolomon::new(3, 2).unwrap();

    {
        let mut shards = make_random_shards!(10, 4);

        assert_eq!(Error::TooFewShards, r.encode(&mut shards).unwrap_err());
        assert_eq!(Error::TooFewShards, r.verify(&shards).unwrap_err());

        let mut option_shards = shards_to_option_shards(&shards);

        assert_eq!(
            Error::TooFewShards,
            r.reconstruct(&mut option_shards).unwrap_err()
        );
    }
    {
        let mut shards = make_random_shards!(10, 6);

        assert_eq!(Error::TooManyShards, r.encode(&mut shards).unwrap_err());
        assert_eq!(Error::TooManyShards, r.verify(&shards).unwrap_err());

        let mut option_shards = shards_to_option_shards(&shards);

        assert_eq!(
            Error::TooManyShards,
            r.reconstruct(&mut option_shards).unwrap_err()
        );
    }
}

#[test]
fn test_check_slices_or_shards_size() {
    let r = ReedSolomon::new(2, 2).unwrap();

    {
        let mut shards = shards!([0, 0, 0], [0, 1], [1, 2, 3], [0, 0, 0]);

        assert_eq!(
            Error::IncorrectShardSize,
            r.encode(&mut shards).unwrap_err()
        );
        assert_eq!(Error::IncorrectShardSize, r.verify(&shards).unwrap_err());

        let mut option_shards = shards_to_option_shards(&shards);

        assert_eq!(
            Error::IncorrectShardSize,
            r.reconstruct(&mut option_shards).unwrap_err()
        );
    }
    {
        let mut shards = shards!([0, 1], [0, 1], [1, 2, 3], [0, 0, 0]);

        assert_eq!(
            Error::IncorrectShardSize,
            r.encode(&mut shards).unwrap_err()
        );
        assert_eq!(Error::IncorrectShardSize, r.verify(&shards).unwrap_err());

        let mut option_shards = shards_to_option_shards(&shards);

        assert_eq!(
            Error::IncorrectShardSize,
            r.reconstruct(&mut option_shards).unwrap_err()
        );
    }
    {
        let mut shards = shards!([0, 1], [0, 1, 4], [1, 2, 3], [0, 0, 0]);

        assert_eq!(
            Error::IncorrectShardSize,
            r.encode(&mut shards).unwrap_err()
        );
        assert_eq!(Error::IncorrectShardSize, r.verify(&shards).unwrap_err());

        let mut option_shards = shards_to_option_shards(&shards);

        assert_eq!(
            Error::IncorrectShardSize,
            r.reconstruct(&mut option_shards).unwrap_err()
        );
    }
    {
        let mut shards = shards!([], [0, 1, 3], [1, 2, 3], [0, 0, 0]);

        assert_eq!(Error::EmptyShard, r.encode(&mut shards).unwrap_err());
        assert_eq!(Error::EmptyShard, r.verify(&shards).unwrap_err());

        let mut option_shards = shards_to_option_shards(&shards);

        assert_eq!(
            Error::EmptyShard,
            r.reconstruct(&mut option_shards).unwrap_err()
        );
    }
    {
        let mut option_shards: Vec<Option<Vec<u8>>> = vec![None, None, None, None];

        assert_eq!(
            Error::TooFewShardsPresent,
            r.reconstruct(&mut option_shards).unwrap_err()
        );
    }
}

#[test]
fn shardbyshard_encode_correctly() {
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(10_000, 13);
        let mut shards_copy = shards.clone();

        r.encode(&mut shards).unwrap();

        for i in 0..10 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode(&mut shards_copy).unwrap();
        }

        assert!(sbs.parity_ready());

        assert_eq!(shards, shards_copy);

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut slices: [[u8; 100]; 13] = [[0; 100]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }
        let mut slices_copy = slices;

        {
            let mut slice_refs = convert_2D_slices!(slices=>to_mut_vec &mut [u8]);
            let mut slice_copy_refs = convert_2D_slices!(slices_copy =>to_mut_vec &mut [u8]);

            r.encode(&mut slice_refs).unwrap();

            for i in 0..10 {
                assert_eq!(i, sbs.cur_input_index());

                sbs.encode(&mut slice_copy_refs).unwrap();
            }
        }

        assert!(sbs.parity_ready());

        for a in 0..13 {
            for b in 0..100 {
                assert_eq!(slices[a][b], slices_copy[a][b]);
            }
        }

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
}

quickcheck! {
    fn qc_shardbyshard_encode_same_as_encode(data: usize,
                                             parity: usize,
                                             size: usize,
                                             reuse: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let reuse = reuse % 10;

        let r = ReedSolomon::new(data, parity).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        for _ in 0..1 + reuse {
            {
                let mut refs =
                    convert_2D_slices!(expect =>to_mut_vec &mut [u8]);

                r.encode(&mut refs).unwrap();
            }

            {
                let mut slice_refs =
                    convert_2D_slices!(shards=>to_mut_vec &mut [u8]);

                for i in 0..data {
                    assert_eq!(i, sbs.cur_input_index());

                    sbs.encode(&mut slice_refs).unwrap();
                }
            }

            if !(expect == shards
                 && sbs.parity_ready()
                 && sbs.cur_input_index() == data
                 && { sbs.reset().unwrap(); !sbs.parity_ready() && sbs.cur_input_index() == 0 }) {
                return false;
            }
        }

        return true;
    }

    fn qc_shardbyshard_encode_same_as_encode_shards(data: usize,
                                                    parity: usize,
                                                    size: usize,
                                                    reuse: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let reuse = reuse % 10;

        let r = ReedSolomon::new(data, parity).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        r.encode(&mut expect).unwrap();

        for _ in 0..1 + reuse {
            for i in 0..data {
                assert_eq!(i, sbs.cur_input_index());

                sbs.encode(&mut shards).unwrap();
            }

            if !(expect == shards
                 && sbs.parity_ready()
                 && sbs.cur_input_index() == data
                 && { sbs.reset().unwrap(); !sbs.parity_ready() && sbs.cur_input_index() == 0 }) {
                return false;
            }
        }

        return true;
    }
}

#[test]
fn shardbyshard_encode_sep_correctly() {
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(10_000, 13);
        let mut shards_copy = shards.clone();

        let (data, parity) = shards.split_at_mut(10);
        let (data_copy, parity_copy) = shards_copy.split_at_mut(10);

        r.encode_sep(data, parity).unwrap();

        for i in 0..10 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode_sep(data_copy, parity_copy).unwrap();
        }

        assert!(sbs.parity_ready());

        assert_eq!(parity, parity_copy);

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut slices: [[u8; 100]; 13] = [[0; 100]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }
        let mut slices_copy = slices;

        {
            let (data, parity) = slices.split_at_mut(10);
            let (data_copy, parity_copy) = slices_copy.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);
            let data_copy_refs = convert_2D_slices!(data_copy =>to_mut_vec &[u8]);
            let mut parity_copy_refs = convert_2D_slices!(parity_copy =>to_mut_vec &mut [u8]);

            r.encode_sep(&data_refs, &mut parity_refs).unwrap();

            for i in 0..10 {
                assert_eq!(i, sbs.cur_input_index());

                sbs.encode_sep(&data_copy_refs, &mut parity_copy_refs)
                    .unwrap();
            }
        }

        assert!(sbs.parity_ready());

        for a in 0..13 {
            for b in 0..100 {
                assert_eq!(slices[a][b], slices_copy[a][b]);
            }
        }

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
}

quickcheck! {
    fn qc_shardbyshard_encode_sep_same_as_encode(data: usize,
                                                 parity: usize,
                                                 size: usize,
                                                 reuse: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let reuse = reuse % 10;

        let r = ReedSolomon::new(data, parity).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        for _ in 0..1 + reuse {
            {
                let (data_shards, parity_shards) =
                    expect.split_at_mut(data);

                let data_refs =
                    convert_2D_slices!(data_shards =>to_mut_vec &[u8]);
                let mut parity_refs =
                    convert_2D_slices!(parity_shards =>to_mut_vec &mut [u8]);

                r.encode_sep(&data_refs, &mut parity_refs).unwrap();
            }

            {
                let (data_shards, parity_shards) =
                    shards.split_at_mut(data);
                let data_refs =
                    convert_2D_slices!(data_shards =>to_mut_vec &[u8]);
                let mut parity_refs =
                    convert_2D_slices!(parity_shards =>to_mut_vec &mut [u8]);

                for i in 0..data {
                    assert_eq!(i, sbs.cur_input_index());

                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap();
                }
            }

            if !(expect == shards
                 && sbs.parity_ready()
                 && sbs.cur_input_index() == data
                 && { sbs.reset().unwrap(); !sbs.parity_ready() && sbs.cur_input_index() == 0 }) {
                return false;
            }
        }

        return true;
    }

    fn qc_shardbyshard_encode_sep_same_as_encode_shards(data: usize,
                                                        parity: usize,
                                                        size: usize,
                                                        reuse: usize) -> bool {
        let data = 1 + data % 255;
        let mut parity = 1 + parity % 255;
        if data + parity > 256 {
            parity -= data + parity - 256;
        }

        let size = 1 + size % 1_000_000;

        let reuse = reuse % 10;

        let r = ReedSolomon::new(data, parity).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        for _ in 0..1 + reuse {
            {
                let (data_shards, parity_shards) =
                    expect.split_at_mut(data);

                r.encode_sep(data_shards, parity_shards).unwrap();
            }

            {
                let (data_shards, parity_shards) =
                    shards.split_at_mut(data);

                for i in 0..data {
                    assert_eq!(i, sbs.cur_input_index());

                    sbs.encode_sep(data_shards, parity_shards).unwrap();
                }
            }

            if !(expect == shards
                 && sbs.parity_ready()
                 && sbs.cur_input_index() == data
                 && { sbs.reset().unwrap(); !sbs.parity_ready() && sbs.cur_input_index() == 0 }) {
                return false;
            }
        }

        return true;
    }
}

#[test]
fn shardbyshard_encode_correctly_more_rigorous() {
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(10_000, 13);
        let mut shards_copy = make_random_shards!(10_000, 13);

        r.encode(&mut shards).unwrap();

        for i in 0..10 {
            assert_eq!(i, sbs.cur_input_index());

            shards_copy[i].clone_from_slice(&shards[i]);
            sbs.encode(&mut shards_copy).unwrap();
            fill_random(&mut shards_copy[i]);
        }

        assert!(sbs.parity_ready());

        for i in 0..10 {
            shards_copy[i].clone_from_slice(&shards[i]);
        }

        assert_eq!(shards, shards_copy);

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut slices: [[u8; 100]; 13] = [[0; 100]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }

        let mut slices_copy: [[u8; 100]; 13] = [[0; 100]; 13];
        for slice in slices_copy.iter_mut() {
            fill_random(slice);
        }

        {
            let mut slice_refs = convert_2D_slices!(slices=>to_mut_vec &mut [u8]);
            let mut slice_copy_refs = convert_2D_slices!(slices_copy =>to_mut_vec &mut [u8]);

            r.encode(&mut slice_refs).unwrap();

            for i in 0..10 {
                assert_eq!(i, sbs.cur_input_index());

                slice_copy_refs[i].clone_from_slice(slice_refs[i]);
                sbs.encode(&mut slice_copy_refs).unwrap();
                fill_random(slice_copy_refs[i]);
            }
        }

        for i in 0..10 {
            slices_copy[i].clone_from_slice(&slices[i]);
        }

        assert!(sbs.parity_ready());

        for a in 0..13 {
            for b in 0..100 {
                assert_eq!(slices[a][b], slices_copy[a][b]);
            }
        }

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
}

#[test]
fn shardbyshard_encode_error_handling() {
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(10_000, 13);

        let mut slice_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

        for i in 0..10 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode(&mut slice_refs).unwrap();
        }

        assert!(sbs.parity_ready());

        assert_eq!(
            SBSError::TooManyCalls,
            sbs.encode(&mut slice_refs).unwrap_err()
        );

        sbs.reset().unwrap();

        for i in 0..1 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode(&mut slice_refs).unwrap();
        }

        assert_eq!(SBSError::LeftoverShards, sbs.reset().unwrap_err());

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(100, 13);
        shards[0] = vec![];
        {
            let mut slice_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

            assert_eq!(0, sbs.cur_input_index());

            assert_eq!(
                SBSError::RSError(Error::EmptyShard),
                sbs.encode(&mut slice_refs).unwrap_err()
            );

            assert_eq!(0, sbs.cur_input_index());

            assert_eq!(
                SBSError::RSError(Error::EmptyShard),
                sbs.encode(&mut slice_refs).unwrap_err()
            );

            assert_eq!(0, sbs.cur_input_index());
        }

        shards[0] = vec![0; 100];

        let mut slice_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

        sbs.encode(&mut slice_refs).unwrap();

        assert_eq!(1, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(100, 13);
        shards[1] = vec![0; 99];
        {
            let mut slice_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

            assert_eq!(0, sbs.cur_input_index());

            assert_eq!(
                SBSError::RSError(Error::IncorrectShardSize),
                sbs.encode(&mut slice_refs).unwrap_err()
            );

            assert_eq!(0, sbs.cur_input_index());

            assert_eq!(
                SBSError::RSError(Error::IncorrectShardSize),
                sbs.encode(&mut slice_refs).unwrap_err()
            );

            assert_eq!(0, sbs.cur_input_index());
        }

        shards[1] = vec![0; 100];

        let mut slice_refs = convert_2D_slices!(shards =>to_mut_vec &mut [u8]);

        sbs.encode(&mut slice_refs).unwrap();

        assert_eq!(1, sbs.cur_input_index());
    }
}

#[test]
fn shardbyshard_encode_shard_error_handling() {
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(10_000, 13);

        for i in 0..10 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode(&mut shards).unwrap();
        }

        assert!(sbs.parity_ready());

        assert_eq!(SBSError::TooManyCalls, sbs.encode(&mut shards).unwrap_err());

        sbs.reset().unwrap();

        for i in 0..1 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode(&mut shards).unwrap();
        }

        assert_eq!(SBSError::LeftoverShards, sbs.reset().unwrap_err());

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(100, 13);
        shards[0] = vec![];
        {
            assert_eq!(0, sbs.cur_input_index());

            assert_eq!(
                SBSError::RSError(Error::EmptyShard),
                sbs.encode(&mut shards).unwrap_err()
            );

            assert_eq!(0, sbs.cur_input_index());

            assert_eq!(
                SBSError::RSError(Error::EmptyShard),
                sbs.encode(&mut shards).unwrap_err()
            );

            assert_eq!(0, sbs.cur_input_index());
        }

        shards[0] = vec![0; 100];

        sbs.encode(&mut shards).unwrap();

        assert_eq!(1, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(100, 13);
        shards[1] = vec![0; 99];
        {
            assert_eq!(0, sbs.cur_input_index());

            assert_eq!(
                SBSError::RSError(Error::IncorrectShardSize),
                sbs.encode(&mut shards).unwrap_err()
            );

            assert_eq!(0, sbs.cur_input_index());

            assert_eq!(
                SBSError::RSError(Error::IncorrectShardSize),
                sbs.encode(&mut shards).unwrap_err()
            );

            assert_eq!(0, sbs.cur_input_index());
        }

        shards[1] = vec![0; 100];

        sbs.encode(&mut shards).unwrap();

        assert_eq!(1, sbs.cur_input_index());
    }
}

#[test]
fn shardbyshard_encode_sep_error_handling() {
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(10_000, 13);

        let (data, parity) = shards.split_at_mut(10);

        for i in 0..10 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode_sep(data, parity).unwrap();
        }

        assert!(sbs.parity_ready());

        assert_eq!(
            SBSError::TooManyCalls,
            sbs.encode_sep(data, parity).unwrap_err()
        );

        sbs.reset().unwrap();

        for i in 0..1 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode_sep(data, parity).unwrap();
        }

        assert_eq!(SBSError::LeftoverShards, sbs.reset().unwrap_err());

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut slices: [[u8; 100]; 13] = [[0; 100]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }
        {
            let (data, parity) = slices.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            for i in 0..10 {
                assert_eq!(i, sbs.cur_input_index());

                sbs.encode_sep(&data_refs, &mut parity_refs).unwrap();
            }

            assert!(sbs.parity_ready());

            assert_eq!(
                SBSError::TooManyCalls,
                sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
            );

            sbs.reset().unwrap();

            for i in 0..1 {
                assert_eq!(i, sbs.cur_input_index());

                sbs.encode_sep(&data_refs, &mut parity_refs).unwrap();
            }
        }

        assert_eq!(SBSError::LeftoverShards, sbs.reset().unwrap_err());

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();

        {
            let mut sbs = ShardByShard::new(&r);

            let mut shards = make_random_shards!(100, 13);
            shards[0] = vec![];

            {
                let (data, parity) = shards.split_at_mut(10);

                let data_refs = convert_2D_slices!(data=>to_vec &[u8]);
                let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::EmptyShard),
                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::EmptyShard),
                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());
            }

            shards[0] = vec![0; 100];

            let (data, parity) = shards.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            sbs.encode_sep(&data_refs, &mut parity_refs).unwrap();

            assert_eq!(1, sbs.cur_input_index());
        }
        {
            let mut sbs = ShardByShard::new(&r);

            let mut shards = make_random_shards!(100, 13);
            shards[10] = vec![];
            {
                let (data, parity) = shards.split_at_mut(10);

                let data_refs = convert_2D_slices!(data=>to_vec &[u8]);
                let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::EmptyShard),
                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::EmptyShard),
                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());
            }

            shards[10] = vec![0; 100];

            let (data, parity) = shards.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            sbs.encode_sep(&data_refs, &mut parity_refs).unwrap();

            assert_eq!(1, sbs.cur_input_index());
        }
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        {
            let mut sbs = ShardByShard::new(&r);

            let mut shards = make_random_shards!(100, 13);
            shards[1] = vec![0; 99];
            {
                let (data, parity) = shards.split_at_mut(10);

                let data_refs = convert_2D_slices!(data=>to_vec &[u8]);
                let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::IncorrectShardSize),
                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::IncorrectShardSize),
                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());
            }

            shards[1] = vec![0; 100];

            let (data, parity) = shards.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            sbs.encode_sep(&data_refs, &mut parity_refs).unwrap();

            assert_eq!(1, sbs.cur_input_index());
        }
        {
            let mut sbs = ShardByShard::new(&r);

            let mut shards = make_random_shards!(100, 13);
            shards[11] = vec![0; 99];
            {
                let (data, parity) = shards.split_at_mut(10);

                let data_refs = convert_2D_slices!(data=>to_vec &[u8]);
                let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::IncorrectShardSize),
                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::IncorrectShardSize),
                    sbs.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());
            }

            shards[11] = vec![0; 100];

            let (data, parity) = shards.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            sbs.encode_sep(&data_refs, &mut parity_refs).unwrap();

            assert_eq!(1, sbs.cur_input_index());
        }
    }
}

#[test]
fn shardbyshard_encode_shard_sep_error_handling() {
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        let mut sbs = ShardByShard::new(&r);

        let mut shards = make_random_shards!(10_000, 13);

        let (data, parity) = shards.split_at_mut(10);

        for i in 0..10 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode_sep(data, parity).unwrap();
        }

        assert!(sbs.parity_ready());

        assert_eq!(
            SBSError::TooManyCalls,
            sbs.encode_sep(data, parity).unwrap_err()
        );

        sbs.reset().unwrap();

        for i in 0..1 {
            assert_eq!(i, sbs.cur_input_index());

            sbs.encode_sep(data, parity).unwrap();
        }

        assert_eq!(SBSError::LeftoverShards, sbs.reset().unwrap_err());

        sbs.reset_force();

        assert_eq!(0, sbs.cur_input_index());
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();

        {
            let mut sbs = ShardByShard::new(&r);

            let mut shards = make_random_shards!(100, 13);
            shards[0] = vec![];

            {
                let (data, parity) = shards.split_at_mut(10);

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::EmptyShard),
                    sbs.encode_sep(data, parity).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::EmptyShard),
                    sbs.encode_sep(data, parity).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());
            }

            shards[0] = vec![0; 100];

            let (data, parity) = shards.split_at_mut(10);

            sbs.encode_sep(data, parity).unwrap();

            assert_eq!(1, sbs.cur_input_index());
        }
        {
            let mut sbs = ShardByShard::new(&r);

            let mut shards = make_random_shards!(100, 13);
            shards[10] = vec![];
            {
                let (data, parity) = shards.split_at_mut(10);

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::EmptyShard),
                    sbs.encode_sep(data, parity).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::EmptyShard),
                    sbs.encode_sep(data, parity).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());
            }

            shards[10] = vec![0; 100];

            let (data, parity) = shards.split_at_mut(10);

            sbs.encode_sep(data, parity).unwrap();

            assert_eq!(1, sbs.cur_input_index());
        }
    }
    {
        let r = ReedSolomon::new(10, 3).unwrap();
        {
            let mut sbs = ShardByShard::new(&r);

            let mut shards = make_random_shards!(100, 13);
            shards[1] = vec![0; 99];
            {
                let (data, parity) = shards.split_at_mut(10);

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::IncorrectShardSize),
                    sbs.encode_sep(data, parity).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::IncorrectShardSize),
                    sbs.encode_sep(data, parity).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());
            }

            shards[1] = vec![0; 100];

            let (data, parity) = shards.split_at_mut(10);

            sbs.encode_sep(data, parity).unwrap();

            assert_eq!(1, sbs.cur_input_index());
        }
        {
            let mut sbs = ShardByShard::new(&r);

            let mut shards = make_random_shards!(100, 13);
            shards[11] = vec![0; 99];
            {
                let (data, parity) = shards.split_at_mut(10);

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::IncorrectShardSize),
                    sbs.encode_sep(data, parity).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());

                assert_eq!(
                    SBSError::RSError(Error::IncorrectShardSize),
                    sbs.encode_sep(data, parity).unwrap_err()
                );

                assert_eq!(0, sbs.cur_input_index());
            }

            shards[11] = vec![0; 100];

            let (data, parity) = shards.split_at_mut(10);

            sbs.encode_sep(data, parity).unwrap();

            assert_eq!(1, sbs.cur_input_index());
        }
    }
}

#[test]
fn test_encode_single_sep() {
    let r = ReedSolomon::new(10, 3).unwrap();

    {
        let mut shards = make_random_shards!(10, 13);
        let mut shards_copy = shards.clone();

        r.encode(&mut shards).unwrap();

        {
            let (data, parity) = shards_copy.split_at_mut(10);

            for (i, shard) in data.iter().enumerate().take(10) {
                r.encode_single_sep(i, shard, parity).unwrap();
            }
        }
        assert!(r.verify(&shards).unwrap());
        assert!(r.verify(&shards_copy).unwrap());

        assert_eq_shards(&shards, &shards_copy);
    }
    {
        let mut slices: [[u8; 100]; 13] = [[0; 100]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }
        let mut slices_copy = slices;

        {
            let mut slice_refs = convert_2D_slices!(slices=>to_mut_vec &mut [u8]);

            let (data_copy, parity_copy) = slices_copy.split_at_mut(10);

            let data_copy_refs = convert_2D_slices!(data_copy =>to_mut_vec &[u8]);
            let mut parity_copy_refs = convert_2D_slices!(parity_copy =>to_mut_vec &mut [u8]);

            r.encode(&mut slice_refs).unwrap();

            for (i, shard) in data_copy_refs.iter().enumerate().take(10) {
                r.encode_single_sep(i, shard, &mut parity_copy_refs)
                    .unwrap();
            }
        }

        for a in 0..13 {
            for b in 0..100 {
                assert_eq!(slices[a][b], slices_copy[a][b]);
            }
        }
    }
}

#[test]
fn test_encode_sep() {
    let r = ReedSolomon::new(10, 3).unwrap();

    {
        let mut shards = make_random_shards!(10_000, 13);
        let mut shards_copy = shards.clone();

        r.encode(&mut shards).unwrap();

        {
            let (data, parity) = shards_copy.split_at_mut(10);

            r.encode_sep(data, parity).unwrap();
        }

        assert_eq_shards(&shards, &shards_copy);
    }
    {
        let mut slices: [[u8; 100]; 13] = [[0; 100]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }
        let mut slices_copy = slices;

        {
            let (data_copy, parity_copy) = slices_copy.split_at_mut(10);

            let mut slice_refs = convert_2D_slices!(slices =>to_mut_vec &mut [u8]);
            let data_copy_refs = convert_2D_slices!(data_copy =>to_mut_vec &[u8]);
            let mut parity_copy_refs = convert_2D_slices!(parity_copy =>to_mut_vec &mut [u8]);

            r.encode(&mut slice_refs).unwrap();

            r.encode_sep(&data_copy_refs, &mut parity_copy_refs)
                .unwrap();
        }

        for a in 0..13 {
            for b in 0..100 {
                assert_eq!(slices[a][b], slices_copy[a][b]);
            }
        }
    }
}

#[test]
fn test_encode_single_sep_error_handling() {
    let r = ReedSolomon::new(10, 3).unwrap();

    {
        let mut shards = make_random_shards!(1000, 13);

        {
            let (data, parity) = shards.split_at_mut(10);

            for (i, shard) in data.iter().enumerate().take(10) {
                r.encode_single_sep(i, shard, parity).unwrap();
            }

            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(10, &data[0], parity).unwrap_err()
            );
            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(11, &data[0], parity).unwrap_err()
            );
            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(12, &data[0], parity).unwrap_err()
            );
            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(13, &data[0], parity).unwrap_err()
            );
            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(14, &data[0], parity).unwrap_err()
            );
        }

        {
            let (data, parity) = shards.split_at_mut(11);

            assert_eq!(
                Error::TooFewParityShards,
                r.encode_single_sep(0, &data[0], parity).unwrap_err()
            );
        }
        {
            let (data, parity) = shards.split_at_mut(9);

            assert_eq!(
                Error::TooManyParityShards,
                r.encode_single_sep(0, &data[0], parity).unwrap_err()
            );
        }
    }
    {
        let mut slices: [[u8; 1000]; 13] = [[0; 1000]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }

        {
            let (data, parity) = slices.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            for (i, shard) in data_refs.iter().enumerate().take(10) {
                r.encode_single_sep(i, shard, &mut parity_refs)
                    .unwrap();
            }

            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(10, data_refs[0], &mut parity_refs)
                    .unwrap_err()
            );
            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(11, data_refs[0], &mut parity_refs)
                    .unwrap_err()
            );
            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(12, data_refs[0], &mut parity_refs)
                    .unwrap_err()
            );
            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(13, data_refs[0], &mut parity_refs)
                    .unwrap_err()
            );
            assert_eq!(
                Error::InvalidIndex,
                r.encode_single_sep(14, data_refs[0], &mut parity_refs)
                    .unwrap_err()
            );
        }
        {
            let (data, parity) = slices.split_at_mut(11);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            assert_eq!(
                Error::TooFewParityShards,
                r.encode_single_sep(0, data_refs[0], &mut parity_refs)
                    .unwrap_err()
            );
        }
        {
            let (data, parity) = slices.split_at_mut(9);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            assert_eq!(
                Error::TooManyParityShards,
                r.encode_single_sep(0, data_refs[0], &mut parity_refs)
                    .unwrap_err()
            );
        }
    }
}

#[test]
fn test_encode_sep_error_handling() {
    let r = ReedSolomon::new(10, 3).unwrap();

    {
        let mut shards = make_random_shards!(1000, 13);

        let (data, parity) = shards.split_at_mut(10);

        r.encode_sep(data, parity).unwrap();

        {
            let mut shards = make_random_shards!(1000, 12);
            let (data, parity) = shards.split_at_mut(9);

            assert_eq!(
                Error::TooFewDataShards,
                r.encode_sep(data, parity).unwrap_err()
            );
        }
        {
            let mut shards = make_random_shards!(1000, 14);
            let (data, parity) = shards.split_at_mut(11);

            assert_eq!(
                Error::TooManyDataShards,
                r.encode_sep(data, parity).unwrap_err()
            );
        }
        {
            let mut shards = make_random_shards!(1000, 12);
            let (data, parity) = shards.split_at_mut(10);

            assert_eq!(
                Error::TooFewParityShards,
                r.encode_sep(data, parity).unwrap_err()
            );
        }
        {
            let mut shards = make_random_shards!(1000, 14);
            let (data, parity) = shards.split_at_mut(10);

            assert_eq!(
                Error::TooManyParityShards,
                r.encode_sep(data, parity).unwrap_err()
            );
        }
    }
    {
        let mut slices: [[u8; 1000]; 13] = [[0; 1000]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }

        let (data, parity) = slices.split_at_mut(10);

        let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
        let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

        r.encode_sep(&data_refs, &mut parity_refs).unwrap();

        {
            let mut slices: [[u8; 1000]; 12] = [[0; 1000]; 12];
            for slice in slices.iter_mut() {
                fill_random(slice);
            }

            let (data, parity) = slices.split_at_mut(9);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            assert_eq!(
                Error::TooFewDataShards,
                r.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
            );
        }
        {
            let mut slices: [[u8; 1000]; 14] = [[0; 1000]; 14];
            for slice in slices.iter_mut() {
                fill_random(slice);
            }

            let (data, parity) = slices.split_at_mut(11);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            assert_eq!(
                Error::TooManyDataShards,
                r.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
            );
        }
        {
            let mut slices: [[u8; 1000]; 12] = [[0; 1000]; 12];
            for slice in slices.iter_mut() {
                fill_random(slice);
            }

            let (data, parity) = slices.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            assert_eq!(
                Error::TooFewParityShards,
                r.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
            );
        }
        {
            let mut slices: [[u8; 1000]; 14] = [[0; 1000]; 14];
            for slice in slices.iter_mut() {
                fill_random(slice);
            }

            let (data, parity) = slices.split_at_mut(10);

            let data_refs = convert_2D_slices!(data=>to_mut_vec &[u8]);
            let mut parity_refs = convert_2D_slices!(parity=>to_mut_vec &mut [u8]);

            assert_eq!(
                Error::TooManyParityShards,
                r.encode_sep(&data_refs, &mut parity_refs).unwrap_err()
            );
        }
    }
}

#[test]
fn test_encode_single_error_handling() {
    let r = ReedSolomon::new(10, 3).unwrap();

    {
        let mut shards = make_random_shards!(1000, 13);

        for i in 0..10 {
            r.encode_single(i, &mut shards).unwrap();
        }

        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(10, &mut shards).unwrap_err()
        );
        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(11, &mut shards).unwrap_err()
        );
        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(12, &mut shards).unwrap_err()
        );
        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(13, &mut shards).unwrap_err()
        );
        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(14, &mut shards).unwrap_err()
        );
    }
    {
        let mut slices: [[u8; 1000]; 13] = [[0; 1000]; 13];
        for slice in slices.iter_mut() {
            fill_random(slice);
        }

        let mut slice_refs = convert_2D_slices!(slices=>to_mut_vec &mut [u8]);

        for i in 0..10 {
            r.encode_single(i, &mut slice_refs).unwrap();
        }

        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(10, &mut slice_refs).unwrap_err()
        );
        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(11, &mut slice_refs).unwrap_err()
        );
        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(12, &mut slice_refs).unwrap_err()
        );
        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(13, &mut slice_refs).unwrap_err()
        );
        assert_eq!(
            Error::InvalidIndex,
            r.encode_single(14, &mut slice_refs).unwrap_err()
        );
    }
}
