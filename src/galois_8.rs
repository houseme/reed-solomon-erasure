//! Implementation of GF(2^8): the finite field with 2^8 elements.

include!(concat!(env!("OUT_DIR"), "/table.rs"));

mod backend;

pub use backend::BackendKind;

#[cfg(feature = "std")]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "std")]
use std::sync::OnceLock;

/// The field GF(2^8).
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Field;

impl crate::Field for Field {
    const ORDER: usize = 256;
    type Elem = u8;

    fn add(a: u8, b: u8) -> u8 {
        add(a, b)
    }

    fn mul(a: u8, b: u8) -> u8 {
        mul(a, b)
    }

    fn div(a: u8, b: u8) -> u8 {
        div(a, b)
    }

    fn exp(elem: u8, n: usize) -> u8 {
        exp(elem, n)
    }

    fn zero() -> u8 {
        0
    }

    fn one() -> u8 {
        1
    }

    fn nth_internal(n: usize) -> u8 {
        n as u8
    }

    fn mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
        mul_slice(c, input, out)
    }

    fn mul_slice_add(c: u8, input: &[u8], out: &mut [u8]) {
        mul_slice_xor(c, input, out)
    }
}

/// Type alias of ReedSolomon over GF(2^8).
pub type ReedSolomon = crate::ReedSolomon<Field>;

/// Type alias of ShardByShard over GF(2^8).
pub type ShardByShard<'a> = crate::ShardByShard<'a, Field>;

#[cfg(feature = "std")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RustNeonProfileStats {
    pub mul_calls: usize,
    pub mul_xor_calls: usize,
    pub total_bytes: usize,
    pub vector_64b_chunks: usize,
    pub vector_16b_chunks: usize,
    pub tail_bytes: usize,
    pub tail_calls: usize,
    pub table_lookups: usize,
}

#[cfg(feature = "std")]
impl RustNeonProfileStats {
    pub fn saturating_sub(self, baseline: Self) -> Self {
        Self {
            mul_calls: self.mul_calls.saturating_sub(baseline.mul_calls),
            mul_xor_calls: self.mul_xor_calls.saturating_sub(baseline.mul_xor_calls),
            total_bytes: self.total_bytes.saturating_sub(baseline.total_bytes),
            vector_64b_chunks: self
                .vector_64b_chunks
                .saturating_sub(baseline.vector_64b_chunks),
            vector_16b_chunks: self
                .vector_16b_chunks
                .saturating_sub(baseline.vector_16b_chunks),
            tail_bytes: self.tail_bytes.saturating_sub(baseline.tail_bytes),
            tail_calls: self.tail_calls.saturating_sub(baseline.tail_calls),
            table_lookups: self.table_lookups.saturating_sub(baseline.table_lookups),
        }
    }
}

#[cfg(feature = "std")]
#[derive(Debug, Default)]
struct RustNeonProfileMetrics {
    mul_calls: AtomicUsize,
    mul_xor_calls: AtomicUsize,
    total_bytes: AtomicUsize,
    vector_64b_chunks: AtomicUsize,
    vector_16b_chunks: AtomicUsize,
    tail_bytes: AtomicUsize,
    tail_calls: AtomicUsize,
    table_lookups: AtomicUsize,
}

#[cfg(feature = "std")]
impl RustNeonProfileMetrics {
    fn snapshot(&self) -> RustNeonProfileStats {
        RustNeonProfileStats {
            mul_calls: self.mul_calls.load(Ordering::Relaxed),
            mul_xor_calls: self.mul_xor_calls.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            vector_64b_chunks: self.vector_64b_chunks.load(Ordering::Relaxed),
            vector_16b_chunks: self.vector_16b_chunks.load(Ordering::Relaxed),
            tail_bytes: self.tail_bytes.load(Ordering::Relaxed),
            tail_calls: self.tail_calls.load(Ordering::Relaxed),
            table_lookups: self.table_lookups.load(Ordering::Relaxed),
        }
    }

    fn reset(&self) {
        self.mul_calls.store(0, Ordering::Relaxed);
        self.mul_xor_calls.store(0, Ordering::Relaxed);
        self.total_bytes.store(0, Ordering::Relaxed);
        self.vector_64b_chunks.store(0, Ordering::Relaxed);
        self.vector_16b_chunks.store(0, Ordering::Relaxed);
        self.tail_bytes.store(0, Ordering::Relaxed);
        self.tail_calls.store(0, Ordering::Relaxed);
        self.table_lookups.store(0, Ordering::Relaxed);
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    ))]
    fn record_call(
        &self,
        is_xor: bool,
        input_len: usize,
        vector_64b_chunks: usize,
        vector_16b_chunks: usize,
        tail_bytes: usize,
    ) {
        if is_xor {
            self.mul_xor_calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.mul_calls.fetch_add(1, Ordering::Relaxed);
        }
        self.total_bytes.fetch_add(input_len, Ordering::Relaxed);
        self.vector_64b_chunks
            .fetch_add(vector_64b_chunks, Ordering::Relaxed);
        self.vector_16b_chunks
            .fetch_add(vector_16b_chunks, Ordering::Relaxed);
        if tail_bytes > 0 {
            self.tail_calls.fetch_add(1, Ordering::Relaxed);
            self.tail_bytes.fetch_add(tail_bytes, Ordering::Relaxed);
        }
        let lookups = vector_64b_chunks
            .saturating_mul(8)
            .saturating_add(vector_16b_chunks.saturating_mul(2));
        self.table_lookups.fetch_add(lookups, Ordering::Relaxed);
    }
}

#[cfg(feature = "std")]
static RUST_NEON_PROFILE_METRICS: RustNeonProfileMetrics = RustNeonProfileMetrics {
    mul_calls: AtomicUsize::new(0),
    mul_xor_calls: AtomicUsize::new(0),
    total_bytes: AtomicUsize::new(0),
    vector_64b_chunks: AtomicUsize::new(0),
    vector_16b_chunks: AtomicUsize::new(0),
    tail_bytes: AtomicUsize::new(0),
    tail_calls: AtomicUsize::new(0),
    table_lookups: AtomicUsize::new(0),
};

#[cfg(feature = "std")]
const RS_NEON_MUL_SLICE_XOR_UNROLL_ENV: &str = "RS_NEON_MUL_SLICE_XOR_UNROLL";
#[cfg(feature = "std")]
const RS_NEON_MUL_SLICE_XOR_SCHEDULE_ENV: &str = "RS_NEON_MUL_SLICE_XOR_SCHEDULE";

#[cfg(feature = "std")]
fn parse_rust_neon_xor_unroll(value: &str) -> Option<usize> {
    match value {
        "2" => Some(2),
        "4" => Some(4),
        _ => None,
    }
}

#[cfg(feature = "std")]
fn rust_neon_mul_slice_xor_unroll() -> usize {
    static UNROLL: OnceLock<usize> = OnceLock::new();
    *UNROLL.get_or_init(|| {
        std::env::var(RS_NEON_MUL_SLICE_XOR_UNROLL_ENV)
            .ok()
            .as_deref()
            .and_then(parse_rust_neon_xor_unroll)
            .unwrap_or(4)
    })
}

#[cfg(feature = "std")]
fn rust_neon_mul_slice_xor_schedule_split() -> bool {
    static SPLIT: OnceLock<bool> = OnceLock::new();
    *SPLIT.get_or_init(|| {
        std::env::var(RS_NEON_MUL_SLICE_XOR_SCHEDULE_ENV)
            .ok()
            .is_some_and(|value| value == "split")
    })
}

