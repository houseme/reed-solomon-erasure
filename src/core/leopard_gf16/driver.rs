use crate::errors::Error;

use super::{
    LeopardGf16EncodeDriver, MODULUS16, WORK_SIZE16, ceil_pow2, init_leopard_gf16_tables,
};
use crate::core::leopard::validate_leopard_shard_len;

pub(super) fn build_leopard_gf16_encode_driver(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> Result<LeopardGf16EncodeDriver, Error> {
    validate_leopard_shard_len(shard_size)?;
    let _tables = init_leopard_gf16_tables();

    let m = ceil_pow2(parity_shards.max(1));
    if m > MODULUS16 {
        return Err(Error::TooManyShards);
    }
    let mtrunc = core::cmp::min(data_shards, m);
    let last_count = data_shards % m;

    Ok(LeopardGf16EncodeDriver {
        shard_size,
        m,
        mtrunc,
        last_count,
        chunk_size: WORK_SIZE16,
        work_slices: m * 2,
        skew_offset: m.saturating_sub(1),
    })
}
