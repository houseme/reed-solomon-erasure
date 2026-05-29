#[cfg(feature = "std")]
use crate::core::RuntimeParallelPolicyCache;

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

#[cfg(feature = "std")]
struct OptionVecReconstructPlan {
    shard_len: usize,
    valid_indices: smallvec::SmallVec<[usize; 32]>,
    invalid_indices: smallvec::SmallVec<[usize; 32]>,
    number_present: usize,
    data_decode_matrix: Option<std::sync::Arc<crate::matrix::Matrix<super::Field>>>,
    required_missing_data_indices: smallvec::SmallVec<[usize; 32]>,
}

impl crate::ReedSolomon<super::Field> {
    #[cfg(feature = "std")]
    fn encode_leopard_gf8_opt<T, U>(&self, data: &[T], parity: &mut [U]) -> Result<(), crate::Error>
    where
        T: AsRef<[u8]> + Sync,
        U: AsRef<[u8]> + AsMut<[u8]> + Send,
    {
        crate::core::leopard_gf8::encode_with_tables(
            self.data_shard_count(),
            self.parity_shard_count(),
            data,
            parity,
        )
        .map(|_| ())
    }

    #[cfg(feature = "std")]
    fn decode_idx_execute_reduced_small_outputs(
        &self,
        matrix_rows: &[Vec<u8>],
        inputs: &[&[u8]],
        outputs: &mut [&mut [u8]],
    ) {
        debug_assert!(!outputs.is_empty());
        debug_assert!(outputs.len() <= 2);

        let shard_len = inputs.first().map(|input| input.len()).unwrap_or(0);
        if shard_len == 0 {
            return;
        }

        let chunk_len = self.code_chunk_len(shard_len);
        if outputs.len() == 1 {
            let matrix_row = matrix_rows[0].as_slice();
            outputs[0]
                .chunks_mut(chunk_len)
                .enumerate()
                .for_each(|(chunk_idx, output_chunk)| {
                    let start = chunk_idx * chunk_len;
                    let end = start + output_chunk.len();
                    super::mul_slice(matrix_row[0], &inputs[0][start..end], output_chunk);
                    for i_input in 1..inputs.len() {
                        super::mul_slice_xor(
                            matrix_row[i_input],
                            &inputs[i_input][start..end],
                            output_chunk,
                        );
                    }
                });
            return;
        }

        let (first, second) = outputs.split_at_mut(1);
        let output0 = &mut first[0];
        let output1 = &mut second[0];
        let row0 = matrix_rows[0].as_slice();
        let row1 = matrix_rows[1].as_slice();
        output0
            .chunks_mut(chunk_len)
            .zip(output1.chunks_mut(chunk_len))
            .enumerate()
            .for_each(|(chunk_idx, (output0_chunk, output1_chunk))| {
                let start = chunk_idx * chunk_len;
                let end = start + output0_chunk.len();
                let input0 = &inputs[0][start..end];
                super::mul_slice(row0[0], input0, output0_chunk);
                super::mul_slice(row1[0], input0, output1_chunk);
                for i_input in 1..inputs.len() {
                    let input_chunk = &inputs[i_input][start..end];
                    super::mul_slice_xor(row0[i_input], input_chunk, output0_chunk);
                    super::mul_slice_xor(row1[i_input], input_chunk, output1_chunk);
                }
            });
    }