#[cfg(feature = "std")]
pub fn rust_neon_profile_stats() -> RustNeonProfileStats {
    RUST_NEON_PROFILE_METRICS.snapshot()
}

#[cfg(feature = "std")]
pub fn reset_rust_neon_profile_stats() {
    RUST_NEON_PROFILE_METRICS.reset();
}

#[cfg(feature = "std")]
const RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES: usize = 512 * 1024;
#[cfg(feature = "std")]
const RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES: usize = 256 * 1024;
#[cfg(feature = "std")]
const RS_RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES_ENV: &str =
    "RS_RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES";
#[cfg(feature = "std")]
const RS_RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES_ENV: &str =
    "RS_RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES";
#[cfg(feature = "std")]
const RS_RECONSTRUCT_MIN_BYTES_PER_JOB_ENV: &str = "RS_RECONSTRUCT_MIN_BYTES_PER_JOB";

impl crate::ReedSolomon<Field> {
    #[cfg(feature = "std")]
    fn read_env_usize(name: &str) -> Option<usize> {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
    }

    #[cfg(feature = "std")]
    fn reconstruct_data_min_parallel_shard_bytes(&self) -> usize {
        Self::read_env_usize(RS_RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES_ENV)
            .filter(|value| *value > 0)
            .unwrap_or(RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES)
    }

    #[cfg(feature = "std")]
    fn reconstruct_full_min_parallel_shard_bytes(&self) -> usize {
        Self::read_env_usize(RS_RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES_ENV)
            .filter(|value| *value > 0)
            .unwrap_or(RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES)
    }

    #[cfg(feature = "std")]
    fn reconstruct_min_bytes_per_job(&self) -> Option<usize> {
        Self::read_env_usize(RS_RECONSTRUCT_MIN_BYTES_PER_JOB_ENV).filter(|value| *value > 0)
    }

    #[cfg(feature = "std")]
    fn first_shard_len<T: AsRef<[u8]>>(slices: &[T]) -> usize {
        slices
            .first()
            .map(|shard| shard.as_ref().len())
            .unwrap_or(0)
    }

    #[cfg(feature = "std")]
    fn first_present_shard_len(shards: &[Option<Vec<u8>>]) -> usize {
        shards
            .iter()
            .find_map(|shard| shard.as_ref().map(|shard| shard.len()))
            .unwrap_or(0)
    }

    #[cfg(feature = "std")]
    fn should_parallel_data_path(&self, shard_len: usize, output_shards: usize) -> bool {
        self.parallel_policy(shard_len, output_shards).use_parallel
    }

    #[cfg(feature = "std")]
    pub(crate) fn reconstruct_parallel_decision_with(
        &self,
        shard_len: usize,
        missing_data: usize,
        missing_total: usize,
        data_only: bool,
        available_parallelism: usize,
    ) -> crate::ParallelDecision {
        let base = self.effective_parallel_policy();
        let data_only_min = self.reconstruct_data_min_parallel_shard_bytes();
        let full_min = self.reconstruct_full_min_parallel_shard_bytes();
        let min_bytes_per_job = self
            .reconstruct_min_bytes_per_job()
            .unwrap_or(base.min_bytes_per_job);
        let tuned = if data_only {
            crate::ParallelPolicy {
                min_parallel_shard_bytes: core::cmp::max(
                    base.min_parallel_shard_bytes,
                    data_only_min,
                ),
                min_bytes_per_job,
                max_jobs: base.max_jobs,
            }
        } else {
            crate::ParallelPolicy {
                min_parallel_shard_bytes: core::cmp::max(
                    base.min_parallel_shard_bytes / 2,
                    full_min,
                ),
                min_bytes_per_job,
                max_jobs: base.max_jobs,
            }
        };
        let output_shards = if data_only {
            missing_data
        } else {
            missing_total
        };
        tuned.decide(
            shard_len,
            self.data_shard_count(),
            output_shards,
            available_parallelism,
        )
    }

    #[cfg(feature = "std")]
    fn reconstruct_parallel_policy(&self, data_only: bool) -> crate::ParallelPolicy {
        let base = self.effective_parallel_policy();
        let data_only_min = self.reconstruct_data_min_parallel_shard_bytes();
        let full_min = self.reconstruct_full_min_parallel_shard_bytes();
        let min_bytes_per_job = self
            .reconstruct_min_bytes_per_job()
            .unwrap_or(base.min_bytes_per_job);
        if data_only {
            crate::ParallelPolicy {
                min_parallel_shard_bytes: core::cmp::max(
                    base.min_parallel_shard_bytes,
                    data_only_min,
                ),
                min_bytes_per_job,
                max_jobs: base.max_jobs,
            }
        } else {
            crate::ParallelPolicy {
                min_parallel_shard_bytes: core::cmp::max(
                    base.min_parallel_shard_bytes / 2,
                    full_min,
                ),
                min_bytes_per_job,
                max_jobs: base.max_jobs,
            }
        }
    }

    #[cfg(feature = "std")]
    fn reconstruct_parallel_decision(
        &self,
        shard_len: usize,
        missing_data: usize,
        missing_total: usize,
        data_only: bool,
    ) -> crate::ParallelDecision {
        self.reconstruct_parallel_decision_with(
            shard_len,
            missing_data,
            missing_total,
            data_only,
            std::thread::available_parallelism()
                .map(|parallelism| parallelism.get())
                .unwrap_or(1),
        )
    }

    #[cfg(feature = "std")]
    pub fn encode_opt<T, U>(&self, shards: T) -> Result<(), crate::Error>
    where
        T: AsRef<[U]> + AsMut<[U]>,
        U: AsRef<[u8]> + AsMut<[u8]> + Send + Sync,
    {
        let shard_len = Self::first_shard_len(shards.as_ref());
        if self.should_parallel_data_path(shard_len, self.parity_shard_count()) {
            self.encode_par(shards)
        } else {
            self.encode(shards)
        }
    }

    #[cfg(feature = "std")]
    pub fn encode_sep_opt<T, U>(&self, data: &[T], parity: &mut [U]) -> Result<(), crate::Error>
    where
        T: AsRef<[u8]> + Sync,
        U: AsRef<[u8]> + AsMut<[u8]> + Send,
    {
        let shard_len = Self::first_shard_len(data);
        if self.should_parallel_data_path(shard_len, parity.len()) {
            self.encode_sep_par(data, parity)
        } else {
            self.encode_sep(data, parity)
        }
    }

    #[cfg(feature = "std")]
    pub fn verify_opt<T>(&self, slices: &[T]) -> Result<bool, crate::Error>
    where
        T: AsRef<[u8]> + Sync,
    {
        let shard_len = Self::first_shard_len(slices);
        if self.should_parallel_data_path(shard_len, self.parity_shard_count()) {
            self.verify_par(slices)
        } else {
            self.verify(slices)
        }
    }

    #[cfg(feature = "std")]
    pub fn verify_with_buffer_opt<T, U>(
        &self,
        slices: &[T],
        buffer: &mut [U],
    ) -> Result<bool, crate::Error>
    where
        T: AsRef<[u8]> + Sync,
        U: AsRef<[u8]> + AsMut<[u8]> + Send,
    {
        let shard_len = Self::first_shard_len(slices);
        if self.should_parallel_data_path(shard_len, buffer.len()) {
            self.verify_with_buffer_par(slices, buffer)
        } else {
            self.verify_with_buffer(slices, buffer)
        }
    }

