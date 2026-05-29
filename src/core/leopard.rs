extern crate alloc;

use alloc::vec::Vec;

use crate::Field;
use crate::errors::Error;
use crate::matrix::Matrix;

use super::CodecFamily;

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
        CodecFamily::LeopardGF16 => Err(Error::UnsupportedLeopardPrototype),
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
        CodecFamily::LeopardGF16 => Err(Error::UnsupportedLeopardPrototype),
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
