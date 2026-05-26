extern crate alloc;

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use smallvec::SmallVec;

use crate::errors::Error;
use crate::errors::SBSError;

use crate::matrix::Matrix;

use hashlink::LruCache;

#[cfg(feature = "std")]
use parking_lot::Mutex;
#[cfg(feature = "std")]
use rayon::prelude::*;
#[cfg(not(feature = "std"))]
use spin::Mutex;
#[cfg(feature = "std")]
use std::sync::atomic::{AtomicUsize, Ordering};

use super::Field;
use super::ReconstructShard;

const DATA_DECODE_MATRIX_CACHE_MIN_CAPACITY: usize = 128;
const DATA_DECODE_MATRIX_CACHE_MAX_CAPACITY: usize = 4096;
const CODE_SLICE_MIN_CHUNK_BYTES: usize = 16 * 1024;
const CODE_SLICE_DEFAULT_CHUNK_BYTES: usize = 64 * 1024;
const CODE_SLICE_LARGE_CHUNK_BYTES: usize = 256 * 1024;
#[cfg(feature = "std")]
const PARALLEL_MIN_SHARD_BYTES: usize = 256 * 1024;
#[cfg(feature = "std")]
pub const PARALLEL_POLICY_VERSION: u32 = 1;
#[cfg(feature = "std")]
const RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES_ENV: &str =
    "RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES";
#[cfg(feature = "std")]
const RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB_ENV: &str = "RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB";
#[cfg(feature = "std")]
const RS_PARALLEL_POLICY_MAX_JOBS_ENV: &str = "RS_PARALLEL_POLICY_MAX_JOBS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixMode {
    Vandermonde,
    Cauchy,
    JerasureLike,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecOptions {
    pub fast_one_parity: bool,
    pub inversion_cache: bool,
    pub inversion_cache_capacity: usize,
    pub matrix_mode: MatrixMode,
}

impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            fast_one_parity: false,
            inversion_cache: true,
            inversion_cache_capacity: 0,
            matrix_mode: MatrixMode::Vandermonde,
        }
    }
}

#[cfg(feature = "std")]
#[derive(Debug, Default)]
struct ReconstructionCacheMetrics {
    requests: AtomicUsize,
    hits: AtomicUsize,
    misses: AtomicUsize,
    inserts: AtomicUsize,
    evictions: AtomicUsize,
}