    #[cfg(feature = "std")]
    pub fn reconstruct_opt(&self, shards: &mut [Option<Vec<u8>>]) -> Result<(), crate::Error> {
        let shard_len = Self::first_present_shard_len(shards);
        let missing_data = shards
            .iter()
            .take(self.data_shard_count())
            .filter(|shard| shard.is_none())
            .count();
        let missing = shards.iter().filter(|shard| shard.is_none()).count();
        if self
            .reconstruct_parallel_decision(shard_len, missing_data, missing, false)
            .use_parallel
        {
            self.reconstruct_internal_option_vec_par_with_policy(
                shards,
                false,
                self.reconstruct_parallel_policy(false),
            )
        } else {
            self.reconstruct(shards)
        }
    }

    #[cfg(feature = "std")]
    pub fn reconstruct_data_opt(
        &self,
        shards: &mut [Option<Vec<u8>>],
    ) -> Result<(), crate::Error> {
        let shard_len = Self::first_present_shard_len(shards);
        let missing_data = shards
            .iter()
            .take(self.data_shard_count())
            .filter(|shard| shard.is_none())
            .count();
        let missing = shards.iter().filter(|shard| shard.is_none()).count();
        if self
            .reconstruct_parallel_decision(shard_len, missing_data, missing, true)
            .use_parallel
        {
            self.reconstruct_internal_option_vec_par_with_policy(
                shards,
                true,
                self.reconstruct_parallel_policy(true),
            )
        } else {
            self.reconstruct_data(shards)
        }
    }

    #[cfg(feature = "std")]
    pub fn reconstruct_some_opt(
        &self,
        shards: &mut [Option<Vec<u8>>],
        required: &[bool],
    ) -> Result<(), crate::Error> {
        if required.len() != self.total_shard_count() {
            return Err(crate::Error::InvalidShardFlags);
        }

        let data_only = required
            .iter()
            .enumerate()
            .all(|(idx, required)| !*required || idx < self.data_shard_count());

        if data_only {
            let mut number_present = 0;
            let mut shard_len = None;
            for shard in shards.iter() {
                if let Some(shard) = shard.as_ref() {
                    if shard.is_empty() {
                        return Err(crate::Error::EmptyShard);
                    }
                    number_present += 1;
                    if let Some(old_len) = shard_len {
                        if shard.len() != old_len {
                            return Err(crate::Error::IncorrectShardSize);
                        }
                    }
                    shard_len = Some(shard.len());
                }
            }

            if number_present == self.total_shard_count() {
                return Ok(());
            }
            if number_present < self.data_shard_count() {
                return Err(crate::Error::TooFewShardsPresent);
            }

            let shard_len = shard_len.expect("at least one shard present; qed");
            let mut valid_indices = smallvec::SmallVec::<[usize; 32]>::with_capacity(self.data_shard_count());
            let mut invalid_indices = smallvec::SmallVec::<[usize; 32]>::with_capacity(self.total_shard_count());

            for (idx, shard) in shards.iter().enumerate() {
                if shard.is_some() {
                    if valid_indices.len() < self.data_shard_count() {
                        valid_indices.push(idx);
                    }
                } else {
                    invalid_indices.push(idx);
                }
            }

            let data_decode_matrix = self.get_data_decode_matrix(&valid_indices, &invalid_indices);
            let sub_shards_snapshot: Vec<Vec<u8>> = valid_indices
                .iter()
                .map(|&idx| {
                    shards[idx]
                        .as_ref()
                        .expect("valid shard index must be present")
                        .clone()
                })
                .collect();
            let sub_shards: smallvec::SmallVec<[&[u8]; 32]> = sub_shards_snapshot
                .iter()
                .map(|shard| shard.as_slice())
                .collect();
            let use_parallel = self.parallel_policy(shard_len, 1).use_parallel;

            for i in 0..self.data_shard_count() {
                if !required[i] || shards[i].is_some() {
                    continue;
                }

                let mut recovered = vec![0u8; shard_len];
                let matrix_rows = [data_decode_matrix.get_row(i)];
                let mut outputs = [&mut recovered[..]];
                if use_parallel {
                    self.code_some_slices_par_raw(&matrix_rows, &sub_shards, &mut outputs);
                } else {
                    self.code_some_slices_chunked(&matrix_rows, &sub_shards, &mut outputs);
                }
                shards[i] = Some(recovered);
            }

            return Ok(());
        }

        self.reconstruct_opt(shards)?;
        Ok(())
    }
}

/// Add two elements.
pub fn add(a: u8, b: u8) -> u8 {
    a ^ b
}

/// Subtract `b` from `a`.
#[cfg(test)]
pub fn sub(a: u8, b: u8) -> u8 {
    a ^ b
}

/// Multiply two elements.
pub fn mul(a: u8, b: u8) -> u8 {
    MUL_TABLE[a as usize][b as usize]
}

/// Divide one element by another. `b`, the divisor, may not be 0.
pub fn div(a: u8, b: u8) -> u8 {
    if a == 0 {
        0
    } else if b == 0 {
        panic!("Divisor is 0")
    } else {
        let log_a = LOG_TABLE[a as usize];
        let log_b = LOG_TABLE[b as usize];
        let mut log_result = log_a as isize - log_b as isize;
        if log_result < 0 {
            log_result += 255;
        }
        EXP_TABLE[log_result as usize]
    }
}

/// Compute a^n.
pub fn exp(a: u8, n: usize) -> u8 {
    if n == 0 {
        1
    } else if a == 0 {
        0
    } else {
        let log_a = LOG_TABLE[a as usize];
        let mut log_result = log_a as usize * n;
        while 255 <= log_result {
            log_result -= 255;
        }
        EXP_TABLE[log_result]
    }
}

const PURE_RUST_UNROLL: isize = 4;

macro_rules! return_if_empty {
    (
        $len:expr
    ) => {
        if $len == 0 {
            return;
        }
    };
}

pub fn mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    (backend::active_backend().mul_slice)(c, input, out);
}

pub fn mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    (backend::active_backend().mul_slice_xor)(c, input, out);
}

pub fn active_backend_name() -> &'static str {
    backend::active_backend().name
}

pub fn active_backend_kind() -> BackendKind {
    backend::active_backend().kind
}

#[cfg(test)]
fn mul_slice_scalar_for_test(c: u8, input: &[u8], out: &mut [u8]) {
    mul_slice_pure_rust(c, input, out);
}

#[cfg(test)]
fn mul_slice_xor_scalar_for_test(c: u8, input: &[u8], out: &mut [u8]) {
    mul_slice_xor_pure_rust(c, input, out);
}

