use crate::errors::Error;

use super::{
    LeopardGf8EncodeDriver, MODULUS8, WORK_SIZE8, WORK_SIZE8_HIGH_FANOUT, ceil_pow2,
    init_leopard_gf8_tables,
};
use crate::core::leopard::validate_leopard_shard_len;

/// Total shard count at or above which high-fanout chunk mode is always used.
/// Above this threshold, per-chunk FFT overhead dominates; larger chunks amortize it.
const HIGH_FANOUT_TOTAL_SHARDS: usize = 192;

/// Total shard count threshold for high-fanout mode when the last IFFT group has
/// a non-zero remainder. The extra IFFT pass per chunk increases overhead, so a
/// lower threshold compensates.
const HIGH_FANOUT_TOTAL_SHARDS_WITH_REMAINDER: usize = 144;

pub(super) fn build_leopard_gf8_encode_driver(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> Result<LeopardGf8EncodeDriver, Error> {
    validate_leopard_shard_len(shard_size)?;
    let _tables = init_leopard_gf8_tables();

    let m = ceil_pow2(parity_shards.max(1));
    // `fft_skew` has MODULUS8 (255) entries. When m > MODULUS8, skew_offset = m - 1
    // would exceed the array bounds. Reject early to prevent OOB access.
    if m > MODULUS8 {
        return Err(Error::TooManyShards);
    }
    let mtrunc = core::cmp::min(data_shards, m);
    let last_count = data_shards % m;
    let total_shards = data_shards.saturating_add(parity_shards);
    let high_fanout_chunk = total_shards >= HIGH_FANOUT_TOTAL_SHARDS
        || (total_shards >= HIGH_FANOUT_TOTAL_SHARDS_WITH_REMAINDER && last_count != 0);
    let chunk_size = if high_fanout_chunk && shard_size >= WORK_SIZE8_HIGH_FANOUT {
        WORK_SIZE8_HIGH_FANOUT
    } else {
        WORK_SIZE8
    };

    Ok(LeopardGf8EncodeDriver {
        shard_size,
        m,
        mtrunc,
        last_count,
        chunk_size,
        work_slices: m * 2,
        skew_offset: m.saturating_sub(1),
    })
}
