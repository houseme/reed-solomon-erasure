extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::Field;

use super::ReedSolomon;

/// Reusable parity scratch space for repeated verify calls.
///
/// This helper keeps the parity buffer allocation outside of `verify` so
/// repeated callers can naturally take the `verify_with_buffer` fast path
/// without having to manage `Vec<Vec<_>>` details themselves.
#[derive(PartialEq, Debug, Clone)]
pub struct VerifyWorkspace<F: Field> {
    parity: Vec<Vec<F::Elem>>,
}

impl<F: Field> VerifyWorkspace<F> {
    pub fn new(codec: &ReedSolomon<F>, shard_len: usize) -> Self {
        let mut parity = Vec::with_capacity(codec.parity_shard_count);
        for _ in 0..codec.parity_shard_count {
            parity.push(vec![F::zero(); shard_len]);
        }
        Self { parity }
    }

    pub fn parity_shards(&self) -> usize {
        self.parity.len()
    }

    pub fn shard_len(&self) -> Option<usize> {
        self.parity.first().map(Vec::len)
    }

    pub fn resize(&mut self, codec: &ReedSolomon<F>, shard_len: usize) {
        if self.parity.len() < codec.parity_shard_count {
            self.parity.reserve(codec.parity_shard_count - self.parity.len());
            while self.parity.len() < codec.parity_shard_count {
                self.parity.push(Vec::new());
            }
        } else if self.parity.len() > codec.parity_shard_count {
            self.parity.truncate(codec.parity_shard_count);
        }

        for shard in &mut self.parity {
            shard.resize(shard_len, F::zero());
        }
    }

    pub(crate) fn prepare(&mut self, codec: &ReedSolomon<F>, shard_len: usize) {
        if self.parity.len() != codec.parity_shard_count
            || self.shard_len() != Some(shard_len)
            || self.parity.iter().any(|shard| shard.len() != shard_len)
        {
            self.resize(codec, shard_len);
        }
    }

    pub(crate) fn as_mut_shards(&mut self) -> &mut [Vec<F::Elem>] {
        &mut self.parity
    }
}