fn mul_slice_pure_rust(c: u8, input: &[u8], out: &mut [u8]) {
    let mt = &MUL_TABLE[c as usize];
    let mt_ptr: *const u8 = &mt[0];

    assert_eq!(input.len(), out.len());

    let len: isize = input.len() as isize;
    return_if_empty!(len);

    let mut input_ptr: *const u8 = &input[0];
    let mut out_ptr: *mut u8 = &mut out[0];

    let mut n: isize = 0;
    unsafe {
        assert_eq!(4, PURE_RUST_UNROLL);
        if len > PURE_RUST_UNROLL {
            let len_minus_unroll = len - PURE_RUST_UNROLL;
            while n < len_minus_unroll {
                *out_ptr = *mt_ptr.offset(*input_ptr as isize);
                *out_ptr.offset(1) = *mt_ptr.offset(*input_ptr.offset(1) as isize);
                *out_ptr.offset(2) = *mt_ptr.offset(*input_ptr.offset(2) as isize);
                *out_ptr.offset(3) = *mt_ptr.offset(*input_ptr.offset(3) as isize);

                input_ptr = input_ptr.offset(PURE_RUST_UNROLL);
                out_ptr = out_ptr.offset(PURE_RUST_UNROLL);
                n += PURE_RUST_UNROLL;
            }
        }
        while n < len {
            *out_ptr = *mt_ptr.offset(*input_ptr as isize);

            input_ptr = input_ptr.offset(1);
            out_ptr = out_ptr.offset(1);
            n += 1;
        }
    }
    /* for n in 0..input.len() {
     *   out[n] = mt[input[n] as usize]
     * }
     */
}

fn mul_slice_xor_pure_rust(c: u8, input: &[u8], out: &mut [u8]) {
    let mt = &MUL_TABLE[c as usize];
    let mt_ptr: *const u8 = &mt[0];

    assert_eq!(input.len(), out.len());

    let len: isize = input.len() as isize;
    return_if_empty!(len);

    let mut input_ptr: *const u8 = &input[0];
    let mut out_ptr: *mut u8 = &mut out[0];

    let mut n: isize = 0;
    unsafe {
        assert_eq!(4, PURE_RUST_UNROLL);
        if len > PURE_RUST_UNROLL {
            let len_minus_unroll = len - PURE_RUST_UNROLL;
            while n < len_minus_unroll {
                *out_ptr ^= *mt_ptr.offset(*input_ptr as isize);
                *out_ptr.offset(1) ^= *mt_ptr.offset(*input_ptr.offset(1) as isize);
                *out_ptr.offset(2) ^= *mt_ptr.offset(*input_ptr.offset(2) as isize);
                *out_ptr.offset(3) ^= *mt_ptr.offset(*input_ptr.offset(3) as isize);

                input_ptr = input_ptr.offset(PURE_RUST_UNROLL);
                out_ptr = out_ptr.offset(PURE_RUST_UNROLL);
                n += PURE_RUST_UNROLL;
            }
        }
        while n < len {
            *out_ptr ^= *mt_ptr.offset(*input_ptr as isize);

            input_ptr = input_ptr.offset(1);
            out_ptr = out_ptr.offset(1);
            n += 1;
        }
    }
    /* for n in 0..input.len() {
     *   out[n] ^= mt[input[n] as usize];
     * }
     */
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn rust_neon_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_neon_mul_slice_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn rust_neon_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_neon_mul_slice_xor_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "neon")]
unsafe fn rust_neon_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::aarch64::{
        uint8x16_t, uint8x16x4_t, vandq_u8, veorq_u8, vdupq_n_u8, vld1q_u8, vld1q_u8_x4,
        vqtbl1q_u8, vshrq_n_u8, vst1q_u8, vst1q_u8_x4,
    };

    let low_tbl = unsafe { vld1q_u8(MUL_TABLE_LOW[c as usize].as_ptr()) };
    let high_tbl = unsafe { vld1q_u8(MUL_TABLE_HIGH[c as usize].as_ptr()) };
    let nibble_mask = vdupq_n_u8(0x0f);
    let bytes_done = input.len() & !15usize;
    let bytes_done_unrolled = input.len() & !63usize;
    #[cfg(feature = "std")]
    {
        let vector_64b_chunks = bytes_done_unrolled / 64;
        let vector_16b_chunks = (bytes_done - bytes_done_unrolled) / 16;
        let tail_bytes = input.len() - bytes_done;
        RUST_NEON_PROFILE_METRICS.record_call(
            false,
            input.len(),
            vector_64b_chunks,
            vector_16b_chunks,
            tail_bytes,
        );
    }

    let mut offset = 0usize;
    while offset < bytes_done_unrolled {
        let inputs: uint8x16x4_t = unsafe { vld1q_u8_x4(input.as_ptr().add(offset)) };
        let input0 = inputs.0;
        let input1 = inputs.1;
        let input2 = inputs.2;
        let input3 = inputs.3;

        let low0 = vandq_u8(input0, nibble_mask);
        let low1 = vandq_u8(input1, nibble_mask);
        let low2 = vandq_u8(input2, nibble_mask);
        let low3 = vandq_u8(input3, nibble_mask);

        let high0 = vshrq_n_u8::<4>(input0);
        let high1 = vshrq_n_u8::<4>(input1);
        let high2 = vshrq_n_u8::<4>(input2);
        let high3 = vshrq_n_u8::<4>(input3);

        let result0: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
        let result1: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));
        let result2: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low2), vqtbl1q_u8(high_tbl, high2));
        let result3: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low3), vqtbl1q_u8(high_tbl, high3));

        unsafe {
            vst1q_u8_x4(
                out.as_mut_ptr().add(offset),
                uint8x16x4_t(result0, result1, result2, result3),
            )
        };
        offset += 64;
    }

    while offset < bytes_done {
        let input_vec = unsafe { vld1q_u8(input.as_ptr().add(offset)) };
        let low = vandq_u8(input_vec, nibble_mask);
        let high = vshrq_n_u8::<4>(input_vec);
        let result: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low), vqtbl1q_u8(high_tbl, high));
        unsafe { vst1q_u8(out.as_mut_ptr().add(offset), result) };
        offset += 16;
    }

    mul_slice_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "neon")]