#[cfg(feature = "std")]
#[derive(Debug, Default)]
struct RuntimeProfileMetrics {
    code_some_serial_calls: AtomicUsize,
    code_some_parallel_calls: AtomicUsize,
    code_some_total_bytes: AtomicUsize,
    code_some_total_chunks: AtomicUsize,
    code_some_small_output_chunk_parallel_calls: AtomicUsize,
    code_some_small_output_chunk_parallel_outputs: AtomicUsize,
    code_some_small_output_chunk_parallel_chunks: AtomicUsize,
    code_single_serial_calls: AtomicUsize,
    code_single_parallel_calls: AtomicUsize,
    code_single_total_bytes: AtomicUsize,
    code_single_total_chunks: AtomicUsize,
    parallel_policy_calls: AtomicUsize,
    parallel_policy_parallel: AtomicUsize,
    parallel_policy_serial: AtomicUsize,
    parallel_policy_total_jobs: AtomicUsize,
    parallel_policy_total_chunk_len: AtomicUsize,
    reconstruct_calls: AtomicUsize,
    reconstruct_data_only_calls: AtomicUsize,
    reconstruct_total_missing_data: AtomicUsize,
    reconstruct_total_missing_parity: AtomicUsize,
    reconstruct_all_present_fast_path: AtomicUsize,
    reconstruct_data_stage_calls: AtomicUsize,
    reconstruct_data_stage_bytes: AtomicUsize,
    reconstruct_parity_stage_calls: AtomicUsize,
    reconstruct_parity_stage_bytes: AtomicUsize,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconstructionCacheStats {
    pub requests: usize,
    pub hits: usize,
    pub misses: usize,
    pub inserts: usize,
    pub evictions: usize,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReconstructionCacheAnalysis {
    pub hit_rate: f64,
    pub reuse_ratio: f64,
    pub miss_cost_per_request: f64,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProfileStats {
    pub code_some_serial_calls: usize,
    pub code_some_parallel_calls: usize,
    pub code_some_total_bytes: usize,
    pub code_some_total_chunks: usize,
    pub code_some_small_output_chunk_parallel_calls: usize,
    pub code_some_small_output_chunk_parallel_outputs: usize,
    pub code_some_small_output_chunk_parallel_chunks: usize,
    pub code_single_serial_calls: usize,
    pub code_single_parallel_calls: usize,
    pub code_single_total_bytes: usize,
    pub code_single_total_chunks: usize,
    pub parallel_policy_calls: usize,
    pub parallel_policy_parallel: usize,
    pub parallel_policy_serial: usize,
    pub parallel_policy_total_jobs: usize,
    pub parallel_policy_total_chunk_len: usize,
    pub reconstruct_calls: usize,
    pub reconstruct_data_only_calls: usize,
    pub reconstruct_total_missing_data: usize,
    pub reconstruct_total_missing_parity: usize,
    pub reconstruct_all_present_fast_path: usize,
    pub reconstruct_data_stage_calls: usize,
    pub reconstruct_data_stage_bytes: usize,
    pub reconstruct_parity_stage_calls: usize,
    pub reconstruct_parity_stage_bytes: usize,
}

#[cfg(feature = "std")]
impl ReconstructionCacheMetrics {
    fn snapshot(&self) -> ReconstructionCacheStats {
        ReconstructionCacheStats {
            requests: self.requests.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            inserts: self.inserts.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
        }
    }
}

#[cfg(feature = "std")]
impl ReconstructionCacheStats {
    #[inline]
    pub fn hit_rate(&self) -> f64 {
        if self.requests == 0 {
            0.0
        } else {
            self.hits as f64 / self.requests as f64
        }
    }

    #[inline]
    pub fn reuse_ratio(&self) -> f64 {
        if self.inserts == 0 {
            0.0
        } else {
            self.hits as f64 / self.inserts as f64
        }
    }

    #[inline]
    pub fn miss_cost_per_request(&self) -> f64 {
        if self.requests == 0 {
            0.0
        } else {
            self.misses as f64 / self.requests as f64
        }
    }

    #[inline]
    pub fn analysis(&self) -> ReconstructionCacheAnalysis {
        ReconstructionCacheAnalysis {
            hit_rate: self.hit_rate(),
            reuse_ratio: self.reuse_ratio(),
            miss_cost_per_request: self.miss_cost_per_request(),
        }
    }
}

#[cfg(feature = "std")]
impl RuntimeProfileMetrics {
    fn snapshot(&self) -> RuntimeProfileStats {
        RuntimeProfileStats {
            code_some_serial_calls: self.code_some_serial_calls.load(Ordering::Relaxed),
            code_some_parallel_calls: self.code_some_parallel_calls.load(Ordering::Relaxed),
            code_some_total_bytes: self.code_some_total_bytes.load(Ordering::Relaxed),
            code_some_total_chunks: self.code_some_total_chunks.load(Ordering::Relaxed),
            code_some_small_output_chunk_parallel_calls: self
                .code_some_small_output_chunk_parallel_calls
                .load(Ordering::Relaxed),
            code_some_small_output_chunk_parallel_outputs: self
                .code_some_small_output_chunk_parallel_outputs
                .load(Ordering::Relaxed),
            code_some_small_output_chunk_parallel_chunks: self
                .code_some_small_output_chunk_parallel_chunks
                .load(Ordering::Relaxed),
            code_single_serial_calls: self.code_single_serial_calls.load(Ordering::Relaxed),
            code_single_parallel_calls: self.code_single_parallel_calls.load(Ordering::Relaxed),
            code_single_total_bytes: self.code_single_total_bytes.load(Ordering::Relaxed),
            code_single_total_chunks: self.code_single_total_chunks.load(Ordering::Relaxed),
            parallel_policy_calls: self.parallel_policy_calls.load(Ordering::Relaxed),
            parallel_policy_parallel: self.parallel_policy_parallel.load(Ordering::Relaxed),
            parallel_policy_serial: self.parallel_policy_serial.load(Ordering::Relaxed),
            parallel_policy_total_jobs: self.parallel_policy_total_jobs.load(Ordering::Relaxed),
            parallel_policy_total_chunk_len: self
                .parallel_policy_total_chunk_len
                .load(Ordering::Relaxed),
            reconstruct_calls: self.reconstruct_calls.load(Ordering::Relaxed),
            reconstruct_data_only_calls: self.reconstruct_data_only_calls.load(Ordering::Relaxed),
            reconstruct_total_missing_data: self
                .reconstruct_total_missing_data
                .load(Ordering::Relaxed),
            reconstruct_total_missing_parity: self
                .reconstruct_total_missing_parity
                .load(Ordering::Relaxed),
            reconstruct_all_present_fast_path: self
                .reconstruct_all_present_fast_path
                .load(Ordering::Relaxed),
            reconstruct_data_stage_calls: self.reconstruct_data_stage_calls.load(Ordering::Relaxed),
            reconstruct_data_stage_bytes: self.reconstruct_data_stage_bytes.load(Ordering::Relaxed),
            reconstruct_parity_stage_calls: self
                .reconstruct_parity_stage_calls
                .load(Ordering::Relaxed),
            reconstruct_parity_stage_bytes: self
                .reconstruct_parity_stage_bytes
                .load(Ordering::Relaxed),
        }
    }

    fn reset(&self) {
        self.code_some_serial_calls.store(0, Ordering::Relaxed);
        self.code_some_parallel_calls.store(0, Ordering::Relaxed);
        self.code_some_total_bytes.store(0, Ordering::Relaxed);
        self.code_some_total_chunks.store(0, Ordering::Relaxed);
        self.code_some_small_output_chunk_parallel_calls
            .store(0, Ordering::Relaxed);
        self.code_some_small_output_chunk_parallel_outputs
            .store(0, Ordering::Relaxed);
        self.code_some_small_output_chunk_parallel_chunks
            .store(0, Ordering::Relaxed);
        self.code_single_serial_calls.store(0, Ordering::Relaxed);
        self.code_single_parallel_calls.store(0, Ordering::Relaxed);
        self.code_single_total_bytes.store(0, Ordering::Relaxed);
        self.code_single_total_chunks.store(0, Ordering::Relaxed);
        self.parallel_policy_calls.store(0, Ordering::Relaxed);
        self.parallel_policy_parallel.store(0, Ordering::Relaxed);
        self.parallel_policy_serial.store(0, Ordering::Relaxed);
        self.parallel_policy_total_jobs.store(0, Ordering::Relaxed);
        self.parallel_policy_total_chunk_len
            .store(0, Ordering::Relaxed);
        self.reconstruct_calls.store(0, Ordering::Relaxed);
        self.reconstruct_data_only_calls.store(0, Ordering::Relaxed);
        self.reconstruct_total_missing_data
            .store(0, Ordering::Relaxed);
        self.reconstruct_total_missing_parity
            .store(0, Ordering::Relaxed);
        self.reconstruct_all_present_fast_path
            .store(0, Ordering::Relaxed);
        self.reconstruct_data_stage_calls
            .store(0, Ordering::Relaxed);
        self.reconstruct_data_stage_bytes
            .store(0, Ordering::Relaxed);
        self.reconstruct_parity_stage_calls
            .store(0, Ordering::Relaxed);
        self.reconstruct_parity_stage_bytes
            .store(0, Ordering::Relaxed);
    }

    fn record_code_some(
        &self,
        parallel: bool,
        shard_len: usize,
        inputs: usize,
        outputs: usize,
        chunk_len: usize,
    ) {
        if parallel {
            self.code_some_parallel_calls
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.code_some_serial_calls.fetch_add(1, Ordering::Relaxed);
        }
        self.code_some_total_bytes.fetch_add(
            shard_len.saturating_mul(inputs).saturating_mul(outputs),
            Ordering::Relaxed,
        );
        let chunks = if chunk_len == 0 {
            0
        } else {
            shard_len.div_ceil(chunk_len)
        };
        self.code_some_total_chunks
            .fetch_add(chunks, Ordering::Relaxed);
    }

    fn record_code_some_small_output_chunk_parallel(&self, outputs: usize, chunks: usize) {
        self.code_some_small_output_chunk_parallel_calls
            .fetch_add(1, Ordering::Relaxed);
        self.code_some_small_output_chunk_parallel_outputs
            .fetch_add(outputs, Ordering::Relaxed);
        self.code_some_small_output_chunk_parallel_chunks
            .fetch_add(chunks, Ordering::Relaxed);
    }

    fn record_code_single(
        &self,
        parallel: bool,
        shard_len: usize,
        outputs: usize,
        chunk_len: usize,
    ) {
        if parallel {
            self.code_single_parallel_calls
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.code_single_serial_calls
                .fetch_add(1, Ordering::Relaxed);
        }
        self.code_single_total_bytes
            .fetch_add(shard_len.saturating_mul(outputs), Ordering::Relaxed);
        let chunks = if chunk_len == 0 {
            0
        } else {
            shard_len.div_ceil(chunk_len)
        };
        self.code_single_total_chunks
            .fetch_add(chunks, Ordering::Relaxed);
    }

    fn record_parallel_policy(&self, decision: ParallelDecision) {
        self.parallel_policy_calls.fetch_add(1, Ordering::Relaxed);
        if decision.use_parallel {
            self.parallel_policy_parallel
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.parallel_policy_serial.fetch_add(1, Ordering::Relaxed);
        }
        self.parallel_policy_total_jobs
            .fetch_add(decision.jobs, Ordering::Relaxed);
        self.parallel_policy_total_chunk_len
            .fetch_add(decision.chunk_len, Ordering::Relaxed);
    }

    fn record_reconstruct(
        &self,
        data_only: bool,
        missing_data_count: usize,
        missing_parity_count: usize,
        all_present_fast_path: bool,
    ) {
        self.reconstruct_calls.fetch_add(1, Ordering::Relaxed);
        if data_only {
            self.reconstruct_data_only_calls
                .fetch_add(1, Ordering::Relaxed);
        }
        if all_present_fast_path {
            self.reconstruct_all_present_fast_path
                .fetch_add(1, Ordering::Relaxed);
        }
        self.reconstruct_total_missing_data
            .fetch_add(missing_data_count, Ordering::Relaxed);
        self.reconstruct_total_missing_parity
            .fetch_add(missing_parity_count, Ordering::Relaxed);
    }

    fn record_reconstruct_data_stage(&self, shard_len: usize, output_count: usize) {
        self.reconstruct_data_stage_calls
            .fetch_add(1, Ordering::Relaxed);
        self.reconstruct_data_stage_bytes
            .fetch_add(shard_len.saturating_mul(output_count), Ordering::Relaxed);
    }

    fn record_reconstruct_parity_stage(&self, shard_len: usize, output_count: usize) {
        self.reconstruct_parity_stage_calls
            .fetch_add(1, Ordering::Relaxed);
        self.reconstruct_parity_stage_bytes
            .fetch_add(shard_len.saturating_mul(output_count), Ordering::Relaxed);
    }
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParallelPolicy {
    pub min_parallel_shard_bytes: usize,
    pub min_bytes_per_job: usize,
    pub max_jobs: usize,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParallelDecision {
    pub use_parallel: bool,
    pub jobs: usize,
    pub chunk_len: usize,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeParallelPolicyCache {
    pub data: ParallelPolicy,
    pub reconstruct_data: ParallelPolicy,
    pub reconstruct_full_data: ParallelPolicy,
    pub reconstruct_full_parity: ParallelPolicy,
}

#[cfg(feature = "std")]
impl Default for ParallelPolicy {
    fn default() -> Self {
        Self {
            min_parallel_shard_bytes: PARALLEL_MIN_SHARD_BYTES,
            min_bytes_per_job: CODE_SLICE_LARGE_CHUNK_BYTES,
            max_jobs: 0,
        }
    }
}

#[cfg(feature = "std")]
impl ParallelPolicy {
    pub fn decide(
        &self,
        shard_size: usize,
        data_shards: usize,
        output_shards: usize,
        available_parallelism: usize,
    ) -> ParallelDecision {
        if shard_size == 0 || data_shards == 0 || output_shards == 0 {
            return ParallelDecision {
                use_parallel: false,
                jobs: 1,
                chunk_len: shard_size,
            };
        }

        let available_parallelism = available_parallelism.max(1);
        let max_jobs = if self.max_jobs == 0 {
            available_parallelism
        } else {
            core::cmp::min(self.max_jobs, available_parallelism)
        }
        .max(1);

        let min_parallel_shard_bytes = self.min_parallel_shard_bytes.max(1);
        let min_bytes_per_job = self.min_bytes_per_job.max(CODE_SLICE_MIN_CHUNK_BYTES);

        let chunk_count = shard_size.div_ceil(min_bytes_per_job).max(1);
        let max_useful_jobs = if output_shards <= 2 {
            // For tiny output counts the hot path parallelizes by chunks, so
            // chunk_count is the practical ceiling for useful work.
            chunk_count
        } else {
            output_shards.saturating_mul(chunk_count)
        }
        .max(1);
        let jobs = core::cmp::min(max_jobs, max_useful_jobs).max(1);

        if shard_size < min_parallel_shard_bytes || jobs < 2 {
            return ParallelDecision {
                use_parallel: false,
                jobs: 1,
                chunk_len: core::cmp::min(Self::serial_chunk_len(shard_size), shard_size),
            };
        }

        let chunks_per_output = core::cmp::max(1, jobs.div_ceil(output_shards));
        let chunk_len = shard_size
            .div_ceil(chunks_per_output)
            .clamp(CODE_SLICE_MIN_CHUNK_BYTES, min_bytes_per_job);

        ParallelDecision {
            use_parallel: true,
            jobs,
            chunk_len: core::cmp::min(chunk_len, shard_size),
        }
    }

    fn serial_chunk_len(shard_size: usize) -> usize {
        if shard_size <= CODE_SLICE_MIN_CHUNK_BYTES {
            shard_size
        } else if shard_size <= CODE_SLICE_DEFAULT_CHUNK_BYTES {
            CODE_SLICE_MIN_CHUNK_BYTES
        } else if shard_size <= 4 * 1024 * 1024 {
            CODE_SLICE_DEFAULT_CHUNK_BYTES
        } else {
            CODE_SLICE_LARGE_CHUNK_BYTES
        }
    }

    pub fn with_env_overrides(self) -> Self {
        let mut policy = self;
        if let Some(value) = parse_env_usize(RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES_ENV)
            && value > 0
        {
            policy.min_parallel_shard_bytes = value;
        }
        if let Some(value) = parse_env_usize(RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB_ENV)
            && value > 0
        {
            policy.min_bytes_per_job = value;
        }
        if let Some(value) = parse_env_usize(RS_PARALLEL_POLICY_MAX_JOBS_ENV) {
            policy.max_jobs = value;
        }
        policy
    }
}

#[cfg(feature = "std")]
impl RuntimeParallelPolicyCache {
    fn new(data: ParallelPolicy) -> Self {
        Self {
            data,
            reconstruct_data: data,
            reconstruct_full_data: data,
            reconstruct_full_parity: data,
        }
    }

    pub(crate) fn reconstruct_policy(&self, data_only: bool) -> ParallelPolicy {
        if data_only {
            self.reconstruct_data
        } else {
            self.reconstruct_full_data
        }
    }

    pub(crate) fn reconstruct_stage_policies(
        &self,
        data_only: bool,
    ) -> (ParallelPolicy, ParallelPolicy) {
        if data_only {
            (self.reconstruct_data, self.reconstruct_data)
        } else {
            (self.reconstruct_full_data, self.reconstruct_full_parity)
        }
    }
}

#[cfg(feature = "std")]
fn parse_env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
}

/// Bookkeeper for shard by shard encoding.
///
/// This is useful for avoiding incorrect use of
/// `encode_single` and `encode_single_sep`
///
/// # Use cases
///
/// Shard by shard encoding is useful for streamed data encoding
/// where you do not have all the needed data shards immediately,
/// but you want to spread out the encoding workload rather than
/// doing the encoding after everything is ready.
///
/// A concrete example would be network packets encoding,
/// where encoding packet by packet as you receive them may be more efficient
/// than waiting for N packets then encode them all at once.
///
/// # Example
///
/// ```
/// # #[macro_use] extern crate reed_solomon_erasure;
/// # use reed_solomon_erasure::*;
/// # fn main () {
/// use reed_solomon_erasure::galois_8::Field;
/// let r: ReedSolomon<Field> = ReedSolomon::new(3, 2).unwrap();
///
/// let mut sbs = ShardByShard::new(&r);
///
/// let mut shards = shards!([0u8,  1,  2,  3,  4],
///                          [5,  6,  7,  8,  9],
///                          // say we don't have the 3rd data shard yet
///                          // and we want to fill it in later
///                          [0,  0,  0,  0,  0],
///                          [0,  0,  0,  0,  0],
///                          [0,  0,  0,  0,  0]);
///
/// // encode 1st and 2nd data shard
/// sbs.encode(&mut shards).unwrap();
/// sbs.encode(&mut shards).unwrap();
///
/// // fill in 3rd data shard
/// shards[2][0] = 10.into();
/// shards[2][1] = 11.into();
/// shards[2][2] = 12.into();
/// shards[2][3] = 13.into();
/// shards[2][4] = 14.into();
///
/// // now do the encoding
/// sbs.encode(&mut shards).unwrap();
///
/// assert!(r.verify(&shards).unwrap());
/// # }
/// ```
#[derive(PartialEq, Debug)]
pub struct ShardByShard<'a, F: 'a + Field> {
    codec: &'a ReedSolomon<F>,
    cur_input: usize,
}

impl<'a, F: 'a + Field> ShardByShard<'a, F> {
    /// Creates a new instance of the bookkeeping struct.
    pub fn new(codec: &'a ReedSolomon<F>) -> ShardByShard<'a, F> {
        ShardByShard {
            codec,
            cur_input: 0,
        }
    }

    /// Checks if the parity shards are ready to use.
    pub fn parity_ready(&self) -> bool {
        self.cur_input == self.codec.data_shard_count
    }

    /// Resets the bookkeeping data.
    ///
    /// You should call this when you have added and encoded
    /// all data shards, and have finished using the parity shards.
    ///
    /// Returns `SBSError::LeftoverShards` when there are shards encoded
    /// but parity shards are not ready to use.
    pub fn reset(&mut self) -> Result<(), SBSError> {
        if self.cur_input > 0 && !self.parity_ready() {
            return Err(SBSError::LeftoverShards);
        }

        self.cur_input = 0;

        Ok(())
    }

    /// Resets the bookkeeping data without checking.
    pub fn reset_force(&mut self) {
        self.cur_input = 0;
    }

    /// Returns the current input shard index.
    pub fn cur_input_index(&self) -> usize {
        self.cur_input
    }

    fn return_ok_and_incre_cur_input(&mut self) -> Result<(), SBSError> {
        self.cur_input += 1;
        Ok(())
    }

    fn sbs_encode_checks<U: AsRef<[F::Elem]> + AsMut<[F::Elem]>>(
        &mut self,
        slices: &mut [U],
    ) -> Result<(), SBSError> {
        let internal_checks = |codec: &ReedSolomon<F>, data: &mut [U]| {
            check_piece_count!(all => codec, data);
            check_slices!(multi => data);

            Ok(())
        };

        if self.parity_ready() {
            return Err(SBSError::TooManyCalls);
        }

        match internal_checks(self.codec, slices) {
            Ok(()) => Ok(()),
            Err(e) => Err(SBSError::RSError(e)),
        }
    }

    fn sbs_encode_sep_checks<T: AsRef<[F::Elem]>, U: AsRef<[F::Elem]> + AsMut<[F::Elem]>>(
        &mut self,
        data: &[T],
        parity: &mut [U],
    ) -> Result<(), SBSError> {
        let internal_checks = |codec: &ReedSolomon<F>, data: &[T], parity: &mut [U]| {
            check_piece_count!(data => codec, data);
            check_piece_count!(parity => codec, parity);
            check_slices!(multi => data, multi => parity);

            Ok(())
        };

        if self.parity_ready() {
            return Err(SBSError::TooManyCalls);
        }

        match internal_checks(self.codec, data, parity) {
            Ok(()) => Ok(()),
            Err(e) => Err(SBSError::RSError(e)),
        }
    }

    /// Constructs the parity shards partially using the current input data shard.
    ///
    /// Returns `SBSError::TooManyCalls` when all input data shards
    /// have already been filled in via `encode`
    pub fn encode<T, U>(&mut self, mut shards: T) -> Result<(), SBSError>
    where
        T: AsRef<[U]> + AsMut<[U]>,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]>,
    {
        let shards = shards.as_mut();
        self.sbs_encode_checks(shards)?;

        self.codec
            .encode_single(self.cur_input, shards)
            .map_err(SBSError::RSError)?;

        self.return_ok_and_incre_cur_input()
    }

    /// Constructs the parity shards partially using the current input data shard.
    ///
    /// Returns `SBSError::TooManyCalls` when all input data shards
    /// have already been filled in via `encode`
    pub fn encode_sep<T: AsRef<[F::Elem]>, U: AsRef<[F::Elem]> + AsMut<[F::Elem]>>(
        &mut self,
        data: &[T],
        parity: &mut [U],
    ) -> Result<(), SBSError> {
        self.sbs_encode_sep_checks(data, parity)?;

        self.codec
            .encode_single_sep(self.cur_input, data[self.cur_input].as_ref(), parity)
            .map_err(SBSError::RSError)?;

        self.return_ok_and_incre_cur_input()
    }
}

/// Reed-Solomon erasure code encoder/decoder.
///
/// # Common error handling
///
/// ## For `encode`, `encode_shards`, `verify`, `verify_shards`, `reconstruct`, `reconstruct_data`, `reconstruct_shards`, `reconstruct_data_shards`
///
/// Return `Error::TooFewShards` or `Error::TooManyShards`
/// when the number of provided shards
/// does not match the codec's one.
///
/// Return `Error::EmptyShard` when the first shard provided is
/// of zero length.
///
/// Return `Error::IncorrectShardSize` when the provided shards
/// are of different lengths.
///
/// ## For `reconstruct`, `reconstruct_data`, `reconstruct_shards`, `reconstruct_data_shards`
///
/// Return `Error::TooFewShardsPresent` when there are not
/// enough shards for reconstruction.
///
/// Return `Error::InvalidShardFlags` when the number of flags does not match
/// the total number of shards.
///
/// # Variants of encoding methods
///
/// ## `sep`
///
/// Methods ending in `_sep` takes an immutable reference to data shards,
/// and a mutable reference to parity shards.
///
/// They are useful as they do not need to borrow the data shards mutably,
/// and other work that only needs read-only access to data shards can be done
/// in parallel/concurrently during the encoding.
///
/// Following is a table of all the `sep` variants
///
/// | not `sep` | `sep` |
/// | --- | --- |
/// | `encode_single` | `encode_single_sep` |
/// | `encode`        | `encode_sep` |
///
/// The `sep` variants do similar checks on the provided data shards and
/// parity shards.
///
/// Return `Error::TooFewDataShards`, `Error::TooManyDataShards`,
/// `Error::TooFewParityShards`, or `Error::TooManyParityShards` when applicable.
///
/// ## `single`
///
/// Methods containing `single` facilitate shard by shard encoding, where
/// the parity shards are partially constructed using one data shard at a time.
/// See `ShardByShard` struct for more details on how shard by shard encoding
/// can be useful.
///
/// They are prone to **misuse**, and it is recommended to use the `ShardByShard`
/// bookkeeping struct instead for shard by shard encoding.
///
/// The ones that are also `sep` are **ESPECIALLY** prone to **misuse**.
/// Only use them when you actually need the flexibility.
///
/// Following is a table of all the shard by shard variants
///
/// | all shards at once | shard by shard |
/// | --- | --- |
/// | `encode` | `encode_single` |
/// | `encode_sep` | `encode_single_sep` |
///
/// The `single` variants do similar checks on the provided data shards and parity shards,
/// and also do index check on `i_data`.
///
/// Return `Error::InvalidIndex` if `i_data >= data_shard_count`.
///
/// # Encoding behaviour
/// ## For `encode`
///
/// You do not need to clear the parity shards beforehand, as the methods
/// will overwrite them completely.
///
/// ## For `encode_single`, `encode_single_sep`
///
/// Calling them with `i_data` being `0` will overwrite the parity shards
/// completely. If you are using the methods correctly, then you do not need
/// to clear the parity shards beforehand.
///
/// # Variants of verifying methods
///
/// `verify` allocate sa buffer on the heap of the same size
/// as the parity shards, and encode the input once using the buffer to store
/// the computed parity shards, then check if the provided parity shards
/// match the computed ones.
///
/// `verify_with_buffer`, allows you to provide
/// the buffer to avoid making heap allocation(s) for the buffer in every call.
///
/// The `with_buffer` variants also guarantee that the buffer contains the correct
/// parity shards if the result is `Ok(_)` (i.e. it does not matter whether the
/// verification passed or not, as long as the result is not an error, the buffer
/// will contain the correct parity shards after the call).
///
/// Following is a table of all the `with_buffer` variants
///
/// | not `with_buffer` | `with_buffer` |
/// | --- | --- |
/// | `verify` | `verify_with_buffer` |
///
/// The `with_buffer` variants also check the dimensions of the buffer and return
/// `Error::TooFewBufferShards`, `Error::TooManyBufferShards`, `Error::EmptyShard`,
/// or `Error::IncorrectShardSize` when applicable.
///
#[derive(Debug)]
pub struct ReedSolomon<F: Field> {
    data_shard_count: usize,
    parity_shard_count: usize,
    total_shard_count: usize,
    matrix: Matrix<F>,
    options: CodecOptions,
    #[cfg(feature = "std")]
    pub(crate) policy_cache: RuntimeParallelPolicyCache,
    data_decode_matrix_cache: Mutex<LruCache<Vec<usize>, Arc<Matrix<F>>>>,
    #[cfg(feature = "std")]
    reconstruction_cache_metrics: ReconstructionCacheMetrics,
    #[cfg(feature = "std")]
    runtime_profile_metrics: RuntimeProfileMetrics,
}

impl<F: Field> Clone for ReedSolomon<F> {
    fn clone(&self) -> ReedSolomon<F> {
        match ReedSolomon::with_options(
            self.data_shard_count,
            self.parity_shard_count,
            self.options,
        ) {
            Ok(codec) => codec,
            Err(_) => panic!("existing codec invariants must produce a valid clone"),
        }
    }
}

impl<F: Field> PartialEq for ReedSolomon<F> {
    fn eq(&self, rhs: &ReedSolomon<F>) -> bool {
        self.data_shard_count == rhs.data_shard_count
            && self.parity_shard_count == rhs.parity_shard_count
    }
}

impl<F: Field> ReedSolomon<F> {
    fn normalize_inversion_cache_capacity(
        data_shards: usize,
        parity_shards: usize,
        requested_capacity: usize,
    ) -> usize {
        if requested_capacity > 0 {
            return requested_capacity;
        }

        Self::recommended_inversion_cache_capacity(data_shards, parity_shards)
    }

    fn derive_inversion_cache_capacity(data_shards: usize, parity_shards: usize) -> usize {
        let total_shards = data_shards.saturating_add(parity_shards);
        let heuristic = total_shards
            .saturating_mul(parity_shards.max(1))
            .saturating_mul(2);
        let rounded = heuristic
            .checked_next_power_of_two()
            .unwrap_or(DATA_DECODE_MATRIX_CACHE_MAX_CAPACITY);

        rounded.clamp(
            DATA_DECODE_MATRIX_CACHE_MIN_CAPACITY,
            DATA_DECODE_MATRIX_CACHE_MAX_CAPACITY,
        )
    }

    // AUDIT
    //
    // Error detection responsibilities
    //
    // Terminologies and symbols:
    //   X =A, B, C=> Y: X delegates error checking responsibilities A, B, C to Y
    //   X:= A, B, C: X needs to handle responsibilities A, B, C
    //
    // Encode methods
    //
    // `encode_single`:=
    //   - check index `i_data` within range [0, data shard count)
    //   - check length of `slices` matches total shard count exactly
    //   - check consistency of length of individual slices
    // `encode_single_sep`:=
    //   - check index `i_data` within range [0, data shard count)
    //   - check length of `parity` matches parity shard count exactly
    //   - check consistency of length of individual parity slices
    //   - check length of `single_data` matches length of first parity slice
    // `encode`:=
    //   - check length of `slices` matches total shard count exactly
    //   - check consistency of length of individual slices
    // `encode_sep`:=
    //   - check length of `data` matches data shard count exactly
    //   - check length of `parity` matches parity shard count exactly
    //   - check consistency of length of individual data slices
    //   - check consistency of length of individual parity slices
    //   - check length of first parity slice matches length of first data slice
    //
    // Verify methods
    //
    // `verify`:=
    //   - check length of `slices` matches total shard count exactly
    //   - check consistency of length of individual slices
    //
    //   Generates buffer then passes control to verify_with_buffer
    //
    // `verify_with_buffer`:=
    //   - check length of `slices` matches total shard count exactly
    //   - check length of `buffer` matches parity shard count exactly
    //   - check consistency of length of individual slices
    //   - check consistency of length of individual slices in buffer
    //   - check length of first slice in buffer matches length of first slice
    //
    // Reconstruct methods
    //
    // `reconstruct` =ALL=> `reconstruct_internal`
    // `reconstruct_data`=ALL=> `reconstruct_internal`
    // `reconstruct_internal`:=
    //   - check length of `slices` matches total shard count exactly
    //   - check consistency of length of individual slices
    //   - check length of `slice_present` matches length of `slices`

    fn get_parity_rows(&self) -> SmallVec<[&[F::Elem]; 32]> {
        let mut parity_rows = SmallVec::with_capacity(self.parity_shard_count);
        let matrix = &self.matrix;
        for i in self.data_shard_count..self.total_shard_count {
            parity_rows.push(matrix.get_row(i));
        }

        parity_rows
    }

    fn build_matrix(data_shards: usize, total_shards: usize) -> Matrix<F> {
        let vandermonde = Matrix::vandermonde(total_shards, data_shards);

        let top = vandermonde.sub_matrix(0, 0, data_shards, data_shards);
        let top_inverted = match top.invert() {
            Ok(inverted) => inverted,
            Err(_) => panic!("vandermonde top matrix must be invertible for valid shard counts"),
        };

        vandermonde.multiply(&top_inverted)
    }

    fn build_matrix_with_options(
        data_shards: usize,
        total_shards: usize,
        options: CodecOptions,
    ) -> Matrix<F> {
        match options.matrix_mode {
            MatrixMode::Vandermonde
            | MatrixMode::Cauchy
            | MatrixMode::JerasureLike
            | MatrixMode::Custom => Self::build_matrix(data_shards, total_shards),
        }
    }

    #[cfg(feature = "std")]
    fn resolve_policy_cache() -> RuntimeParallelPolicyCache {
        let data = ParallelPolicy::default().with_env_overrides();
        if core::any::type_name::<F>() == core::any::type_name::<crate::galois_8::Field>() {
            crate::galois_8::resolve_runtime_parallel_policy_cache(data)
        } else {
            RuntimeParallelPolicyCache::new(data)
        }
    }

    /// Creates a new instance of Reed-Solomon erasure code encoder/decoder.
    ///
    /// Returns `Error::TooFewDataShards` if `data_shards == 0`.
    ///
    /// Returns `Error::TooFewParityShards` if `parity_shards == 0`.
    ///
    /// Returns `Error::TooManyShards` if `data_shards + parity_shards > F::ORDER`.
    pub fn new(data_shards: usize, parity_shards: usize) -> Result<ReedSolomon<F>, Error> {
        Self::with_options(data_shards, parity_shards, CodecOptions::default())
    }

    /// Creates a new instance of Reed-Solomon erasure code encoder/decoder with explicit options.
    pub fn with_options(
        data_shards: usize,
        parity_shards: usize,
        mut options: CodecOptions,
    ) -> Result<ReedSolomon<F>, Error> {
        if data_shards == 0 {
            return Err(Error::TooFewDataShards);
        }
        if parity_shards == 0 {
            return Err(Error::TooFewParityShards);
        }
        if data_shards + parity_shards > F::ORDER {
            return Err(Error::TooManyShards);
        }

        let total_shards = data_shards + parity_shards;

        options.inversion_cache_capacity = Self::normalize_inversion_cache_capacity(
            data_shards,
            parity_shards,
            options.inversion_cache_capacity,
        );

        let matrix = Self::build_matrix_with_options(data_shards, total_shards, options);
        #[cfg(feature = "std")]
        let policy_cache = Self::resolve_policy_cache();

        Ok(ReedSolomon {
            data_shard_count: data_shards,
            parity_shard_count: parity_shards,
            total_shard_count: total_shards,
            matrix,
            options,
            #[cfg(feature = "std")]
            policy_cache,
            data_decode_matrix_cache: Mutex::new(LruCache::new(options.inversion_cache_capacity)),
            #[cfg(feature = "std")]
            reconstruction_cache_metrics: ReconstructionCacheMetrics::default(),
            #[cfg(feature = "std")]
            runtime_profile_metrics: RuntimeProfileMetrics::default(),
        })
    }

    pub fn data_shard_count(&self) -> usize {
        self.data_shard_count
    }

    pub fn parity_shard_count(&self) -> usize {
        self.parity_shard_count
    }

    pub fn total_shard_count(&self) -> usize {
        self.total_shard_count
    }

    pub fn inversion_cache_capacity(&self) -> usize {
        self.options.inversion_cache_capacity
    }

    pub fn recommended_inversion_cache_capacity(
        data_shards: usize,
        parity_shards: usize,
    ) -> usize {
        Self::derive_inversion_cache_capacity(data_shards, parity_shards)
    }

    #[cfg(feature = "std")]
    pub fn reconstruction_cache_stats(&self) -> ReconstructionCacheStats {
        self.reconstruction_cache_metrics.snapshot()
    }

    #[cfg(feature = "std")]
    pub fn runtime_profile_stats(&self) -> RuntimeProfileStats {
        self.runtime_profile_metrics.snapshot()
    }

    #[cfg(feature = "std")]
    pub fn reset_runtime_profile_stats(&self) {
        self.runtime_profile_metrics.reset();
    }

    pub fn split(&self, data: &[F::Elem]) -> Result<Vec<Vec<F::Elem>>, Error> {
        let data_shards = self.data_shard_count;
        let shard_len = if data.is_empty() {
            0
        } else {
            data.len().div_ceil(data_shards)
        };

        let mut shards = Vec::with_capacity(data_shards);
        for i in 0..data_shards {
            let start = i * shard_len;
            let end = core::cmp::min(start + shard_len, data.len());
            let mut shard = vec![F::zero(); shard_len];
            if start < data.len() {
                shard[..end - start].copy_from_slice(&data[start..end]);
            }
            shards.push(shard);
        }

        Ok(shards)
    }

    pub fn join<T: AsRef<[F::Elem]>>(
        &self,
        shards: &[T],
        out_len: usize,
    ) -> Result<Vec<F::Elem>, Error> {
        check_piece_count!(data => self, shards);
        check_slices!(multi => shards);

        let available = shards
            .iter()
            .map(|shard| shard.as_ref().len())
            .sum::<usize>();
        let target_len = core::cmp::min(out_len, available);
        let mut result = Vec::with_capacity(target_len);

        for shard in shards {
            let remaining = target_len.saturating_sub(result.len());
            if remaining == 0 {
                break;
            }

            let data = shard.as_ref();
            let to_take = core::cmp::min(remaining, data.len());
            result.extend_from_slice(&data[..to_take]);
        }

        result.truncate(target_len);
        Ok(result)
    }

    fn code_some_slices<T: AsRef<[F::Elem]>, U: AsMut<[F::Elem]>>(
        &self,
        matrix_rows: &[&[F::Elem]],
        inputs: &[T],
        outputs: &mut [U],
    ) {
        self.code_some_slices_chunked(matrix_rows, inputs, outputs);
    }

    pub(crate) fn code_some_slices_chunked<T: AsRef<[F::Elem]>, U: AsMut<[F::Elem]>>(
        &self,
        matrix_rows: &[&[F::Elem]],
        inputs: &[T],
        outputs: &mut [U],
    ) {
        let shard_len = inputs
            .first()
            .map(|input| input.as_ref().len())
            .unwrap_or(0);
        if shard_len == 0 {
            return;
        }

        let chunk_len = self.code_chunk_len(shard_len);
        #[cfg(feature = "std")]
        self.runtime_profile_metrics.record_code_some(
            false,
            shard_len,
            inputs.len(),
            outputs.len(),
            chunk_len,
        );
        let mut start = 0;
        while start < shard_len {
            let end = core::cmp::min(start + chunk_len, shard_len);
            for (i_input, input) in inputs.iter().enumerate().take(self.data_shard_count) {
                self.code_single_slice_range(
                    matrix_rows,
                    i_input,
                    input.as_ref(),
                    outputs,
                    start,
                    end,
                );
            }
            start = end;
        }
    }

    pub(crate) fn code_chunk_len(&self, shard_len: usize) -> usize {
        let chunk = Self::serial_code_chunk_len(shard_len);

        core::cmp::min(chunk, shard_len)
    }

    fn serial_code_chunk_len(shard_len: usize) -> usize {
        if shard_len <= CODE_SLICE_MIN_CHUNK_BYTES {
            shard_len
        } else if shard_len <= CODE_SLICE_DEFAULT_CHUNK_BYTES {
            CODE_SLICE_MIN_CHUNK_BYTES
        } else if shard_len <= 4 * 1024 * 1024 {
            CODE_SLICE_DEFAULT_CHUNK_BYTES
        } else {
            CODE_SLICE_LARGE_CHUNK_BYTES
        }
    }

    #[cfg(feature = "std")]
    pub fn parallel_policy(&self, shard_len: usize, output_shards: usize) -> ParallelDecision {
        let decision = self.parallel_policy_with(
            shard_len,
            output_shards,
            std::thread::available_parallelism()
                .map(|parallelism| parallelism.get())
                .unwrap_or(1),
        );

        #[cfg(debug_assertions)]
        if std::env::var_os("RS_PARALLEL_POLICY_DEBUG").is_some() {
            eprintln!(
                "rs-parallel-policy v{} shard_len={} outputs={} -> use_parallel={} jobs={} chunk_len={}",
                PARALLEL_POLICY_VERSION,
                shard_len,
                output_shards,
                decision.use_parallel,
                decision.jobs,
                decision.chunk_len
            );
        }

        self.runtime_profile_metrics
            .record_parallel_policy(decision);
        decision
    }

    #[cfg(feature = "std")]
    pub(crate) fn parallel_policy_with(
        &self,
        shard_len: usize,
        output_shards: usize,
        available_parallelism: usize,
    ) -> ParallelDecision {
        self.effective_parallel_policy().decide(
            shard_len,
            self.data_shard_count,
            output_shards,
            available_parallelism,
        )
    }

    #[cfg(feature = "std")]
    pub fn parallel_policy_version(&self) -> u32 {
        PARALLEL_POLICY_VERSION
    }

    #[cfg(feature = "std")]
    pub fn effective_parallel_policy(&self) -> ParallelPolicy {
        self.policy_cache.data
    }

    #[cfg(feature = "std")]
    fn code_some_slices_par_chunked<T, U>(
        &self,
        matrix_rows: &[&[F::Elem]],
        inputs: &[T],
        outputs: &mut [U],
        chunk_len: usize,
    ) where
        F::Elem: Send + Sync,
        T: AsRef<[F::Elem]> + Sync,
        U: AsMut<[F::Elem]> + Send,
    {
        let shard_len = inputs
            .first()
            .map(|input| input.as_ref().len())
            .unwrap_or(0);
        if shard_len == 0 {
            return;
        }

        self.runtime_profile_metrics.record_code_some(
            true,
            shard_len,
            inputs.len(),
            outputs.len(),
            chunk_len,
        );
        let data_shard_count = self.data_shard_count;
        let chunk_count = shard_len.div_ceil(chunk_len);
        if outputs.len() <= 2 && chunk_count > 1 {
            self.runtime_profile_metrics
                .record_code_some_small_output_chunk_parallel(outputs.len(), chunk_count);
            if outputs.len() == 1 {
                let matrix_row = matrix_rows[0];
                outputs[0]
                    .as_mut()
                    .par_chunks_mut(chunk_len)
                    .enumerate()
                    .for_each(|(chunk_idx, output_chunk)| {
                        let start = chunk_idx * chunk_len;
                        let end = start + output_chunk.len();

                        F::mul_slice(matrix_row[0], &inputs[0].as_ref()[start..end], output_chunk);
                        for i_input in 1..data_shard_count {
                            F::mul_slice_add(
                                matrix_row[i_input],
                                &inputs[i_input].as_ref()[start..end],
                                output_chunk,
                            );
                        }
                    });
            } else {
                let matrix_row0 = matrix_rows[0];
                let matrix_row1 = matrix_rows[1];
                let (first, second) = outputs.split_at_mut(1);
                let output0 = first[0].as_mut();
                let output1 = second[0].as_mut();

                output0
                    .par_chunks_mut(chunk_len)
                    .zip(output1.par_chunks_mut(chunk_len))
                    .enumerate()
                    .for_each(|(chunk_idx, (output0_chunk, output1_chunk))| {
                        let start = chunk_idx * chunk_len;
                        let end = start + output0_chunk.len();
                        let input0 = &inputs[0].as_ref()[start..end];

                        F::mul_slice(matrix_row0[0], input0, output0_chunk);
                        F::mul_slice(matrix_row1[0], input0, output1_chunk);
                        for i_input in 1..data_shard_count {
                            let input_chunk = &inputs[i_input].as_ref()[start..end];
                            F::mul_slice_add(matrix_row0[i_input], input_chunk, output0_chunk);
                            F::mul_slice_add(matrix_row1[i_input], input_chunk, output1_chunk);
                        }
                    });
            }
        } else {
            outputs
                .par_iter_mut()
                .enumerate()
                .for_each(|(i_row, output)| {
                    let matrix_row = matrix_rows[i_row];
                    let output = output.as_mut();

                    let mut start = 0;
                    while start < shard_len {
                        let end = core::cmp::min(start + chunk_len, shard_len);
                        let output_chunk = &mut output[start..end];

                        F::mul_slice(matrix_row[0], &inputs[0].as_ref()[start..end], output_chunk);
                        for i_input in 1..data_shard_count {
                            F::mul_slice_add(
                                matrix_row[i_input],
                                &inputs[i_input].as_ref()[start..end],
                                output_chunk,
                            );
                        }

                        start = end;
                    }
                });
        }
    }

    #[cfg(feature = "std")]
    fn code_single_slice_par_chunked<U: AsMut<[F::Elem]> + Send>(
        &self,
        matrix_rows: &[&[F::Elem]],
        i_input: usize,
        input: &[F::Elem],
        outputs: &mut [U],
        chunk_len: usize,
    ) where
        F::Elem: Send + Sync,
    {
        let shard_len = input.len();
        if shard_len == 0 {
            return;
        }

        self.runtime_profile_metrics
            .record_code_single(true, shard_len, outputs.len(), chunk_len);
        outputs
            .par_iter_mut()
            .enumerate()
            .for_each(|(i_row, output)| {
                let coefficient = matrix_rows[i_row][i_input];
                let output = output.as_mut();

                let mut start = 0;
                while start < shard_len {
                    let end = core::cmp::min(start + chunk_len, shard_len);
                    let output_chunk = &mut output[start..end];
                    let input_chunk = &input[start..end];
                    if i_input == 0 {
                        F::mul_slice(coefficient, input_chunk, output_chunk);
                    } else {
                        F::mul_slice_add(coefficient, input_chunk, output_chunk);
                    }
                    start = end;
                }
            });
    }

    #[cfg(feature = "std")]
    /// Constructs parity shards using the std-only parallel fast path when the
    /// input/output types satisfy the required thread-safety bounds.
    pub fn encode_sep_par<T, U>(&self, data: &[T], parity: &mut [U]) -> Result<(), Error>
    where
        F::Elem: Send + Sync,
        T: AsRef<[F::Elem]> + Sync,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]> + Send,
    {
        check_piece_count!(data => self, data);
        check_piece_count!(parity => self, parity);
        check_slices!(multi => data, multi => parity);

        if self.fast_one_parity_enabled() {
            self.encode_fast_one_parity(data, parity);
            return Ok(());
        }

        let parity_rows = self.get_parity_rows();
        let shard_len = data[0].as_ref().len();
        let decision = self.parallel_policy(shard_len, parity.len());
        if !decision.use_parallel {
            self.code_some_slices(&parity_rows, data, parity);
            return Ok(());
        }
        self.code_some_slices_par_chunked(&parity_rows, data, parity, decision.chunk_len);

        Ok(())
    }

    #[cfg(feature = "std")]
    /// Constructs the parity shards partially using only one data shard on the
    /// std-only parallel fast path.
    pub fn encode_single_sep_par<U>(
        &self,
        i_data: usize,
        single_data: &[F::Elem],
        parity: &mut [U],
    ) -> Result<(), Error>
    where
        F::Elem: Send + Sync,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]> + Send,
    {
        check_slice_index!(data => self, i_data);
        check_piece_count!(parity => self, parity);
        check_slices!(multi => parity, single => single_data);

        let parity_rows = self.get_parity_rows();
        let decision = self.parallel_policy(single_data.len(), parity.len());
        if !decision.use_parallel {
            self.code_single_slice(&parity_rows, i_data, single_data, parity);
            return Ok(());
        }
        self.code_single_slice_par_chunked(
            &parity_rows,
            i_data,
            single_data,
            parity,
            decision.chunk_len,
        );

        Ok(())
    }

    #[cfg(feature = "std")]
    /// Constructs the parity shards partially using one data shard and
    /// automatically chooses serial/parallel execution.
    pub fn encode_single_sep_opt<U>(
        &self,
        i_data: usize,
        single_data: &[F::Elem],
        parity: &mut [U],
    ) -> Result<(), Error>
    where
        F::Elem: Send + Sync,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]> + Send,
    {
        let decision = self.parallel_policy(single_data.len(), parity.len());
        if decision.use_parallel {
            self.encode_single_sep_par(i_data, single_data, parity)
        } else {
            self.encode_single_sep(i_data, single_data, parity)
        }
    }

    #[cfg(feature = "std")]
    /// Constructs parity shards partially from the indexed data shard and
    /// automatically chooses serial/parallel execution.
    pub fn encode_single_opt<T, U>(&self, i_data: usize, mut shards: T) -> Result<(), Error>
    where
        F::Elem: Send + Sync,
        T: AsRef<[U]> + AsMut<[U]>,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]> + Send,
    {
        let slices = shards.as_mut();

        check_slice_index!(data => self, i_data);
        check_piece_count!(all=> self, slices);
        check_slices!(multi => slices);

        let (mut_input, output) = slices.split_at_mut(self.data_shard_count);
        let input = mut_input[i_data].as_ref();
        let decision = self.parallel_policy(input.len(), output.len());
        let parity_rows = self.get_parity_rows();
        if decision.use_parallel {
            self.code_single_slice_par_chunked(
                &parity_rows,
                i_data,
                input,
                output,
                decision.chunk_len,
            );
        } else {
            self.code_single_slice(&parity_rows, i_data, input, output);
        }
        Ok(())
    }

