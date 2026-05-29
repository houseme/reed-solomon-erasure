extern crate alloc;

use alloc::vec;

use smallvec::SmallVec;

use crate::Field;
use crate::errors::Error;

use super::{ReedSolomon, VERIFY_INLINE_SCRATCH_ELEMS, VerifyWorkspace};

impl<F: Field> ReedSolomon<F> {
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

    pub fn verify<T: AsRef<[F::Elem]>>(&self, slices: &[T]) -> Result<bool, Error> {
        self.ensure_classic_family_execution()?;
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
        let scratch_len = self.parity_shard_count * slice_len;
        let mut scratch: SmallVec<[F::Elem; VERIFY_INLINE_SCRATCH_ELEMS]> =
            SmallVec::with_capacity(scratch_len);
        scratch.resize(scratch_len, F::zero());
        let mut buffer_views: SmallVec<[&mut [F::Elem]; 32]> =
            scratch.chunks_mut(slice_len).collect();

        Ok(self.check_some_slices_with_buffer_raw(&parity_rows, data, to_check, &mut buffer_views))
    }

    pub fn verify_with_workspace<T>(
        &self,
        slices: &[T],
        workspace: &mut VerifyWorkspace<F>,
    ) -> Result<bool, Error>
    where
        T: AsRef<[F::Elem]>,
    {
        self.ensure_classic_family_execution()?;
        check_piece_count!(all => self, slices);
        check_slices!(multi => slices);

        let slice_len = slices[0].as_ref().len();
        workspace.prepare(self, slice_len);
        self.verify_with_buffer(slices, workspace.as_mut_shards())
    }

    pub fn verify_with_buffer<T, U>(&self, slices: &[T], buffer: &mut [U]) -> Result<bool, Error>
    where
        T: AsRef<[F::Elem]>,
        U: AsRef<[F::Elem]> + AsMut<[F::Elem]>,
    {
        self.ensure_classic_family_execution()?;
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

    #[cfg(feature = "std")]
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
        self.ensure_classic_family_execution()?;
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
    pub fn verify_par<T>(&self, slices: &[T]) -> Result<bool, Error>
    where
        F::Elem: Send + Sync,
        T: AsRef<[F::Elem]> + Sync,
    {
        self.ensure_classic_family_execution()?;
        check_piece_count!(all => self, slices);
        check_slices!(multi => slices);

        let slice_len = slices[0].as_ref().len();
        let scratch_len = self.parity_shard_count * slice_len;
        let mut scratch: SmallVec<[F::Elem; VERIFY_INLINE_SCRATCH_ELEMS]> =
            SmallVec::with_capacity(scratch_len);
        scratch.resize(scratch_len, F::zero());
        let mut buffer_views: SmallVec<[&mut [F::Elem]; 32]> =
            scratch.chunks_mut(slice_len).collect();

        self.verify_with_buffer_par(slices, &mut buffer_views)
    }
}
