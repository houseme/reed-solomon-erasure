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

    pub(crate) fn setup_matrix(&self) -> &Matrix<F> {
        unreachable!(
            "LeopardGF8 prototype only keeps setup metadata, not an executable matrix handle"
        )
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
    if shard_len == 0 || shard_len % 64 != 0 {
        return Err(Error::IncorrectShardSize);
    }

    Ok(())
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
            Matrix::new(setup_matrix.row_count(), setup_matrix.col_count()),
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
        &tables,
    )
}