    #[cfg(feature = "std")]
    /// Constructs parity shards using the std-only parallel fast path when the
    /// shard container satisfies the required thread-safety bounds.
    pub fn encode_par<T, U>(&self, mut shards: T) -> Result<(), Error>
    where
        F::Elem: Send + Sync,
        T: AsRef<[U]> + AsMut<[U]>,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]> + Send + Sync,
    {
        let slices: &mut [U] = shards.as_mut();

        check_piece_count!(all => self, slices);
        check_slices!(multi => slices);

        let (input, output) = slices.split_at_mut(self.data_shard_count);
        self.encode_sep_par(&*input, output)
    }

    #[cfg(feature = "std")]
    /// Checks parity shards using the std-only parallel fast path and caller-provided buffer.
    pub fn verify_with_buffer_par<T, U>(
        &self,
        slices: &[T],
        buffer: &mut [U],
    ) -> Result<bool, Error>
    where
        F::Elem: Send + Sync,
        T: AsRef<[F::Elem]> + Sync,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]> + Send,
    {
        check_piece_count!(all => self, slices);
        check_piece_count!(parity_buf => self, buffer);
        check_slices!(multi => slices, multi => buffer);

        let data = &slices[0..self.data_shard_count];
        let to_check = &slices[self.data_shard_count..];

        if self.fast_one_parity_enabled() {
            self.encode_fast_one_parity(data, buffer);
            return Ok(buffer[0].as_ref() == to_check[0].as_ref());
        }

        self.encode_sep_par(data, buffer)?;

        Ok(buffer
            .iter()
            .zip(to_check.iter())
            .all(|(expected, actual)| expected.as_ref() == actual.as_ref()))
    }

    #[cfg(feature = "std")]
    /// Checks parity shards using the std-only parallel fast path.
    pub fn verify_par<T>(&self, slices: &[T]) -> Result<bool, Error>
    where
        F::Elem: Send + Sync,
        T: AsRef<[F::Elem]> + Sync,
    {
        check_piece_count!(all => self, slices);
        check_slices!(multi => slices);

        let slice_len = slices[0].as_ref().len();
        let mut buffer: SmallVec<[Vec<F::Elem>; 32]> =
            SmallVec::with_capacity(self.parity_shard_count);

        for _ in 0..self.parity_shard_count {
            buffer.push(vec![F::zero(); slice_len]);
        }

        self.verify_with_buffer_par(slices, &mut buffer)
    }

    #[cfg(feature = "std")]
    pub(crate) fn code_some_slices_par_raw(
        &self,
        matrix_rows: &[&[F::Elem]],
        inputs: &[&[F::Elem]],
        outputs: &mut [&mut [F::Elem]],
    ) where
        F::Elem: Send + Sync,
    {
        let shard_len = inputs.first().map(|input| input.len()).unwrap_or(0);
        if shard_len == 0 {
            return;
        }

        let decision = self.parallel_policy(shard_len, outputs.len());
        if !decision.use_parallel {
            self.code_some_slices_chunked(matrix_rows, inputs, outputs);
            return;
        }
        self.code_some_slices_par_chunked(matrix_rows, inputs, outputs, decision.chunk_len);
    }

    #[cfg(feature = "std")]
    pub(crate) fn code_some_slices_with_policy_raw(
        &self,
        matrix_rows: &[&[F::Elem]],
        inputs: &[&[F::Elem]],
        outputs: &mut [&mut [F::Elem]],
        policy: ParallelPolicy,
    ) where
        F::Elem: Send + Sync,
    {
        let shard_len = inputs.first().map(|input| input.len()).unwrap_or(0);
        if shard_len == 0 {
            return;
        }

        let decision = policy.decide(
            shard_len,
            self.data_shard_count,
            outputs.len(),
            std::thread::available_parallelism()
                .map(|parallelism| parallelism.get())
                .unwrap_or(1),
        );
        self.runtime_profile_metrics
            .record_parallel_policy(decision);
        if !decision.use_parallel {
            self.code_some_slices_chunked(matrix_rows, inputs, outputs);
            return;
        }
        self.code_some_slices_par_chunked(matrix_rows, inputs, outputs, decision.chunk_len);
    }

    #[cfg(feature = "std")]
    pub(crate) fn reconstruct_internal_option_vec_par(
        &self,
        shards: &mut [Option<Vec<F::Elem>>],
        data_only: bool,
    ) -> Result<(), Error>
    where
        F::Elem: Send + Sync,
    {
        let (data_policy, parity_policy) = self.policy_cache.reconstruct_stage_policies(data_only);
        self.reconstruct_internal_option_vec_par_with_stage_policies(
            shards,
            data_only,
            data_policy,
            parity_policy,
        )
    }

    #[cfg(feature = "std")]
    pub(crate) fn reconstruct_internal_option_vec_par_with_stage_policies(
        &self,
        shards: &mut [Option<Vec<F::Elem>>],
        data_only: bool,
        data_policy: ParallelPolicy,
        parity_policy: ParallelPolicy,
    ) -> Result<(), Error>
    where
        F::Elem: Send + Sync,
    {
        check_piece_count!(all => self, shards);

        let data_shard_count = self.data_shard_count;

        let mut number_present = 0;
        let mut shard_len = None;
        for shard in shards.iter() {
            if let Some(shard) = shard.as_ref() {
                let len = shard.len();
                if len == 0 {
                    return Err(Error::EmptyShard);
                }
                number_present += 1;
                if let Some(old_len) = shard_len
                    && len != old_len
                {
                    return Err(Error::IncorrectShardSize);
                }
                shard_len = Some(len);
            }
        }

        if number_present == self.total_shard_count {
            self.runtime_profile_metrics
                .record_reconstruct(data_only, 0, 0, true);
            return Ok(());
        }
        if number_present < data_shard_count {
            return Err(Error::TooFewShardsPresent);
        }

        let shard_len = shard_len.expect("at least one shard present; qed");

        let mut valid_indices: SmallVec<[usize; 32]> = SmallVec::with_capacity(data_shard_count);
        let mut invalid_indices: SmallVec<[usize; 32]> =
            SmallVec::with_capacity(self.total_shard_count);
        let mut missing_data_indices: SmallVec<[usize; 32]> = SmallVec::new();
        let mut missing_parity_indices: SmallVec<[usize; 32]> = SmallVec::new();

        for (matrix_row, shard) in shards.iter().enumerate() {
            match shard.as_ref() {
                Some(_shard) => {
                    if valid_indices.len() < data_shard_count {
                        valid_indices.push(matrix_row);
                    }
                }
                None => {
                    invalid_indices.push(matrix_row);
                    if matrix_row < data_shard_count {
                        missing_data_indices.push(matrix_row);
                    } else if !data_only {
                        missing_parity_indices.push(matrix_row);
                    }
                }
            }
        }

        self.runtime_profile_metrics.record_reconstruct(
            data_only,
            missing_data_indices.len(),
            missing_parity_indices.len(),
            false,
        );

        let data_decode_matrix = self.get_data_decode_matrix(&valid_indices, &invalid_indices);

        if !missing_data_indices.is_empty() {
            #[cfg(feature = "std")]
            self.runtime_profile_metrics
                .record_reconstruct_data_stage(shard_len, missing_data_indices.len());
            let mut matrix_rows: SmallVec<[&[F::Elem]; 32]> =
                SmallVec::with_capacity(missing_data_indices.len());
            for &idx in &missing_data_indices {
                matrix_rows.push(data_decode_matrix.get_row(idx));
            }

            let mut recovered_data: Vec<Vec<F::Elem>> = missing_data_indices
                .iter()
                .map(|_| vec![F::zero(); shard_len])
                .collect();

            {
                let mut sub_shards: SmallVec<[&[F::Elem]; 32]> =
                    SmallVec::with_capacity(data_shard_count);
                for &idx in &valid_indices {
                    let shard = shards[idx].as_deref().ok_or(Error::TooFewShardsPresent)?;
                    sub_shards.push(shard);
                }

                let mut outputs: SmallVec<[&mut [F::Elem]; 32]> = recovered_data
                    .iter_mut()
                    .map(|shard| shard.as_mut_slice())
                    .collect();

                self.code_some_slices_with_policy_raw(
                    &matrix_rows,
                    &sub_shards,
                    &mut outputs,
                    data_policy,
                );
            }

            for (idx, recovered) in missing_data_indices.into_iter().zip(recovered_data) {
                shards[idx] = Some(recovered);
            }
        }

        if data_only {
            return Ok(());
        }

        if missing_parity_indices.is_empty() {
            return Ok(());
        }

        #[cfg(feature = "std")]
        self.runtime_profile_metrics
            .record_reconstruct_parity_stage(shard_len, missing_parity_indices.len());
        let parity_rows = self.get_parity_rows();
        let mut matrix_rows: SmallVec<[&[F::Elem]; 32]> =
            SmallVec::with_capacity(missing_parity_indices.len());
        for &idx in &missing_parity_indices {
            matrix_rows.push(parity_rows[idx - data_shard_count]);
        }

        let mut recovered_parity: Vec<Vec<F::Elem>> = missing_parity_indices
            .iter()
            .map(|_| vec![F::zero(); shard_len])
            .collect();

        {
            let mut all_data: SmallVec<[&[F::Elem]; 32]> =
                SmallVec::with_capacity(data_shard_count);
            for shard in shards.iter().take(data_shard_count) {
                let shard = shard.as_deref().ok_or(Error::TooFewShardsPresent)?;
                all_data.push(shard);
            }

            let mut outputs: SmallVec<[&mut [F::Elem]; 32]> = recovered_parity
                .iter_mut()
                .map(|shard| shard.as_mut_slice())
                .collect();

            self.code_some_slices_with_policy_raw(
                &matrix_rows,
                &all_data,
                &mut outputs,
                parity_policy,
            );
        }
        for (idx, recovered) in missing_parity_indices.into_iter().zip(recovered_parity) {
            shards[idx] = Some(recovered);
        }
        Ok(())
    }

    #[cfg(feature = "std")]
    pub(crate) fn reconstruct_internal_option_vec_par_with_policy(
        &self,
        shards: &mut [Option<Vec<F::Elem>>],
        data_only: bool,
        policy: ParallelPolicy,
    ) -> Result<(), Error>
    where
        F::Elem: Send + Sync,
    {
        self.reconstruct_internal_option_vec_par_with_stage_policies(
            shards, data_only, policy, policy,
        )
    }

    fn code_single_slice_range<U: AsMut<[F::Elem]>>(
        &self,
        matrix_rows: &[&[F::Elem]],
        i_input: usize,
        input: &[F::Elem],
        outputs: &mut [U],
        start: usize,
        end: usize,
    ) {
        let input = &input[start..end];
        outputs.iter_mut().enumerate().for_each(|(i_row, output)| {
            let matrix_row_to_use = matrix_rows[i_row][i_input];
            let output = &mut output.as_mut()[start..end];

            if i_input == 0 {
                F::mul_slice(matrix_row_to_use, input, output);
            } else {
                F::mul_slice_add(matrix_row_to_use, input, output);
            }
        })
    }

    fn code_single_slice<U: AsMut<[F::Elem]>>(
        &self,
        matrix_rows: &[&[F::Elem]],
        i_input: usize,
        input: &[F::Elem],
        outputs: &mut [U],
    ) {
        #[cfg(feature = "std")]
        self.runtime_profile_metrics.record_code_single(
            false,
            input.len(),
            outputs.len(),
            input.len(),
        );
        self.code_single_slice_range(matrix_rows, i_input, input, outputs, 0, input.len());
    }

    fn fast_one_parity_enabled(&self) -> bool {
        self.options.fast_one_parity && self.parity_shard_count == 1
    }

    fn encode_fast_one_parity<T: AsRef<[F::Elem]>, U: AsRef<[F::Elem]> + AsMut<[F::Elem]>>(
        &self,
        data: &[T],
        parity: &mut [U],
    ) {
        let output = parity[0].as_mut();
        output.copy_from_slice(data[0].as_ref());
        for input in &data[1..] {
            for (out, value) in output.iter_mut().zip(input.as_ref().iter()) {
                *out = F::add(*out, *value);
            }
        }
    }

    fn check_some_slices_with_buffer<T, U>(
        &self,
        matrix_rows: &[&[F::Elem]],
        inputs: &[T],
        to_check: &[T],
        buffer: &mut [U],
    ) -> bool
    where
        T: AsRef<[F::Elem]>,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]>,
    {
        self.code_some_slices(matrix_rows, inputs, buffer);

        for (expected_parity_shard, actual_parity_shard) in buffer.iter().zip(to_check.iter()) {
            if expected_parity_shard.as_ref() != actual_parity_shard.as_ref() {
                return false;
            }
        }
        true
    }

    fn check_some_slices_with_buffer_raw<T: AsRef<[F::Elem]>>(
        &self,
        matrix_rows: &[&[F::Elem]],
        inputs: &[T],
        to_check: &[T],
        buffer: &mut [&mut [F::Elem]],
    ) -> bool {
        self.code_some_slices(matrix_rows, inputs, buffer);

        for (expected_parity_shard, actual_parity_shard) in buffer.iter().zip(to_check.iter()) {
            if *expected_parity_shard != actual_parity_shard.as_ref() {
                return false;
            }
        }
        true
    }

    /// Constructs the parity shards partially using only the data shard
    /// indexed by `i_data`.
    ///
    /// The slots where the parity shards sit at will be overwritten.
    ///
    /// # Warning
    ///
    /// You must apply this method on the data shards in strict sequential order (0..data shard count),
    /// otherwise the parity shards will be incorrect.
    ///
    /// It is recommended to use the `ShardByShard` bookkeeping struct instead of this method directly.
    pub fn encode_single<T, U>(&self, i_data: usize, mut shards: T) -> Result<(), Error>
    where
        T: AsRef<[U]> + AsMut<[U]>,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]>,
    {
        let slices = shards.as_mut();

        check_slice_index!(data => self, i_data);
        check_piece_count!(all=> self, slices);
        check_slices!(multi => slices);

        // Get the slice of output buffers.
        let (mut_input, output) = slices.split_at_mut(self.data_shard_count);

        let input = mut_input[i_data].as_ref();

        self.encode_single_sep(i_data, input, output)
    }

    /// Constructs the parity shards partially using only the data shard provided.
    ///
    /// The data shard must match the index `i_data`.
    ///
    /// The slots where the parity shards sit at will be overwritten.
    ///
    /// # Warning
    ///
    /// You must apply this method on the data shards in strict sequential order (0..data shard count),
    /// otherwise the parity shards will be incorrect.
    ///
    /// It is recommended to use the `ShardByShard` bookkeeping struct instead of this method directly.
    pub fn encode_single_sep<U: AsRef<[F::Elem]> + AsMut<[F::Elem]>>(
        &self,
        i_data: usize,
        single_data: &[F::Elem],
        parity: &mut [U],
    ) -> Result<(), Error> {
        check_slice_index!(data => self, i_data);
        check_piece_count!(parity => self, parity);
        check_slices!(multi => parity, single => single_data);

        let parity_rows = self.get_parity_rows();

        // Do the coding.
        self.code_single_slice(&parity_rows, i_data, single_data, parity);

        Ok(())
    }

    /// Constructs the parity shards.
    ///
    /// The slots where the parity shards sit at will be overwritten.
    pub fn encode<T, U>(&self, mut shards: T) -> Result<(), Error>
    where
        T: AsRef<[U]> + AsMut<[U]>,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]>,
    {
        let slices: &mut [U] = shards.as_mut();

        check_piece_count!(all => self, slices);
        check_slices!(multi => slices);

        // Get the slice of output buffers.
        let (input, output) = slices.split_at_mut(self.data_shard_count);

        self.encode_sep(&*input, output)
    }

    /// Constructs the parity shards using a read-only view into the
    /// data shards.
    ///
    /// The slots where the parity shards sit at will be overwritten.
    pub fn encode_sep<T: AsRef<[F::Elem]>, U: AsRef<[F::Elem]> + AsMut<[F::Elem]>>(
        &self,
        data: &[T],
        parity: &mut [U],
    ) -> Result<(), Error> {
        check_piece_count!(data => self, data);
        check_piece_count!(parity => self, parity);
        check_slices!(multi => data, multi => parity);

        if self.fast_one_parity_enabled() {
            self.encode_fast_one_parity(data, parity);
            return Ok(());
        }

        let parity_rows = self.get_parity_rows();

        // Do the coding.
        self.code_some_slices(&parity_rows, data, parity);

        Ok(())
    }

    /// Checks if the parity shards are correct.
    ///
    /// This is a wrapper of `verify_with_buffer`.
    pub fn verify<T: AsRef<[F::Elem]>>(&self, slices: &[T]) -> Result<bool, Error> {
        check_piece_count!(all => self, slices);
        check_slices!(multi => slices);

        let slice_len = slices[0].as_ref().len();
        let data = &slices[0..self.data_shard_count];
        let to_check = &slices[self.data_shard_count..];

        if self.fast_one_parity_enabled() {
            let mut buffer = vec![F::zero(); slice_len];
            self.encode_fast_one_parity(data, core::slice::from_mut(&mut buffer));
            return Ok(buffer.as_slice() == to_check[0].as_ref());
        }

        let parity_rows = self.get_parity_rows();
        let mut scratch = vec![F::zero(); self.parity_shard_count * slice_len];
        let mut buffer_views: SmallVec<[&mut [F::Elem]; 32]> =
            scratch.chunks_mut(slice_len).collect();

        Ok(self.check_some_slices_with_buffer_raw(&parity_rows, data, to_check, &mut buffer_views))
    }

    /// Checks if the parity shards are correct.
    pub fn verify_with_buffer<T, U>(&self, slices: &[T], buffer: &mut [U]) -> Result<bool, Error>
    where
        T: AsRef<[F::Elem]>,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]>,
    {
        check_piece_count!(all => self, slices);
        check_piece_count!(parity_buf => self, buffer);
        check_slices!(multi => slices, multi => buffer);

        let data = &slices[0..self.data_shard_count];
        let to_check = &slices[self.data_shard_count..];

        if self.fast_one_parity_enabled() {
            self.encode_fast_one_parity(data, buffer);
            return Ok(buffer[0].as_ref() == to_check[0].as_ref());
        }

        let parity_rows = self.get_parity_rows();

        Ok(self.check_some_slices_with_buffer(&parity_rows, data, to_check, buffer))
    }

    /// Reconstructs all shards.
    ///
    /// The shards marked not present are only overwritten when no error
    /// is detected. All provided shards must have the same length.
    ///
    /// This means if the method returns an `Error`, then nothing is touched.
    ///
    /// `reconstruct`, `reconstruct_data`, `reconstruct_shards`,
    /// `reconstruct_data_shards` share the same core code base.
    pub fn reconstruct<T: ReconstructShard<F>>(&self, slices: &mut [T]) -> Result<(), Error> {
        self.reconstruct_internal(slices, false)
    }

    /// Reconstructs only the data shards.
    ///
    /// The shards marked not present are only overwritten when no error
    /// is detected. All provided shards must have the same length.
    ///
    /// This means if the method returns an `Error`, then nothing is touched.
    ///
    /// `reconstruct`, `reconstruct_data`, `reconstruct_shards`,
    /// `reconstruct_data_shards` share the same core code base.
    pub fn reconstruct_data<T: ReconstructShard<F>>(&self, slices: &mut [T]) -> Result<(), Error> {
        self.reconstruct_internal(slices, true)
    }

    pub fn reconstruct_some<T: ReconstructShard<F>>(
        &self,
        shards: &mut [T],
        required: &[bool],
    ) -> Result<(), Error> {
        if required.len() != self.total_shard_count {
            return Err(Error::InvalidShardFlags);
        }

        check_piece_count!(all => self, shards);

        let mut number_present = 0;
        let mut shard_len = None;

        for shard in shards.iter_mut() {
            if let Some(len) = shard.len() {
                if len == 0 {
                    return Err(Error::EmptyShard);
                }
                number_present += 1;
                if let Some(old_len) = shard_len
                    && len != old_len
                {
                    return Err(Error::IncorrectShardSize);
                }
                shard_len = Some(len);
            }
        }

        if number_present == self.total_shard_count {
            return Ok(());
        }

        if number_present < self.data_shard_count {
            return Err(Error::TooFewShardsPresent);
        }

        let shard_len = shard_len.expect("at least one shard present; qed");
        let required_data_only = required
            .iter()
            .enumerate()
            .all(|(i, required)| !*required || i < self.data_shard_count);

        let originally_missing: Vec<bool> = shards
            .iter_mut()
            .map(|shard| shard.get().is_none())
            .collect();

        if required_data_only {
            let mut valid_indices: SmallVec<[usize; 32]> =
                SmallVec::with_capacity(self.data_shard_count);
            let mut invalid_indices: SmallVec<[usize; 32]> =
                SmallVec::with_capacity(self.total_shard_count);
            for (index, shard) in shards.iter_mut().enumerate() {
                if shard.get().is_some() {
                    if valid_indices.len() < self.data_shard_count {
                        valid_indices.push(index);
                    }
                } else {
                    invalid_indices.push(index);
                }
            }

            let data_decode_matrix = self.get_data_decode_matrix(&valid_indices, &invalid_indices);
            let sub_shards_snapshot: Vec<Vec<F::Elem>> = valid_indices
                .iter()
                .map(|&idx| {
                    shards[idx]
                        .get()
                        .expect("valid shard index must be present")
                        .to_vec()
                })
                .collect();
            let sub_shards: SmallVec<[&[F::Elem]; 32]> = sub_shards_snapshot
                .iter()
                .map(|shard| shard.as_slice())
                .collect();

            for i in 0..self.data_shard_count {
                if !required[i] || !originally_missing[i] {
                    continue;
                }

                let mut recovered = vec![F::zero(); shard_len];
                let matrix_rows = [data_decode_matrix.get_row(i)];
                let mut outputs = [&mut recovered[..]];
                self.code_some_slices(&matrix_rows, &sub_shards, &mut outputs);

                match shards[i].get_or_initialize(shard_len) {
                    Ok(dst) | Err(Ok(dst)) => dst.copy_from_slice(&recovered),
                    Err(Err(err)) => return Err(err),
                }
            }
        } else {
            let mut working: Vec<Option<Vec<F::Elem>>> = shards
                .iter_mut()
                .map(|shard| shard.get().map(|data| data.to_vec()))
                .collect();
            self.reconstruct(&mut working)?;

            for (i, shard) in shards.iter_mut().enumerate() {
                if !required[i] || !originally_missing[i] {
                    continue;
                }

                let recovered = working[i]
                    .as_ref()
                    .expect("recovered shard must be present");
                match shard.get_or_initialize(shard_len) {
                    Ok(dst) | Err(Ok(dst)) => dst.copy_from_slice(recovered),
                    Err(Err(err)) => return Err(err),
                }
            }
        }

        Ok(())
    }

    pub(crate) fn get_data_decode_matrix(
        &self,
        valid_indices: &[usize],
        invalid_indices: &[usize],
    ) -> Arc<Matrix<F>> {
        if self.options.inversion_cache {
            #[cfg(feature = "std")]
            self.reconstruction_cache_metrics
                .requests
                .fetch_add(1, Ordering::Relaxed);

            let mut cache = self.data_decode_matrix_cache.lock();
            if let Some(entry) = cache.get(invalid_indices) {
                #[cfg(feature = "std")]
                self.reconstruction_cache_metrics
                    .hits
                    .fetch_add(1, Ordering::Relaxed);
                return entry.clone();
            }

            #[cfg(feature = "std")]
            self.reconstruction_cache_metrics
                .misses
                .fetch_add(1, Ordering::Relaxed);
        }
        // Pull out the rows of the matrix that correspond to the shards that
        // we have and build a square matrix. This matrix could be used to
        // generate the shards that we have from the original data.
        let mut sub_matrix = Matrix::new(self.data_shard_count, self.data_shard_count);
        for (sub_matrix_row, &valid_index) in valid_indices.iter().enumerate() {
            for c in 0..self.data_shard_count {
                sub_matrix.set(sub_matrix_row, c, self.matrix.get(valid_index, c));
            }
        }
        // Invert the matrix, so we can go from the encoded shards back to the
        // original data. Then pull out the row that generates the shard that
        // we want to decode. Note that since this matrix maps back to the
        // original data, it can be used to create a data shard, but not a
        // parity shard.
        let data_decode_matrix = match sub_matrix.invert() {
            Ok(inverted) => Arc::new(inverted),
            Err(_) => panic!(
                "selected shard submatrix must remain invertible when enough shards are present"
            ),
        };
        // Cache the inverted matrix for future use keyed on the indices of the
        // invalid rows.
        if self.options.inversion_cache {
            let data_decode_matrix = data_decode_matrix.clone();
            let mut cache = self.data_decode_matrix_cache.lock();
            #[cfg(feature = "std")]
            let before_len = cache.len();
            #[cfg(feature = "std")]
            let capacity = cache.capacity();
            cache.insert(Vec::from(invalid_indices), data_decode_matrix);
            #[cfg(feature = "std")]
            if capacity > 0 && before_len >= capacity {
                self.reconstruction_cache_metrics
                    .evictions
                    .fetch_add(1, Ordering::Relaxed);
            }
            #[cfg(feature = "std")]
            self.reconstruction_cache_metrics
                .inserts
                .fetch_add(1, Ordering::Relaxed);
        }
        data_decode_matrix
    }

    fn reconstruct_internal<T: ReconstructShard<F>>(
        &self,
        shards: &mut [T],
        data_only: bool,
    ) -> Result<(), Error> {
        check_piece_count!(all => self, shards);

        let data_shard_count = self.data_shard_count;

        // Quick check: are all of the shards present?  If so, there's
        // nothing to do.
        let mut number_present = 0;
        let mut shard_len = None;

        for shard in shards.iter_mut() {
            if let Some(len) = shard.len() {
                if len == 0 {
                    return Err(Error::EmptyShard);
                }
                number_present += 1;
                if let Some(old_len) = shard_len
                    && len != old_len
                {
                    // mismatch between shards.
                    return Err(Error::IncorrectShardSize);
                }
                shard_len = Some(len);
            }
        }

        if number_present == self.total_shard_count {
            // Cool.  All of the shards are there.  We don't
            // need to do anything.
            #[cfg(feature = "std")]
            self.runtime_profile_metrics
                .record_reconstruct(data_only, 0, 0, true);
            return Ok(());
        }

        // More complete sanity check
        if number_present < data_shard_count {
            return Err(Error::TooFewShardsPresent);
        }

        let shard_len = shard_len.expect("at least one shard present; qed");

        // Pull out an array holding just the shards that
        // correspond to the rows of the submatrix.  These shards
        // will be the input to the decoding process that re-creates
        // the missing data shards.
        //
        // Also, create an array of indices of the valid rows we do have
        // and the invalid rows we don't have.
        //
        // The valid indices are used to construct the data decode matrix,
        // the invalid indices are used to key the data decode matrix
        // in the data decode matrix cache.
        //
        // We only need exactly N valid indices, where N = `data_shard_count`,
        // as the data decode matrix is a N x N matrix, thus only needs
        // N valid indices for determining the N rows to pick from
        // `self.matrix`.
        let mut sub_shards: SmallVec<[&[F::Elem]; 32]> = SmallVec::with_capacity(data_shard_count);
        let mut missing_data_slices: SmallVec<[&mut [F::Elem]; 32]> =
            SmallVec::with_capacity(self.parity_shard_count);
        let mut missing_parity_slices: SmallVec<[&mut [F::Elem]; 32]> =
            SmallVec::with_capacity(self.parity_shard_count);
        let mut valid_indices: SmallVec<[usize; 32]> = SmallVec::with_capacity(data_shard_count);
        let mut invalid_indices: SmallVec<[usize; 32]> = SmallVec::with_capacity(data_shard_count);

        // Separate the shards into groups
        for (matrix_row, shard) in shards.iter_mut().enumerate() {
            // get or initialize the shard so we can reconstruct in-place,
            // but if we are only reconstructing data shard,
            // do not initialize if the shard is not a data shard
            let shard_data = if matrix_row >= data_shard_count && data_only {
                shard.get().ok_or(None)
            } else {
                shard.get_or_initialize(shard_len).map_err(Some)
            };

            match shard_data {
                Ok(shard) => {
                    if sub_shards.len() < data_shard_count {
                        sub_shards.push(shard);
                        valid_indices.push(matrix_row);
                    } else {
                        // Already have enough shards in `sub_shards`
                        // as we only need N shards, where N = `data_shard_count`,
                        // for the data decode matrix
                        //
                        // So nothing to do here
                    }
                }
                Err(None) => {
                    // the shard data is not meant to be initialized here,
                    // but we should still note it missing.
                    invalid_indices.push(matrix_row);
                }
                Err(Some(x)) => {
                    // initialized missing shard data.
                    let shard = x?;
                    if matrix_row < data_shard_count {
                        missing_data_slices.push(shard);
                    } else {
                        missing_parity_slices.push(shard);
                    }

                    invalid_indices.push(matrix_row);
                }
            }
        }

        #[cfg(feature = "std")]
        {
            let missing_data_count = invalid_indices
                .iter()
                .filter(|&&i| i < data_shard_count)
                .count();
            let missing_parity_count = if data_only {
                0
            } else {
                invalid_indices
                    .iter()
                    .filter(|&&i| i >= data_shard_count)
                    .count()
            };
            self.runtime_profile_metrics.record_reconstruct(
                data_only,
                missing_data_count,
                missing_parity_count,
                false,
            );
        }

        let data_decode_matrix = self.get_data_decode_matrix(&valid_indices, &invalid_indices);

        // Re-create any data shards that were missing.
        //
        // The input to the coding is all of the shards we actually
        // have, and the output is the missing data shards. The computation
        // is done using the special decode matrix we just built.
        let mut matrix_rows: SmallVec<[&[F::Elem]; 32]> =
            SmallVec::with_capacity(self.parity_shard_count);

        for i_slice in invalid_indices
            .iter()
            .cloned()
            .take_while(|i| i < &data_shard_count)
        {
            matrix_rows.push(data_decode_matrix.get_row(i_slice));
        }

        #[cfg(feature = "std")]
        self.runtime_profile_metrics
            .record_reconstruct_data_stage(shard_len, matrix_rows.len());
        self.code_some_slices(&matrix_rows, &sub_shards, &mut missing_data_slices);

        if data_only {
            Ok(())
        } else {
            // Now that we have all of the data shards intact, we can
            // compute any of the parity that is missing.
            //
            // The input to the coding is ALL of the data shards, including
            // any that we just calculated.  The output is whichever of the
            // parity shards were missing.
            let mut matrix_rows: SmallVec<[&[F::Elem]; 32]> =
                SmallVec::with_capacity(self.parity_shard_count);
            let parity_rows = self.get_parity_rows();

            for i_slice in invalid_indices
                .iter()
                .cloned()
                .skip_while(|i| i < &data_shard_count)
            {
                matrix_rows.push(parity_rows[i_slice - data_shard_count]);
            }
            #[cfg(feature = "std")]
            self.runtime_profile_metrics
                .record_reconstruct_parity_stage(shard_len, matrix_rows.len());
            {
                // Gather up all the data shards.
                // old data shards are in `sub_shards`,
                // new ones are in `missing_data_slices`.
                let mut i_old_data_slice = 0;
                let mut i_new_data_slice = 0;

                let mut all_data_slices: SmallVec<[&[F::Elem]; 32]> =
                    SmallVec::with_capacity(data_shard_count);

                let mut next_maybe_good = 0;
                let mut push_good_up_to = move |data_slices: &mut SmallVec<_>, up_to| {
                    // if next_maybe_good == up_to, this loop is a no-op.
                    for _ in next_maybe_good..up_to {
                        // push all good indices we just skipped.
                        data_slices.push(sub_shards[i_old_data_slice]);
                        i_old_data_slice += 1;
                    }

                    next_maybe_good = up_to + 1;
                };

                for i_slice in invalid_indices
                    .iter()
                    .cloned()
                    .take_while(|i| i < &data_shard_count)
                {
                    push_good_up_to(&mut all_data_slices, i_slice);
                    all_data_slices.push(missing_data_slices[i_new_data_slice]);
                    i_new_data_slice += 1;
                }
                push_good_up_to(&mut all_data_slices, data_shard_count);

                // Now do the actual computation for the missing
                // parity shards
                self.code_some_slices(&matrix_rows, &all_data_slices, &mut missing_parity_slices);
            }

            Ok(())
        }
    }
}
