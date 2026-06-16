extern crate alloc;

#[cfg(feature = "std")]
pub(crate) mod cache_detect;
mod codec;
mod encode;
mod leopard;
pub(crate) mod leopard_gf16;
pub(crate) mod leopard_gf8;
mod metrics;
mod options;
#[cfg(feature = "std")]
mod parallel;
mod reconstruct;
mod shard_by_shard;
#[cfg(feature = "std")]
pub mod stream;
mod verify;
mod workspace;

use alloc::sync::Arc;
use alloc::vec::Vec;

use hashlink::LruCache;
#[cfg(feature = "std")]
use parking_lot::Mutex;
#[cfg(not(feature = "std"))]
use spin::Mutex;

use crate::Field;
use crate::matrix::Matrix;

use leopard::FamilyState;

#[cfg(feature = "std")]
pub use leopard_gf8::LeopardGf8ProfileStats;
#[cfg(feature = "std")]
pub(crate) use leopard_gf8::{leopard_gf8_profile_stats, reset_leopard_gf8_profile_stats};
#[cfg(feature = "std")]
pub use metrics::{ReconstructionCacheAnalysis, ReconstructionCacheStats, RuntimeProfileStats};
pub use options::{CodecFamily, CodecOptions, MatrixMode};
#[cfg(feature = "std")]
pub(crate) use parallel::RuntimeParallelPolicyCache;
#[cfg(feature = "std")]
pub use parallel::{PARALLEL_POLICY_VERSION, ParallelDecision, ParallelPolicy};
pub use shard_by_shard::ShardByShard;
pub use workspace::VerifyWorkspace;

#[cfg(feature = "std")]
use metrics::{ReconstructionCacheMetrics, RuntimeProfileMetrics};

pub(crate) const DATA_DECODE_MATRIX_CACHE_MIN_CAPACITY: usize = 128;
pub(crate) const DATA_DECODE_MATRIX_CACHE_MAX_CAPACITY: usize = 4096;
pub(crate) const CODE_SLICE_MIN_CHUNK_BYTES: usize = 16 * 1024;
pub(crate) const CODE_SLICE_DEFAULT_CHUNK_BYTES: usize = 64 * 1024;
pub(crate) const CODE_SLICE_LARGE_CHUNK_BYTES: usize = 256 * 1024;
pub(crate) const VERIFY_INLINE_SCRATCH_ELEMS: usize = 4 * 1024;
#[cfg(feature = "std")]
pub(crate) const PARALLEL_MIN_SHARD_BYTES: usize = 256 * 1024;

#[derive(Debug)]
pub struct ReedSolomon<F: Field> {
    data_shard_count: usize,
    parity_shard_count: usize,
    total_shard_count: usize,
    codec_family: CodecFamily,
    pub(crate) family_state: FamilyState<F>,
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
            Err(err) => {
                debug_assert!(
                    false,
                    "existing codec invariants must produce a valid clone, but got error: {err:?}"
                );
                let mut matrix = Matrix::new(self.matrix.row_count(), self.matrix.col_count());
                for row in 0..self.matrix.row_count() {
                    for col in 0..self.matrix.col_count() {
                        matrix.set(row, col, self.matrix.get(row, col));
                    }
                }

                let family_state = super::leopard::build_family_state(
                    self.codec_family,
                    self.data_shard_count,
                    self.parity_shard_count,
                    &matrix,
                )
                .unwrap_or_else(|_| {
                    debug_assert!(
                        false,
                        "fallback clone path could not rebuild family state from stored matrix"
                    );
                    match self.codec_family {
                        super::CodecFamily::Classic => FamilyState::Classic,
                        super::CodecFamily::LeopardGF16 => FamilyState::LeopardGF16,
                        super::CodecFamily::LeopardGF8 => {
                            debug_assert!(false, "LeopardGF8 should have a recoverable family state");
                            FamilyState::Classic
                        }
                    }
                });

                let options = self.options;
                #[cfg(feature = "std")]
                let policy_cache = self.policy_cache;

                ReedSolomon {
                    data_shard_count: self.data_shard_count,
                    parity_shard_count: self.parity_shard_count,
                    total_shard_count: self.total_shard_count,
                    codec_family: self.codec_family,
                    family_state,
                    matrix,
                    options,
                    #[cfg(feature = "std")]
                    policy_cache,
                    data_decode_matrix_cache: Mutex::new(
                        LruCache::new(options.inversion_cache_capacity),
                    ),
                    #[cfg(feature = "std")]
                    reconstruction_cache_metrics: ReconstructionCacheMetrics::default(),
                    #[cfg(feature = "std")]
                    runtime_profile_metrics: RuntimeProfileMetrics::default(),
                }
            }
        }
    }
}

impl<F: Field> PartialEq for ReedSolomon<F> {
    fn eq(&self, rhs: &ReedSolomon<F>) -> bool {
        self.data_shard_count == rhs.data_shard_count
            && self.parity_shard_count == rhs.parity_shard_count
            && self.codec_family == rhs.codec_family
    }
}

impl<F: Field> ReedSolomon<F> {
    /// Returns `Ok(())` for Classic, LeopardGF8, and LeopardGF16 families.
    ///
    /// Methods that are genuinely Classic-only (e.g., `update`, `decode_idx`) should
    /// check `is_leopard_gf8_family()` / `is_leopard_gf16_family()` separately and
    /// return an appropriate error.
    pub(crate) fn ensure_classic_family_execution(&self) -> Result<(), crate::Error> {
        match self.family_state {
            FamilyState::Classic | FamilyState::LeopardGF8(_) | FamilyState::LeopardGF16 => Ok(()),
        }
    }

    pub(crate) fn is_leopard_gf8_family(&self) -> bool {
        matches!(self.family_state, FamilyState::LeopardGF8(_))
    }

    pub(crate) fn is_leopard_gf16_family(&self) -> bool {
        matches!(self.family_state, FamilyState::LeopardGF16)
    }
}