    #[cfg(feature = "std")]
    fn execute_option_vec_required_data_plan(
        &self,
        shards: &mut [Option<Vec<u8>>],
        plan: OptionVecReconstructPlan,
    ) -> Result<(), crate::Error> {
        if plan.required_missing_data_indices.is_empty() {
            return Ok(());
        }

        let data_decode_matrix = plan
            .data_decode_matrix
            .as_ref()
            .expect("non-empty plan must include decode matrix");
        let mut matrix_rows: smallvec::SmallVec<[&[u8]; 32]> =
            smallvec::SmallVec::with_capacity(plan.required_missing_data_indices.len());
        for &idx in &plan.required_missing_data_indices {
            matrix_rows.push(data_decode_matrix.get_row(idx));
        }

        let mut recovered_data: Vec<Vec<u8>> = plan
            .required_missing_data_indices
            .iter()
            .map(|_| vec![0u8; plan.shard_len])
            .collect();
        {
            let sub_shards_snapshot: Vec<&[u8]> = plan
                .valid_indices
                .iter()
                .map(|&idx| {
                    shards[idx]
                        .as_deref()
                        .expect("valid shard index must be present")
                })
                .collect();
            let sub_shards: smallvec::SmallVec<[&[u8]; 32]> =
                sub_shards_snapshot.into_iter().collect();

            let mut outputs: smallvec::SmallVec<[&mut [u8]; 32]> = recovered_data
                .iter_mut()
                .map(|shard| shard.as_mut_slice())
                .collect();
            let use_parallel = self
                .parallel_policy(plan.shard_len, plan.required_missing_data_indices.len())
                .use_parallel;
            if use_parallel {
                self.code_some_slices_par_raw(&matrix_rows, &sub_shards, &mut outputs);
            } else {
                self.code_some_slices_chunked(&matrix_rows, &sub_shards, &mut outputs);
            }
        }

        for (idx, recovered) in plan
            .required_missing_data_indices
            .into_iter()
            .zip(recovered_data.into_iter())
        {
            shards[idx] = Some(recovered);
        }

        Ok(())
    }

