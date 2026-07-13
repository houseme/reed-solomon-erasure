extern crate alloc;

use alloc::vec::Vec;

use crate::Field;
use crate::errors::Error;
use crate::matrix::Matrix;

use super::CodecFamily;

/// Trait for safely reinterpreting `F::Elem` as `u8` for leopard encode.
///
/// Only implemented for `u8` (i.e., `galois_8::Field`). This enables the generic
/// `encode_sep` to call the `u8`-specific leopard FFT engine without `unsafe`.
pub(crate) trait AsLeopardU8: Sized {
    fn slice_to_u8(slice: &[Self]) -> &[u8];
    fn slice_to_u8_mut(slice: &mut [Self]) -> &mut [u8];
}

impl AsLeopardU8 for u8 {
    #[inline]
    fn slice_to_u8(slice: &[u8]) -> &[u8] {
        slice
    }
    #[inline]
    fn slice_to_u8_mut(slice: &mut [u8]) -> &mut [u8] {
        slice
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FamilyState<F: Field> {
    Classic,
    LeopardGF8(LeopardGF8Codec<F>),
    LeopardGF16,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LeopardGF8Codec<F: Field> {
    data_shards: usize,
    parity_shards: usize,
    total_shards: usize,
    setup_rows: usize,
    setup_cols: usize,
    parity_rows: Vec<Vec<F::Elem>>,
    _marker: core::marker::PhantomData<F>,
}

impl<F: Field> LeopardGF8Codec<F> {
    pub(crate) fn new(
        data_shards: usize,
        parity_shards: usize,
        setup_matrix: Matrix<F>,
    ) -> Result<Self, Error> {
        validate_leopard_gf8::<F>(data_shards, parity_shards)?;

        Ok(Self {
            data_shards,
            parity_shards,
            total_shards: data_shards.saturating_add(parity_shards),
            setup_rows: setup_matrix.row_count(),
            setup_cols: setup_matrix.col_count(),
            parity_rows: (data_shards..(data_shards + parity_shards))
                .map(|row| setup_matrix.get_row(row).to_vec())
                .collect(),
            _marker: core::marker::PhantomData,
        })
    }

    pub(crate) fn data_shards(&self) -> usize {
        self.data_shards
    }

    pub(crate) fn parity_shards(&self) -> usize {
        self.parity_shards
    }

    pub(crate) fn total_shards(&self) -> usize {
        self.total_shards
    }

    pub(crate) fn setup_shape(&self) -> (usize, usize) {
        (self.setup_rows, self.setup_cols)
    }

    pub(crate) fn parity_rows(&self) -> Vec<&[F::Elem]> {
        self.parity_rows.iter().map(|row| row.as_slice()).collect()
    }
}

pub(crate) fn leopard_gf8_state<F: Field>(
    family_state: &FamilyState<F>,
) -> Result<&LeopardGF8Codec<F>, Error> {
    match family_state {
        FamilyState::LeopardGF8(codec) => Ok(codec),
        FamilyState::Classic => Err(Error::UnsupportedCodecFamily),
        FamilyState::LeopardGF16 => Err(Error::UnsupportedLeopardPrototype),
    }
}

pub(crate) fn validate_leopard_shard_len(shard_len: usize) -> Result<(), Error> {
    if shard_len == 0 || !shard_len.is_multiple_of(64) {
        return Err(Error::IncorrectShardSize);
    }

    Ok(())
}

/// Required byte multiple (and cache-line alignment) of every Leopard shard.
///
/// Leopard shards must be a non-zero multiple of this value; see
/// [`validate_leopard_shard_len`]. Equal to
/// [`SHARD_ALIGNMENT`](crate::galois_8::SHARD_ALIGNMENT).
pub const LEOPARD_SHARD_MULTIPLE: usize = 64;

/// Computes a per-shard length, in bytes, that Leopard will accept for a payload
/// of `data_len` bytes spread across `data_shards` data shards.
///
/// The result is **always a non-zero multiple of [`LEOPARD_SHARD_MULTIPLE`]**, so
/// it is guaranteed to pass [`validate_leopard_shard_len`], for every input:
///
/// * `data_len == 0` (or any payload smaller than one block) clamps up to
///   [`LEOPARD_SHARD_MULTIPLE`].
/// * `data_shards == 0` is treated as a single shard (no divide-by-zero).
/// * Payloads near [`usize::MAX`] saturate to the largest multiple of
///   [`LEOPARD_SHARD_MULTIPLE`] that fits in a `usize`, rather than overflowing.
pub fn leopard_aligned_shard_len(data_len: usize, data_shards: usize) -> usize {
    // A zero shard count is nonsensical; treat the whole payload as one shard
    // instead of dividing by zero.
    let shards = if data_shards == 0 { 1 } else { data_shards };

    // Bytes per shard, rounding the payload up so every byte has a home.
    // `div_ceil` cannot overflow and `shards >= 1`, so this is total.
    let per_shard = data_len.div_ceil(shards);

    // Round up to the next multiple of LEOPARD_SHARD_MULTIPLE. `per_shard +
    // (MULTIPLE - remainder)` can overflow near usize::MAX, so saturate.
    let remainder = per_shard % LEOPARD_SHARD_MULTIPLE;
    let rounded = if remainder == 0 {
        per_shard
    } else {
        per_shard.saturating_add(LEOPARD_SHARD_MULTIPLE - remainder)
    };

    // Guarantee a non-zero result (covers data_len == 0), then floor back onto a
    // 64-boundary in case the saturation above landed on usize::MAX (which is
    // not itself a multiple of 64). For all normal inputs this is a no-op.
    let clamped = rounded.max(LEOPARD_SHARD_MULTIPLE);
    clamped - (clamped % LEOPARD_SHARD_MULTIPLE)
}

// Colocated with `leopard_aligned_shard_len`; the dispatch helpers below are
// unrelated, so allow the "items after test module" style lint here.
#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod leopard_shard_len_tests {
    use super::{LEOPARD_SHARD_MULTIPLE, leopard_aligned_shard_len, validate_leopard_shard_len};

    #[test]
    fn zero_data_len_clamps_to_multiple() {
        let len = leopard_aligned_shard_len(0, 4);
        assert_eq!(len, LEOPARD_SHARD_MULTIPLE);
        assert!(validate_leopard_shard_len(len).is_ok());
    }

    #[test]
    fn non_multiple_payload_rounds_up() {
        // ceil(100 / 4) = 25 bytes/shard -> rounds up to 64.
        let len = leopard_aligned_shard_len(100, 4);
        assert_eq!(len, 64);
        // 65 bytes/shard -> rounds up to 128.
        let len2 = leopard_aligned_shard_len(65 * 4, 4);
        assert_eq!(len2, 128);
        assert!(len.is_multiple_of(LEOPARD_SHARD_MULTIPLE));
        assert!(validate_leopard_shard_len(len).is_ok());
        assert!(validate_leopard_shard_len(len2).is_ok());
    }

    #[test]
    fn zero_shards_treated_as_one() {
        // ceil(200 / 1) = 200 -> rounds up to 256.
        let len = leopard_aligned_shard_len(200, 0);
        assert_eq!(len, 256);
        assert!(validate_leopard_shard_len(len).is_ok());
    }

    #[test]
    fn near_usize_max_saturates_to_valid_multiple() {
        let len = leopard_aligned_shard_len(usize::MAX, 1);
        assert_ne!(len, 0);
        assert!(len.is_multiple_of(LEOPARD_SHARD_MULTIPLE));
        assert!(validate_leopard_shard_len(len).is_ok());
        // Largest 64-multiple representable in a usize.
        assert_eq!(len, usize::MAX - (usize::MAX % LEOPARD_SHARD_MULTIPLE));
    }
}

pub(crate) fn build_family_state<F: Field>(
    codec_family: CodecFamily,
    data_shards: usize,
    parity_shards: usize,
    setup_matrix: &Matrix<F>,
) -> Result<FamilyState<F>, Error> {
    match codec_family {
        CodecFamily::Classic => Ok(FamilyState::Classic),
        CodecFamily::LeopardGF8 => Ok(FamilyState::LeopardGF8(LeopardGF8Codec::new(
            data_shards,
            parity_shards,
            {
                let mut matrix = Matrix::new(setup_matrix.row_count(), setup_matrix.col_count());
                for row in 0..setup_matrix.row_count() {
                    for col in 0..setup_matrix.col_count() {
                        matrix.set(row, col, setup_matrix.get(row, col));
                    }
                }
                matrix
            },
        )?)),
        CodecFamily::LeopardGF16 => Ok(FamilyState::LeopardGF16),
    }
}

pub(crate) fn validate_leopard_family<F: Field>(
    codec_family: CodecFamily,
    data_shards: usize,
    parity_shards: usize,
) -> Result<(), Error> {
    match codec_family {
        CodecFamily::Classic => Ok(()),
        CodecFamily::LeopardGF8 => validate_leopard_gf8::<F>(data_shards, parity_shards),
        CodecFamily::LeopardGF16 => validate_leopard_gf16::<F>(data_shards, parity_shards),
    }
}

fn validate_leopard_gf8<F: Field>(data_shards: usize, parity_shards: usize) -> Result<(), Error> {
    let total_shards = data_shards.saturating_add(parity_shards);

    if F::ORDER != 256 {
        return Err(Error::UnsupportedCodecFamily);
    }

    if total_shards == 0 || total_shards > 256 {
        return Err(Error::UnsupportedCodecFamily);
    }

    Ok(())
}

fn validate_leopard_gf16<F: Field>(data_shards: usize, parity_shards: usize) -> Result<(), Error> {
    let total_shards = data_shards.saturating_add(parity_shards);

    if F::ORDER != 256 {
        return Err(Error::UnsupportedCodecFamily);
    }

    // The GF16 Leopard codec handles bytes as little-endian `u16` split-layout
    // pairs. Big-endian correctness is not yet verified end to end, so reject at
    // construction rather than risk silently producing wrong results
    // (rustfs/backlog#1238). Little-endian builds are unaffected; GF8 Leopard is
    // byte-oriented and endian-agnostic, so it is not gated here.
    if cfg!(target_endian = "big") {
        return Err(Error::UnsupportedCodecFamily);
    }

    if total_shards == 0 || total_shards > 65536 {
        return Err(Error::UnsupportedCodecFamily);
    }

    Ok(())
}

/// Dispatch encode to the Leopard GF8 FFT engine.
///
/// Accepts `u8` slices directly. The caller (`encode_leopard_gf8_sep`) is responsible
/// for converting from `F::Elem` to `u8` (safe because Leopard GF8 is only
/// instantiated for `galois_8::Field` where `Elem = u8`).
pub(crate) fn leopard_gf8_encode(
    data_shards: usize,
    parity_shards: usize,
    data: &[&[u8]],
    parity: &mut [&mut [u8]],
) -> Result<(), Error> {
    super::leopard_gf8::encode_with_tables(data_shards, parity_shards, data, parity)?;
    Ok(())
}

/// Dispatch encode to the Leopard GF16 FFT engine.
pub(crate) fn leopard_gf16_encode(
    data_shards: usize,
    parity_shards: usize,
    data: &[&[u8]],
    parity: &mut [&mut [u8]],
) -> Result<(), Error> {
    super::leopard_gf16::encode::encode_with_tables16(data_shards, parity_shards, data, parity)?;
    Ok(())
}

/// Dispatch reconstruct to the Leopard GF16 Forney decoder.
pub(crate) fn leopard_gf16_reconstruct(
    present: &[bool],
    outputs: &mut [&mut [u8]],
    input_data: &[Option<&[u8]>],
    data_shards: usize,
    parity_shards: usize,
) -> Result<(), Error> {
    let tables = super::leopard_gf16::init_leopard_gf16_tables();
    super::leopard_gf16::decode::reconstruct_with_tables16(
        present,
        outputs,
        input_data,
        data_shards,
        parity_shards,
        tables,
    )
}
