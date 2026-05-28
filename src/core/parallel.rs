use crate::Field;

use super::{
    CODE_SLICE_DEFAULT_CHUNK_BYTES, CODE_SLICE_LARGE_CHUNK_BYTES, CODE_SLICE_MIN_CHUNK_BYTES,
    PARALLEL_MIN_SHARD_BYTES, ReedSolomon,
};

#[cfg(feature = "std")]
pub const PARALLEL_POLICY_VERSION: u32 = 1;
#[cfg(feature = "std")]
const RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES_ENV: &str =
    "RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES";
#[cfg(feature = "std")]
const RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB_ENV: &str = "RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB";
#[cfg(feature = "std")]
const RS_PARALLEL_POLICY_MAX_JOBS_ENV: &str = "RS_PARALLEL_POLICY_MAX_JOBS";

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
    pub(crate) fn new(data: ParallelPolicy) -> Self {
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

impl<F: Field> ReedSolomon<F> {
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
    pub(crate) fn resolve_policy_cache() -> RuntimeParallelPolicyCache {
        let data = ParallelPolicy::default().with_env_overrides();
        if core::any::type_name::<F>() == core::any::type_name::<crate::galois_8::Field>() {
            crate::galois_8::resolve_runtime_parallel_policy_cache(data)
        } else {
            RuntimeParallelPolicyCache::new(data)
        }
    }
}