unsafe fn rust_neon_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::aarch64::{
        uint8x16_t, uint8x16x2_t, uint8x16x4_t, vandq_u8, veorq_u8, vdupq_n_u8, vld1q_u8,
        vld1q_u8_x2, vld1q_u8_x4, vqtbl1q_u8, vshrq_n_u8, vst1q_u8, vst1q_u8_x2, vst1q_u8_x4,
    };

    let low_tbl = unsafe { vld1q_u8(MUL_TABLE_LOW[c as usize].as_ptr()) };
    let high_tbl = unsafe { vld1q_u8(MUL_TABLE_HIGH[c as usize].as_ptr()) };
    let nibble_mask = vdupq_n_u8(0x0f);
    let unroll4 = {
        #[cfg(feature = "std")]
        {
            rust_neon_mul_slice_xor_unroll() != 2
        }
        #[cfg(not(feature = "std"))]
        {
            true
        }
    };
    let bytes_done = input.len() & !15usize;
    let bytes_done_unrolled = if unroll4 {
        input.len() & !63usize
    } else {
        input.len() & !31usize
    };
    #[cfg(feature = "std")]
    {
        let vector_64b_chunks = if unroll4 { bytes_done_unrolled / 64 } else { 0 };
        let vector_16b_chunks = if unroll4 {
            (bytes_done - bytes_done_unrolled) / 16
        } else {
            ((bytes_done_unrolled / 32) * 2) + ((bytes_done - bytes_done_unrolled) / 16)
        };
        let tail_bytes = input.len() - bytes_done;
        RUST_NEON_PROFILE_METRICS.record_call(
            true,
            input.len(),
            vector_64b_chunks,
            vector_16b_chunks,
            tail_bytes,
        );
    }

    let mut offset = 0usize;
    if unroll4 {
        let schedule_split = {
            #[cfg(feature = "std")]
            {
                rust_neon_mul_slice_xor_schedule_split()
            }
            #[cfg(not(feature = "std"))]
            {
                false
            }
        };
        if schedule_split {
            while offset < bytes_done_unrolled {
                let inputs: uint8x16x4_t = unsafe { vld1q_u8_x4(input.as_ptr().add(offset)) };
                let input0 = inputs.0;
                let input1 = inputs.1;
                let input2 = inputs.2;
                let input3 = inputs.3;

                let low0 = vandq_u8(input0, nibble_mask);
                let low1 = vandq_u8(input1, nibble_mask);
                let low2 = vandq_u8(input2, nibble_mask);
                let low3 = vandq_u8(input3, nibble_mask);

                let high0 = vshrq_n_u8::<4>(input0);
                let high1 = vshrq_n_u8::<4>(input1);
                let high2 = vshrq_n_u8::<4>(input2);
                let high3 = vshrq_n_u8::<4>(input3);

                let product0: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
                let product1: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));
                let product2: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low2), vqtbl1q_u8(high_tbl, high2));
                let product3: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low3), vqtbl1q_u8(high_tbl, high3));
                let outs: uint8x16x4_t = unsafe { vld1q_u8_x4(out.as_ptr().add(offset)) };
                unsafe {
                    vst1q_u8_x4(
                        out.as_mut_ptr().add(offset),
                        uint8x16x4_t(
                            veorq_u8(outs.0, product0),
                            veorq_u8(outs.1, product1),
                            veorq_u8(outs.2, product2),
                            veorq_u8(outs.3, product3),
                        ),
                    )
                };
                offset += 64;
            }
        } else {
            while offset < bytes_done_unrolled {
                let inputs: uint8x16x4_t = unsafe { vld1q_u8_x4(input.as_ptr().add(offset)) };
                let input0 = inputs.0;
                let input1 = inputs.1;
                let input2 = inputs.2;
                let input3 = inputs.3;

                let low0 = vandq_u8(input0, nibble_mask);
                let low1 = vandq_u8(input1, nibble_mask);
                let low2 = vandq_u8(input2, nibble_mask);
                let low3 = vandq_u8(input3, nibble_mask);

                let high0 = vshrq_n_u8::<4>(input0);
                let high1 = vshrq_n_u8::<4>(input1);
                let high2 = vshrq_n_u8::<4>(input2);
                let high3 = vshrq_n_u8::<4>(input3);

                let product0: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
                let product1: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));
                let product2: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low2), vqtbl1q_u8(high_tbl, high2));
                let product3: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low3), vqtbl1q_u8(high_tbl, high3));
                let outs: uint8x16x4_t = unsafe { vld1q_u8_x4(out.as_ptr().add(offset)) };
                unsafe {
                    vst1q_u8_x4(
                        out.as_mut_ptr().add(offset),
                        uint8x16x4_t(
                            veorq_u8(outs.0, product0),
                            veorq_u8(outs.1, product1),
                            veorq_u8(outs.2, product2),
                            veorq_u8(outs.3, product3),
                        ),
                    )
                };
                offset += 64;
            }
        }
    } else {
        while offset < bytes_done_unrolled {
            let inputs: uint8x16x2_t = unsafe { vld1q_u8_x2(input.as_ptr().add(offset)) };
            let input0 = inputs.0;
            let input1 = inputs.1;

            let low0 = vandq_u8(input0, nibble_mask);
            let low1 = vandq_u8(input1, nibble_mask);

            let high0 = vshrq_n_u8::<4>(input0);
            let high1 = vshrq_n_u8::<4>(input1);

            let product0: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
            let product1: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));

            let outs: uint8x16x2_t = unsafe { vld1q_u8_x2(out.as_ptr().add(offset)) };
            unsafe {
                vst1q_u8_x2(
                    out.as_mut_ptr().add(offset),
                    uint8x16x2_t(
                        veorq_u8(outs.0, product0),
                        veorq_u8(outs.1, product1),
                    ),
                )
            };
            offset += 32;
        }
    }

    while offset < bytes_done {
        let input_vec = unsafe { vld1q_u8(input.as_ptr().add(offset)) };
        let low = vandq_u8(input_vec, nibble_mask);
        let high = vshrq_n_u8::<4>(input_vec);
        let product: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low), vqtbl1q_u8(high_tbl, high));
        let out_vec = unsafe { vld1q_u8(out.as_ptr().add(offset)) };
        unsafe { vst1q_u8(out.as_mut_ptr().add(offset), veorq_u8(out_vec, product)) };
        offset += 16;
    }

    mul_slice_xor_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn rust_avx2_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_avx2_mul_slice_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn rust_avx2_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_avx2_mul_slice_xor_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "avx2")]
unsafe fn rust_avx2_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_loadu_si128, _mm256_and_si256, _mm256_broadcastsi128_si256,
        _mm256_loadu_si256, _mm256_shuffle_epi8, _mm256_srli_epi64, _mm256_storeu_si256,
        _mm256_xor_si256, _mm256_set1_epi8,
    };

    let low128: __m128i = unsafe { _mm_loadu_si128(MUL_TABLE_LOW[c as usize].as_ptr().cast()) };
    let high128: __m128i = unsafe { _mm_loadu_si128(MUL_TABLE_HIGH[c as usize].as_ptr().cast()) };
    let low_tbl: __m256i = _mm256_broadcastsi128_si256(low128);
    let high_tbl: __m256i = _mm256_broadcastsi128_si256(high128);
    let nibble_mask: __m256i = _mm256_set1_epi8(0x0f);

    let bytes_done = input.len() & !31usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm256_loadu_si256(input.as_ptr().add(offset).cast()) };
        let low = _mm256_and_si256(input_vec, nibble_mask);
        let high = _mm256_and_si256(_mm256_srli_epi64::<4>(input_vec), nibble_mask);
        let result = _mm256_xor_si256(_mm256_shuffle_epi8(low_tbl, low), _mm256_shuffle_epi8(high_tbl, high));
        unsafe { _mm256_storeu_si256(out.as_mut_ptr().add(offset).cast(), result) };
        offset += 32;
    }

    mul_slice_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "avx2")]
unsafe fn rust_avx2_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_loadu_si128, _mm256_and_si256, _mm256_broadcastsi128_si256,
        _mm256_loadu_si256, _mm256_shuffle_epi8, _mm256_srli_epi64, _mm256_storeu_si256,
        _mm256_xor_si256, _mm256_set1_epi8,
    };

    let low128: __m128i = unsafe { _mm_loadu_si128(MUL_TABLE_LOW[c as usize].as_ptr().cast()) };
    let high128: __m128i = unsafe { _mm_loadu_si128(MUL_TABLE_HIGH[c as usize].as_ptr().cast()) };
    let low_tbl: __m256i = _mm256_broadcastsi128_si256(low128);
    let high_tbl: __m256i = _mm256_broadcastsi128_si256(high128);
    let nibble_mask: __m256i = _mm256_set1_epi8(0x0f);

    let bytes_done = input.len() & !31usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm256_loadu_si256(input.as_ptr().add(offset).cast()) };
        let low = _mm256_and_si256(input_vec, nibble_mask);
        let high = _mm256_and_si256(_mm256_srli_epi64::<4>(input_vec), nibble_mask);
        let product = _mm256_xor_si256(_mm256_shuffle_epi8(low_tbl, low), _mm256_shuffle_epi8(high_tbl, high));
        let out_vec = unsafe { _mm256_loadu_si256(out.as_ptr().add(offset).cast()) };
        unsafe { _mm256_storeu_si256(out.as_mut_ptr().add(offset).cast(), _mm256_xor_si256(out_vec, product)) };
        offset += 32;
    }

    mul_slice_xor_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(test)]