    #[cfg(feature = "std")]
    fn plan_option_vec_reconstruct(
        &self,
        shards: &[Option<Vec<u8>>],
        required: Option<&[bool]>,
    ) -> Result<OptionVecReconstructPlan, crate::Error> {
        let mut number_present = 0;
        let mut shard_len = None;
        for shard in shards.iter() {
            if let Some(shard) = shard.as_ref() {
                if shard.is_empty() {
                    return Err(crate::Error::EmptyShard);
                }
                number_present += 1;
                if let Some(old_len) = shard_len
                    && shard.len() != old_len
                {
                    return Err(crate::Error::IncorrectShardSize);
                }
                shard_len = Some(shard.len());
            }
        }

        if number_present == self.total_shard_count() {
            return Ok(OptionVecReconstructPlan {
                shard_len: 0,
                valid_indices: smallvec::SmallVec::new(),
                invalid_indices: smallvec::SmallVec::new(),
                number_present,
                data_decode_matrix: None,
                required_missing_data_indices: smallvec::SmallVec::new(),
            });
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
        let required_missing_data_indices = required
            .map(|required| {
                (0..self.data_shard_count())
                    .filter(|&i| required[i] && shards[i].is_none())
                    .collect()
            })
            .unwrap_or_else(smallvec::SmallVec::new);

        Ok(OptionVecReconstructPlan {
            shard_len,
            valid_indices,
            invalid_indices,
            number_present,
            data_decode_matrix: Some(data_decode_matrix),
            required_missing_data_indices,
        })
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
        let tuned = self.policy_cache.reconstruct_policy(data_only);
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
        data_only: bool,
    ) -> (crate::ParallelPolicy, crate::ParallelPolicy) {
        self.policy_cache.reconstruct_stage_policies(data_only)
    }

    #[cfg(test)]
    #[cfg(feature = "std")]
    pub(crate) fn reconstruct_stage_policies_for_test(
        &self,
        data_only: bool,
    ) -> (crate::ParallelPolicy, crate::ParallelPolicy) {
        self.reconstruct_stage_policies(data_only)
    }
    #[cfg(feature = "std")]
    pub fn encode_opt<T, U>(&self, mut shards: T) -> Result<(), crate::Error>
    where
        T: AsRef<[U]> + AsMut<[U]>,
        U: AsRef<[u8]> + AsMut<[u8]> + Send + Sync,
    {
        if self.is_leopard_gf8_family() {
            let slices = shards.as_mut();
            if slices.len() != self.total_shard_count() {
                return Err(crate::Error::TooFewShards);
            }
            if slices.is_empty() {
                return Err(crate::Error::TooFewShards);
            }
            let shard_len = slices[0].as_ref().len();
            if shard_len == 0 {
                return Err(crate::Error::EmptyShard);
            }
            for shard in slices.iter() {
                if shard.as_ref().len() != shard_len {
                    return Err(crate::Error::IncorrectShardSize);
                }
            }
            let (data, parity) = slices.split_at_mut(self.data_shard_count());
            return self.encode_leopard_gf8_opt(&*data, parity);
        }

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
        if self.is_leopard_gf8_family() {
            return self.encode_leopard_gf8_opt(data, parity);
        }

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
        self.ensure_classic_family_execution()?;
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
        self.ensure_classic_family_execution()?;
        let shard_len = Self::first_shard_len(slices);
        if self.should_parallel_data_path(shard_len, buffer.len()) {
            self.verify_with_buffer_par(slices, buffer)
        } else {
            self.verify_with_buffer(slices, buffer)
        }
    }

    #[cfg(feature = "std")]
    pub fn verify_with_workspace_opt(
        &self,
        slices: &[Vec<u8>],
        workspace: &mut crate::VerifyWorkspace<crate::galois_8::Field>,
    ) -> Result<bool, crate::Error> {
        self.ensure_classic_family_execution()?;
        let shard_len = Self::first_shard_len(slices);
        workspace.resize(self, shard_len);
        if self.should_parallel_data_path(shard_len, self.parity_shard_count()) {
            self.verify_with_buffer_par(slices, workspace.as_mut_shards())
        } else {
            self.verify_with_buffer(slices, workspace.as_mut_shards())
        }
    }

    #[cfg(feature = "std")]
    pub fn reconstruct_opt(&self, shards: &mut [Option<Vec<u8>>]) -> Result<(), crate::Error> {
        self.ensure_classic_family_execution()?;
        let plan = self.plan_option_vec_reconstruct(shards, None)?;
        if plan.shard_len == 0 {
            return Ok(());
        }
        let missing_data = plan
            .invalid_indices
            .iter()
            .filter(|&&idx| idx < self.data_shard_count())
            .count();
        let missing = plan.invalid_indices.len();
        if self
            .reconstruct_parallel_decision(plan.shard_len, missing_data, missing, false)
            .use_parallel
        {
            let (data_policy, parity_policy) = self.reconstruct_stage_policies(false);
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
    pub fn reconstruct_data_opt(&self, shards: &mut [Option<Vec<u8>>]) -> Result<(), crate::Error> {
        self.ensure_classic_family_execution()?;
        let plan = self.plan_option_vec_reconstruct(shards, None)?;
        if plan.shard_len == 0 {
            return Ok(());
        }
        let missing_data = plan
            .invalid_indices
            .iter()
            .filter(|&&idx| idx < self.data_shard_count())
            .count();
        let missing = plan.invalid_indices.len();
        if self
            .reconstruct_parallel_decision(plan.shard_len, missing_data, missing, true)
            .use_parallel
        {
            let (data_policy, _parity_policy) = self.reconstruct_stage_policies(true);
            self.reconstruct_internal_option_vec_par_with_policy(shards, true, data_policy)
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
        self.ensure_classic_family_execution()?;
        if required.len() != self.total_shard_count() {
            return Err(crate::Error::InvalidShardFlags);
        }

        let data_only = required
            .iter()
            .enumerate()
            .all(|(idx, required)| !*required || idx < self.data_shard_count());

        if data_only {
            let plan = self.plan_option_vec_reconstruct(shards, Some(required))?;
            if plan.number_present == self.total_shard_count() {
                return Ok(());
            }
            return self.execute_option_vec_required_data_plan(shards, plan);
        }

        self.reconstruct_opt(shards)?;
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn decode_idx(
        &self,
        dst: &mut [Option<Vec<u8>>],
        expect_input: Option<&[bool]>,
        input: &[Option<Vec<u8>>],
    ) -> Result<(), crate::Error> {
        self.ensure_classic_family_execution()?;
        if dst.len() != self.total_shard_count() || input.len() != self.total_shard_count() {
            return Err(crate::Error::TooFewShards);
        }

        if let Some(expect_input) = expect_input {
            if expect_input.len() != self.total_shard_count() {
                return Err(crate::Error::InvalidShardFlags);
            }

            let mut valid_indices =
                smallvec::SmallVec::<[usize; 32]>::with_capacity(self.data_shard_count());
            let mut invalid_indices =
                smallvec::SmallVec::<[usize; 32]>::with_capacity(self.total_shard_count());

            for (idx, expected) in expect_input.iter().copied().enumerate() {
                if expected {
                    valid_indices.push(idx);
                } else {
                    invalid_indices.push(idx);
                }
            }

            if valid_indices.len() < self.data_shard_count() {
                return Err(crate::Error::TooFewShardsPresent);
            }

            let shard_len = input
                .iter()
                .chain(dst.iter())
                .find_map(|shard| shard.as_ref().map(|shard| shard.len()))
                .ok_or(crate::Error::TooFewShardsPresent)?;

            for shard in input.iter().flatten() {
                if shard.len() != shard_len {
                    return Err(crate::Error::IncorrectShardSize);
                }
            }
            for shard in dst.iter().flatten() {
                if shard.len() != shard_len {
                    return Err(crate::Error::IncorrectShardSize);
                }
            }

            let data_decode_matrix = self.get_data_decode_matrix(&valid_indices, &invalid_indices);
            let mut output_indices: smallvec::SmallVec<[usize; 32]> = smallvec::SmallVec::new();
            let mut matrix_rows: smallvec::SmallVec<[Vec<u8>; 32]> = smallvec::SmallVec::new();

            for (idx, shard) in dst.iter().enumerate() {
                let Some(_dst_shard) = shard.as_ref() else {
                    continue;
                };
                output_indices.push(idx);
                if idx < self.data_shard_count() {
                    matrix_rows.push(data_decode_matrix.get_row(idx).to_vec());
                } else {
                    let parity_rows = self.get_parity_rows();
                    let parity_row = parity_rows[idx - self.data_shard_count()];
                    let mut row = vec![0u8; self.data_shard_count()];
                    for col in 0..self.data_shard_count() {
                        let mut acc = 0u8;
                        for i in 0..self.data_shard_count() {
                            acc ^= super::mul(parity_row[i], data_decode_matrix.get(i, col));
                        }
                        row[col] = acc;
                    }
                    matrix_rows.push(row);
                }
            }

            if output_indices.is_empty() {
                return Ok(());
            }

            let mut input_positions = smallvec::SmallVec::<[usize; 32]>::new();
            let mut input_refs = smallvec::SmallVec::<[&[u8]; 32]>::new();
            for (col, &idx) in valid_indices
                .iter()
                .take(self.data_shard_count())
                .enumerate()
            {
                if let Some(shard) = input[idx].as_deref() {
                    input_positions.push(col);
                    input_refs.push(shard);
                }
            }

            if input_refs.is_empty() {
                return Ok(());
            }

            let reduced_rows: smallvec::SmallVec<[Vec<u8>; 32]> = matrix_rows
                .iter()
                .map(|row| input_positions.iter().map(|&col| row[col]).collect())
                .collect();

            let mut recovered_data: Vec<Vec<u8>> = output_indices
                .iter()
                .map(|_| vec![0u8; shard_len])
                .collect();
            {
                let mut output_refs: smallvec::SmallVec<[&mut [u8]; 32]> = recovered_data
                    .iter_mut()
                    .map(|shard| shard.as_mut_slice())
                    .collect();

                if output_refs.len() <= 2 {
                    self.decode_idx_execute_reduced_small_outputs(
                        &reduced_rows,
                        &input_refs,
                        &mut output_refs,
                    );
                } else {
                    let reduced_row_refs: smallvec::SmallVec<[&[u8]; 32]> =
                        reduced_rows.iter().map(|row| row.as_slice()).collect();
                    self.code_some_slices_chunked(&reduced_row_refs, &input_refs, &mut output_refs);
                }
            }

            for (&idx, recovered) in output_indices.iter().zip(recovered_data.into_iter()) {
                let dst_shard = dst[idx]
                    .as_deref_mut()
                    .expect("output index was collected only for present destinations");
                for (dst_byte, recovered_byte) in dst_shard.iter_mut().zip(recovered.iter()) {
                    *dst_byte ^= *recovered_byte;
                }
            }

            return Ok(());
        }

        for (dst_shard, input_shard) in dst.iter_mut().zip(input.iter()) {
            match (dst_shard.as_deref_mut(), input_shard.as_deref()) {
                (Some(dst), Some(input)) => {
                    if dst.len() != input.len() {
                        return Err(crate::Error::IncorrectShardSize);
                    }
                    for (dst_byte, input_byte) in dst.iter_mut().zip(input.iter()) {
                        *dst_byte ^= *input_byte;
                    }
                }
                (None, Some(_)) => return Err(crate::Error::TooFewShards),
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(feature = "std")]
fn reconstruct_parallel_policy_default(
    base: crate::ParallelPolicy,
    data_only: bool,
) -> crate::ParallelPolicy {
    let data_only_min = parse_positive_env_usize(RS_RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES_ENV)
        .unwrap_or(RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES);
    let full_min = parse_positive_env_usize(RS_RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES_ENV)
        .unwrap_or(RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES);
    let min_bytes_per_job = parse_positive_env_usize(RS_RECONSTRUCT_MIN_BYTES_PER_JOB_ENV)
        .unwrap_or(base.min_bytes_per_job);
    if data_only {
        crate::ParallelPolicy {
            min_parallel_shard_bytes: core::cmp::max(base.min_parallel_shard_bytes, data_only_min),
            min_bytes_per_job,
            max_jobs: base.max_jobs,
        }
    } else {
        crate::ParallelPolicy {
            min_parallel_shard_bytes: core::cmp::max(base.min_parallel_shard_bytes / 2, full_min),
            min_bytes_per_job,
            max_jobs: base.max_jobs,
        }
    }
}

#[cfg(feature = "std")]
fn parse_positive_env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
}

#[cfg(all(feature = "std", target_arch = "aarch64"))]
fn reconstruct_policy_cache_aarch64(base: crate::ParallelPolicy) -> RuntimeParallelPolicyCache {
    let mut reconstruct_full_data = reconstruct_parallel_policy_default(base, false);
    if let Some(value) =
        parse_positive_env_usize(RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES_ENV)
    {
        reconstruct_full_data.min_parallel_shard_bytes = value;
    }
    if let Some(value) = parse_positive_env_usize(RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB_ENV) {
        reconstruct_full_data.min_bytes_per_job = value;
    }
    if let Some(value) = std::env::var(RS_AARCH64_RECONSTRUCT_MAX_JOBS_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    {
        reconstruct_full_data.max_jobs = value;
    }

    let mut reconstruct_data = reconstruct_parallel_policy_default(base, true);
    reconstruct_data.min_parallel_shard_bytes = reconstruct_full_data.min_parallel_shard_bytes;
    reconstruct_data.min_bytes_per_job = reconstruct_full_data.min_bytes_per_job;
    reconstruct_data.max_jobs = reconstruct_full_data.max_jobs;

    let mut reconstruct_full_parity = reconstruct_parallel_policy_default(base, false);
    if let Some(value) =
        parse_positive_env_usize(RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB_ENV)
    {
        reconstruct_full_parity.min_bytes_per_job = value;
    }

    if let Some(value) = parse_positive_env_usize(RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB_ENV)
    {
        reconstruct_data.min_bytes_per_job = value;
    }

    RuntimeParallelPolicyCache {
        data: base,
        reconstruct_data,
        reconstruct_full_data,
        reconstruct_full_parity,
    }
}

#[cfg(all(feature = "std", not(target_arch = "aarch64")))]
pub(crate) fn resolve_runtime_parallel_policy_cache(
    base: crate::ParallelPolicy,
) -> RuntimeParallelPolicyCache {
    let reconstruct_data = reconstruct_parallel_policy_default(base, true);
    let reconstruct_full = reconstruct_parallel_policy_default(base, false);
    RuntimeParallelPolicyCache {
        data: base,
        reconstruct_data,
        reconstruct_full_data: reconstruct_full,
        reconstruct_full_parity: reconstruct_full,
    }
}

#[cfg(all(feature = "std", target_arch = "aarch64"))]
pub(crate) fn resolve_runtime_parallel_policy_cache(
    base: crate::ParallelPolicy,
) -> RuntimeParallelPolicyCache {
    reconstruct_policy_cache_aarch64(base)
}
