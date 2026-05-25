use super::{active_backend_id, BackendId};

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
#[cfg(all(feature = "std", target_arch = "aarch64"))]
const RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES_ENV: &str =
    "RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES";
#[cfg(all(feature = "std", target_arch = "aarch64"))]
const RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB_ENV: &str =
    "RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB";
#[cfg(all(feature = "std", target_arch = "aarch64"))]
const RS_AARCH64_RECONSTRUCT_MAX_JOBS_ENV: &str = "RS_AARCH64_RECONSTRUCT_MAX_JOBS";
#[cfg(all(feature = "std", target_arch = "aarch64"))]
const RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB_ENV: &str =
    "RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB";
#[cfg(all(feature = "std", target_arch = "aarch64"))]
const RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB_ENV: &str =
    "RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB";

impl crate::ReedSolomon<super::Field> {
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
        let tuned = self.reconstruct_parallel_policy(missing_data, missing_total, data_only);
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
    fn reconstruct_parallel_policy(
        &self,
        missing_data: usize,
        missing_total: usize,
        data_only: bool,
    ) -> crate::ParallelPolicy {
        #[cfg(target_arch = "aarch64")]
        {
            return self.reconstruct_parallel_policy_aarch64(
                missing_data,
                missing_total,
                data_only,
            );
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            return self.reconstruct_parallel_policy_default(
                missing_data,
                missing_total,
                data_only,
            );
        }
    }

    #[cfg(feature = "std")]
    fn reconstruct_parallel_policy_default(
        &self,
        _missing_data: usize,
        _missing_total: usize,
        data_only: bool,
    ) -> crate::ParallelPolicy {
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

    #[cfg(all(feature = "std", target_arch = "aarch64"))]
    fn reconstruct_parallel_policy_aarch64(
        &self,
        missing_data: usize,
        missing_total: usize,
        data_only: bool,
    ) -> crate::ParallelPolicy {
        let mut policy =
            self.reconstruct_parallel_policy_default(missing_data, missing_total, data_only);
        if let Some(value) =
            Self::read_env_usize(RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES_ENV)
        {
            if value > 0 {
                policy.min_parallel_shard_bytes = value;
            }
        }
        if let Some(value) = Self::read_env_usize(RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB_ENV) {
            if value > 0 {
                policy.min_bytes_per_job = value;
            }
        }
        if let Some(value) = Self::read_env_usize(RS_AARCH64_RECONSTRUCT_MAX_JOBS_ENV) {
            policy.max_jobs = value;
        }
        policy
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
    fn reconstruct_stage_policies(
        &self,
        missing_data: usize,
        missing_total: usize,
        data_only: bool,
    ) -> (crate::ParallelPolicy, crate::ParallelPolicy) {
        #[cfg(target_arch = "aarch64")]
        {
            return self.reconstruct_stage_policies_aarch64(
                missing_data,
                missing_total,
                data_only,
            );
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            let policy = self.reconstruct_parallel_policy_default(
                missing_data,
                missing_total,
                data_only,
            );
            return (policy, policy);
        }
    }

    #[cfg(test)]
    #[cfg(feature = "std")]
    pub(crate) fn reconstruct_stage_policies_for_test(
        &self,
        missing_data: usize,
        missing_total: usize,
        data_only: bool,
    ) -> (crate::ParallelPolicy, crate::ParallelPolicy) {
        self.reconstruct_stage_policies(missing_data, missing_total, data_only)
    }

    #[cfg(all(feature = "std", target_arch = "aarch64"))]
    fn reconstruct_stage_policies_aarch64(
        &self,
        missing_data: usize,
        missing_total: usize,
        data_only: bool,
    ) -> (crate::ParallelPolicy, crate::ParallelPolicy) {
        let mut data_policy =
            self.reconstruct_parallel_policy_aarch64(missing_data, missing_total, data_only);
        let mut parity_policy =
            self.reconstruct_parallel_policy_default(missing_data, missing_total, data_only);
        if let Some(value) =
            Self::read_env_usize(RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB_ENV)
        {
            if value > 0 {
                data_policy.min_bytes_per_job = value;
            }
        }
        if let Some(value) =
            Self::read_env_usize(RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB_ENV)
        {
            if value > 0 {
                parity_policy.min_bytes_per_job = value;
            }
        }
        (data_policy, parity_policy)
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
            let (data_policy, parity_policy) =
                self.reconstruct_stage_policies(missing_data, missing, false);
            self.reconstruct_internal_option_vec_par_with_stage_policies(
                shards,
                false,
                data_policy,
                parity_policy,
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
            let (data_policy, _parity_policy) =
                self.reconstruct_stage_policies(missing_data, missing, true);
            self.reconstruct_internal_option_vec_par_with_policy(
                shards,
                true,
                data_policy,
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
            let mut valid_indices =
                smallvec::SmallVec::<[usize; 32]>::with_capacity(self.data_shard_count());
            let mut invalid_indices =
                smallvec::SmallVec::<[usize; 32]>::with_capacity(self.total_shard_count());

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