fn slice_xor(input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());

    let len: isize = input.len() as isize;
    return_if_empty!(len);

    let mut input_ptr: *const u8 = &input[0];
    let mut out_ptr: *mut u8 = &mut out[0];

    let mut n: isize = 0;
    unsafe {
        assert_eq!(4, PURE_RUST_UNROLL);
        if len > PURE_RUST_UNROLL {
            let len_minus_unroll = len - PURE_RUST_UNROLL;
            while n < len_minus_unroll {
                *out_ptr ^= *input_ptr;
                *out_ptr.offset(1) ^= *input_ptr.offset(1);
                *out_ptr.offset(2) ^= *input_ptr.offset(2);
                *out_ptr.offset(3) ^= *input_ptr.offset(3);

                input_ptr = input_ptr.offset(PURE_RUST_UNROLL);
                out_ptr = out_ptr.offset(PURE_RUST_UNROLL);
                n += PURE_RUST_UNROLL;
            }
        }
        while n < len {
            *out_ptr ^= *input_ptr;

            input_ptr = input_ptr.offset(1);
            out_ptr = out_ptr.offset(1);
            n += 1;
        }
    }
    /* for n in 0..input.len() {
     *   out[n] ^= input[n]
     * }
     */
}

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
unsafe extern "C" {
    fn reedsolomon_gal_mul(
        low: *const u8,
        high: *const u8,
        input: *const u8,
        out: *mut u8,
        len: libc::size_t,
    ) -> libc::size_t;

    fn reedsolomon_gal_mul_xor(
        low: *const u8,
        high: *const u8,
        input: *const u8,
        out: *mut u8,
        len: libc::size_t,
    ) -> libc::size_t;
}

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn simd_c_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    let low: *const u8 = &MUL_TABLE_LOW[c as usize][0];
    let high: *const u8 = &MUL_TABLE_HIGH[c as usize][0];

    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    let input_ptr: *const u8 = &input[0];
    let out_ptr: *mut u8 = &mut out[0];
    let size: libc::size_t = input.len();

    let bytes_done: usize =
        unsafe { reedsolomon_gal_mul(low, high, input_ptr, out_ptr, size) as usize };

    mul_slice_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn simd_c_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    let low: *const u8 = &MUL_TABLE_LOW[c as usize][0];
    let high: *const u8 = &MUL_TABLE_HIGH[c as usize][0];

    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    let input_ptr: *const u8 = &input[0];
    let out_ptr: *mut u8 = &mut out[0];
    let size: libc::size_t = input.len();

    let bytes_done: usize =
        unsafe { reedsolomon_gal_mul_xor(low, high, input_ptr, out_ptr, size) as usize };

    mul_slice_xor_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec;

    use super::*;
    use crate::tests::fill_random;
    use rand;

    static BACKBLAZE_LOG_TABLE: [u8; 256] = [
        //-1,    0,    1,   25,    2,   50,   26,  198,
        // first value is changed from -1 to 0
        0, 0, 1, 25, 2, 50, 26, 198, 3, 223, 51, 238, 27, 104, 199, 75, 4, 100, 224, 14, 52, 141,
        239, 129, 28, 193, 105, 248, 200, 8, 76, 113, 5, 138, 101, 47, 225, 36, 15, 33, 53, 147,
        142, 218, 240, 18, 130, 69, 29, 181, 194, 125, 106, 39, 249, 185, 201, 154, 9, 120, 77,
        228, 114, 166, 6, 191, 139, 98, 102, 221, 48, 253, 226, 152, 37, 179, 16, 145, 34, 136, 54,
        208, 148, 206, 143, 150, 219, 189, 241, 210, 19, 92, 131, 56, 70, 64, 30, 66, 182, 163,
        195, 72, 126, 110, 107, 58, 40, 84, 250, 133, 186, 61, 202, 94, 155, 159, 10, 21, 121, 43,
        78, 212, 229, 172, 115, 243, 167, 87, 7, 112, 192, 247, 140, 128, 99, 13, 103, 74, 222,
        237, 49, 197, 254, 24, 227, 165, 153, 119, 38, 184, 180, 124, 17, 68, 146, 217, 35, 32,
        137, 46, 55, 63, 209, 91, 149, 188, 207, 205, 144, 135, 151, 178, 220, 252, 190, 97, 242,
        86, 211, 171, 20, 42, 93, 158, 132, 60, 57, 83, 71, 109, 65, 162, 31, 45, 67, 216, 183,
        123, 164, 118, 196, 23, 73, 236, 127, 12, 111, 246, 108, 161, 59, 82, 41, 157, 85, 170,
        251, 96, 134, 177, 187, 204, 62, 90, 203, 89, 95, 176, 156, 169, 160, 81, 11, 245, 22, 235,
        122, 117, 44, 215, 79, 174, 213, 233, 230, 231, 173, 232, 116, 214, 244, 234, 168, 80, 88,
        175,
    ];

    #[test]
    fn log_table_same_as_backblaze() {
        for i in 0..256 {
            assert_eq!(LOG_TABLE[i], BACKBLAZE_LOG_TABLE[i]);
        }
    }

    #[test]
    fn test_associativity() {
        for a in 0..256 {
            let a = a as u8;
            for b in 0..256 {
                let b = b as u8;
                for c in 0..256 {
                    let c = c as u8;
                    let x = add(a, add(b, c));
                    let y = add(add(a, b), c);
                    assert_eq!(x, y);
                    let x = mul(a, mul(b, c));
                    let y = mul(mul(a, b), c);
                    assert_eq!(x, y);
                }
            }
        }
    }

    quickcheck! {
        fn qc_add_associativity(a: u8, b: u8, c: u8) -> bool {
            add(a, add(b, c)) == add(add(a, b), c)
        }

        fn qc_mul_associativity(a: u8, b: u8, c: u8) -> bool {
            mul(a, mul(b, c)) == mul(mul(a, b), c)
        }
    }

    #[test]
    fn test_identity() {
        for a in 0..256 {
            let a = a as u8;
            let b = sub(0, a);
            let c = sub(a, b);
            assert_eq!(c, 0);
            if a != 0 {
                let b = div(1, a);
                let c = mul(a, b);
                assert_eq!(c, 1);
            }
        }
    }

    quickcheck! {
        fn qc_additive_identity(a: u8) -> bool {
            sub(a, sub(0, a)) == 0
        }

        fn qc_multiplicative_identity(a: u8) -> bool {
            if a == 0 { true }
            else      { mul(a, div(1, a)) == 1 }
        }
    }

    #[test]
    fn test_commutativity() {
        for a in 0..256 {
            let a = a as u8;
            for b in 0..256 {
                let b = b as u8;
                let x = add(a, b);
                let y = add(b, a);
                assert_eq!(x, y);
                let x = mul(a, b);
                let y = mul(b, a);
                assert_eq!(x, y);
            }
        }
    }

    quickcheck! {
        fn qc_add_commutativity(a: u8, b: u8) -> bool {
            add(a, b) == add(b, a)
        }

        fn qc_mul_commutativity(a: u8, b: u8) -> bool {
            mul(a, b) == mul(b, a)
        }
    }

    #[test]
    fn test_distributivity() {
        for a in 0..256 {
            let a = a as u8;
            for b in 0..256 {
                let b = b as u8;
                for c in 0..256 {
                    let c = c as u8;
                    let x = mul(a, add(b, c));
                    let y = add(mul(a, b), mul(a, c));
                    assert_eq!(x, y);
                }
            }
        }
    }

    quickcheck! {
        fn qc_add_distributivity(a: u8, b: u8, c: u8) -> bool {
            mul(a, add(b, c)) == add(mul(a, b), mul(a, c))
        }
    }

    #[test]
    fn test_exp() {
        for a in 0..256 {
            let a = a as u8;
            let mut power = 1u8;
            for j in 0..256 {
                let x = exp(a, j);
                assert_eq!(x, power);
                power = mul(power, a);
            }
        }
    }

    #[test]
    fn test_galois() {
        assert_eq!(mul(3, 4), 12);
        assert_eq!(mul(7, 7), 21);
        assert_eq!(mul(23, 45), 41);

        let input = [
            0, 1, 2, 3, 4, 5, 6, 10, 50, 100, 150, 174, 201, 255, 99, 32, 67, 85, 200, 199, 198,
            197, 196, 195, 194, 193, 192, 191, 190, 189, 188, 187, 186, 185,
        ];
        let mut output1 = vec![0; input.len()];
        let mut output2 = vec![0; input.len()];
        mul_slice(25, &input, &mut output1);
        let expect = [
            0x0, 0x19, 0x32, 0x2b, 0x64, 0x7d, 0x56, 0xfa, 0xb8, 0x6d, 0xc7, 0x85, 0xc3, 0x1f,
            0x22, 0x7, 0x25, 0xfe, 0xda, 0x5d, 0x44, 0x6f, 0x76, 0x39, 0x20, 0xb, 0x12, 0x11, 0x8,
            0x23, 0x3a, 0x75, 0x6c, 0x47,
        ];
        for i in 0..input.len() {
            assert_eq!(expect[i], output1[i]);
        }
        mul_slice(25, &input, &mut output2);
        for i in 0..input.len() {
            assert_eq!(expect[i], output2[i]);
        }

        let expect_xor = [
            0x0, 0x2d, 0x5a, 0x77, 0xb4, 0x99, 0xee, 0x2f, 0x79, 0xf2, 0x7, 0x51, 0xd4, 0x19, 0x31,
            0xc9, 0xf8, 0xfc, 0xf9, 0x4f, 0x62, 0x15, 0x38, 0xfb, 0xd6, 0xa1, 0x8c, 0x96, 0xbb,
            0xcc, 0xe1, 0x22, 0xf, 0x78,
        ];
        mul_slice_xor(52, &input, &mut output1);
        for i in 0..input.len() {
            assert_eq!(expect_xor[i], output1[i]);
        }
        mul_slice_xor(52, &input, &mut output2);
        for i in 0..input.len() {
            assert_eq!(expect_xor[i], output2[i]);
        }

        let expect = [
            0x0, 0xb1, 0x7f, 0xce, 0xfe, 0x4f, 0x81, 0x9e, 0x3, 0x6, 0xe8, 0x75, 0xbd, 0x40, 0x36,
            0xa3, 0x95, 0xcb, 0xc, 0xdd, 0x6c, 0xa2, 0x13, 0x23, 0x92, 0x5c, 0xed, 0x1b, 0xaa,
            0x64, 0xd5, 0xe5, 0x54, 0x9a,
        ];
        mul_slice(177, &input, &mut output1);
        for i in 0..input.len() {
            assert_eq!(expect[i], output1[i]);
        }
        mul_slice(177, &input, &mut output2);
        for i in 0..input.len() {
            assert_eq!(expect[i], output2[i]);
        }

        let expect_xor = [
            0x0, 0xc4, 0x95, 0x51, 0x37, 0xf3, 0xa2, 0xfb, 0xec, 0xc5, 0xd0, 0xc7, 0x53, 0x88,
            0xa3, 0xa5, 0x6, 0x78, 0x97, 0x9f, 0x5b, 0xa, 0xce, 0xa8, 0x6c, 0x3d, 0xf9, 0xdf, 0x1b,
            0x4a, 0x8e, 0xe8, 0x2c, 0x7d,
        ];
        mul_slice_xor(117, &input, &mut output1);
        for i in 0..input.len() {
            assert_eq!(expect_xor[i], output1[i]);
        }
        mul_slice_xor(117, &input, &mut output2);
        for i in 0..input.len() {
            assert_eq!(expect_xor[i], output2[i]);
        }

        assert_eq!(exp(2, 2), 4);
        assert_eq!(exp(5, 20), 235);
        assert_eq!(exp(13, 7), 43);
    }

    #[test]
    fn test_slice_add() {
        let length_list = [16, 32, 34];
        for len in length_list.iter() {
            let mut input = vec![0; *len];
            fill_random(&mut input);
            let mut output = vec![0; *len];
            fill_random(&mut output);
            let mut expect = vec![0; *len];
            for i in 0..expect.len() {
                expect[i] = input[i] ^ output[i];
            }
            slice_xor(&input, &mut output);
            for i in 0..expect.len() {
                assert_eq!(expect[i], output[i]);
            }
            fill_random(&mut output);
            for i in 0..expect.len() {
                expect[i] = input[i] ^ output[i];
            }
            slice_xor(&input, &mut output);
            for i in 0..expect.len() {
                assert_eq!(expect[i], output[i]);
            }
        }
    }

    #[test]
    fn test_div_a_is_0() {
        assert_eq!(0, div(0, 100));
    }

    #[test]
    #[should_panic]
    fn test_div_b_is_0() {
        div(1, 0);
    }

    #[test]
    fn test_same_as_maybe_ffi() {
        let len = 10_003;
        for _ in 0..100 {
            let c = rand::random::<u8>();
            let mut input = vec![0; len];
            fill_random(&mut input);
            {
                let mut output = vec![0; len];
                fill_random(&mut output);
                let mut output_copy = output.clone();

                mul_slice(c, &input, &mut output);
                mul_slice(c, &input, &mut output_copy);

                assert_eq!(output, output_copy);
            }
            {
                let mut output = vec![0; len];
                fill_random(&mut output);
                let mut output_copy = output.clone();

                mul_slice_xor(c, &input, &mut output);
                mul_slice_xor(c, &input, &mut output_copy);

                assert_eq!(output, output_copy);
            }
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        any(target_arch = "x86_64", target_arch = "aarch64"),
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_simd_c_matches_scalar_mul_slice() {
        let lengths = [0usize, 1, 15, 16, 17, 31, 32, 33, 255, 1024, 10_003];
        for &len in &lengths {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut simd = vec![0; len];

                mul_slice_scalar_for_test(c, &input, &mut scalar);
                simd_c_mul_slice(c, &input, &mut simd);

                assert_eq!(scalar, simd);
            }
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        any(target_arch = "x86_64", target_arch = "aarch64"),
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_simd_c_matches_scalar_mul_slice_xor() {
        let lengths = [0usize, 1, 15, 16, 17, 31, 32, 33, 255, 1024, 10_003];
        for &len in &lengths {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut simd = vec![0; len];
                fill_random(&mut scalar);
                simd.copy_from_slice(&scalar);

                mul_slice_xor_scalar_for_test(c, &input, &mut scalar);
                simd_c_mul_slice_xor(c, &input, &mut simd);

                assert_eq!(scalar, simd);
            }
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_rust_neon_matches_scalar_mul_slice() {
        let lengths = [0usize, 1, 15, 16, 17, 31, 32, 33, 255, 1024, 10_003];
        for &len in &lengths {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut neon = vec![0; len];

                mul_slice_scalar_for_test(c, &input, &mut scalar);
                rust_neon_mul_slice(c, &input, &mut neon);

                assert_eq!(scalar, neon);
            }
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_rust_neon_matches_scalar_mul_slice_xor() {
        let lengths = [0usize, 1, 15, 16, 17, 31, 32, 33, 255, 1024, 10_003];
        for &len in &lengths {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut neon = vec![0; len];
                fill_random(&mut scalar);
                neon.copy_from_slice(&scalar);

                mul_slice_xor_scalar_for_test(c, &input, &mut scalar);
                rust_neon_mul_slice_xor(c, &input, &mut neon);

                assert_eq!(scalar, neon);
            }
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_rust_neon_matches_simd_c() {
        let lengths = [0usize, 1, 15, 16, 17, 31, 32, 33, 255, 1024, 10_003];
        for &len in &lengths {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut simd_c = vec![0; len];
                let mut neon = vec![0; len];

                simd_c_mul_slice(c, &input, &mut simd_c);
                rust_neon_mul_slice(c, &input, &mut neon);

                assert_eq!(simd_c, neon);
            }
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_rust_neon_profile_stats_track_vector_vs_tail() {
        reset_rust_neon_profile_stats();

        let c = 25u8;
        let mut input = vec![0u8; 65];
        fill_random(&mut input);
        let mut out = vec![0u8; 65];
        let mut out_xor = vec![0u8; 65];

        let before = rust_neon_profile_stats();
        rust_neon_mul_slice(c, &input, &mut out);
        rust_neon_mul_slice_xor(c, &input, &mut out_xor);
        let delta = rust_neon_profile_stats().saturating_sub(before);

        assert_eq!(1, delta.mul_calls);
        assert_eq!(1, delta.mul_xor_calls);
        assert_eq!(130, delta.total_bytes);
        assert_eq!(2, delta.vector_64b_chunks);
        assert_eq!(0, delta.vector_16b_chunks);
        assert_eq!(2, delta.tail_bytes);
        assert_eq!(2, delta.tail_calls);
        assert_eq!(16, delta.table_lookups);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_parse_rust_neon_xor_unroll() {
        assert_eq!(Some(2), parse_rust_neon_xor_unroll("2"));
        assert_eq!(Some(4), parse_rust_neon_xor_unroll("4"));
        assert_eq!(None, parse_rust_neon_xor_unroll("1"));
        assert_eq!(None, parse_rust_neon_xor_unroll("8"));
        assert_eq!(None, parse_rust_neon_xor_unroll("abc"));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_rust_neon_xor_schedule_env_constant() {
        assert_eq!(
            "RS_NEON_MUL_SLICE_XOR_SCHEDULE",
            RS_NEON_MUL_SLICE_XOR_SCHEDULE_ENV
        );
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_rust_avx2_matches_scalar_mul_slice() {
        let lengths = [0usize, 1, 31, 32, 33, 255, 1024, 10_003];
        for &len in &lengths {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut avx2 = vec![0; len];

                mul_slice_scalar_for_test(c, &input, &mut scalar);
                rust_avx2_mul_slice(c, &input, &mut avx2);

                assert_eq!(scalar, avx2);
            }
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_rust_avx2_matches_scalar_mul_slice_xor() {
        let lengths = [0usize, 1, 31, 32, 33, 255, 1024, 10_003];
        for &len in &lengths {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut avx2 = vec![0; len];
                fill_random(&mut scalar);
                avx2.copy_from_slice(&scalar);

                mul_slice_xor_scalar_for_test(c, &input, &mut scalar);
                rust_avx2_mul_slice_xor(c, &input, &mut avx2);

                assert_eq!(scalar, avx2);
            }
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_rust_avx2_matches_simd_c() {
        let lengths = [0usize, 1, 31, 32, 33, 255, 1024, 10_003];
        for &len in &lengths {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut simd_c = vec![0; len];
                let mut avx2 = vec![0; len];

                simd_c_mul_slice(c, &input, &mut simd_c);
                rust_avx2_mul_slice(c, &input, &mut avx2);

                assert_eq!(simd_c, avx2);
            }
        }
    }

    #[test]
    fn test_active_backend_metadata() {
        #[cfg(all(
            feature = "simd-accel",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            not(target_env = "msvc"),
            not(any(target_os = "android", target_os = "ios"))
        ))]
        {
            #[cfg(all(feature = "std", target_arch = "x86_64"))]
            {
                if cfg!(rse_simd_c_build_haswell) {
                    if std::is_x86_feature_detected!("avx2") {
                        assert_eq!(active_backend_name(), "simd-c");
                        assert_eq!(active_backend_kind(), BackendKind::SimdC);
                    } else {
                        assert_eq!(active_backend_name(), "scalar-rust");
                        assert_eq!(active_backend_kind(), BackendKind::Scalar);
                    }
                } else {
                    assert_eq!(active_backend_name(), "simd-c");
                    assert_eq!(active_backend_kind(), BackendKind::SimdC);
                }
            }

            #[cfg(all(feature = "std", target_arch = "aarch64"))]
            {
                assert_eq!(active_backend_name(), "rust-neon");
                assert_eq!(active_backend_kind(), BackendKind::RustSimd);
            }

            #[cfg(not(feature = "std"))]
            {
                assert_eq!(active_backend_name(), "scalar-rust");
                assert_eq!(active_backend_kind(), BackendKind::Scalar);
            }
        }

        #[cfg(not(all(
            feature = "simd-accel",
            any(target_arch = "x86_64", target_arch = "aarch64"),
            not(target_env = "msvc"),
            not(any(target_os = "android", target_os = "ios"))
        )))]
        {
            assert_eq!(active_backend_name(), "scalar-rust");
            assert_eq!(active_backend_kind(), BackendKind::Scalar);
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        feature = "std",
        any(target_arch = "x86_64", target_arch = "aarch64"),
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    ))]
    #[test]
    fn test_backend_override_affects_active_backend() {
        #[cfg(target_arch = "aarch64")]
        {
            unsafe { std::env::set_var("RSE_BACKEND_OVERRIDE", "rust-neon") };
            assert_eq!(super::backend::runtime_override_backend_name_for_test(), Some("rust-neon"));
            unsafe { std::env::remove_var("RSE_BACKEND_OVERRIDE") };
        }

        #[cfg(target_arch = "x86_64")]
        {
            unsafe { std::env::set_var("RSE_BACKEND_OVERRIDE", "rust-avx2") };
            assert_eq!(super::backend::runtime_override_backend_name_for_test(), Some("rust-avx2"));
            unsafe { std::env::remove_var("RSE_BACKEND_OVERRIDE") };
        }
    }
}
