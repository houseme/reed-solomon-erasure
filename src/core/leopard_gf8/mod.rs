extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Once;

use crate::errors::Error;
#[cfg(feature = "std")]
use std::sync::atomic::AtomicUsize;
#[cfg(feature = "std")]
use std::sync::atomic::Ordering;

mod driver;
mod encode;
mod ops;
mod tables;
#[cfg(test)]
mod tests;
mod work;

const BITWIDTH8: usize = 8;
const ORDER8: usize = 1 << BITWIDTH8;
const MODULUS8: usize = ORDER8 - 1;
const POLYNOMIAL8: usize = 0x11D;
pub(crate) const WORK_SIZE8: usize = 32 << 10;
const WORK_SIZE8_HIGH_FANOUT: usize = 128 << 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Mul8Lut {
    value: [u8; 256],
}

#[derive(Debug)]
pub(crate) struct LeopardGf8Tables {
    pub(crate) fft_skew: Box<[u8; MODULUS8]>,
    pub(crate) log_walsh: Box<[u8; ORDER8]>,
    pub(crate) log_lut: Box<[u8; ORDER8]>,
    pub(crate) exp_lut: Box<[u8; ORDER8]>,
    pub(crate) mul_luts: Box<[Mul8Lut; ORDER8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LeopardGf8EncodeDriver {
    pub(crate) shard_size: usize,
    pub(crate) m: usize,
    pub(crate) mtrunc: usize,
    pub(crate) last_count: usize,
    pub(crate) chunk_size: usize,
    pub(crate) work_slices: usize,
    pub(crate) skew_offset: usize,
}

#[derive(Debug, Clone, Copy)]
struct Stage4Block {
    r: usize,
    dist: usize,
    log_m01: u8,
    log_m23: u8,
    log_m02: u8,
}

#[derive(Debug, Clone, Copy)]
struct Stage2Block {
    dist: usize,
    log_m: u8,
}

#[derive(Debug, Clone)]
struct FftDit8Plan {
    mtrunc: usize,
    stage4_blocks: Vec<Stage4Block>,
    final_stage: Option<Stage2Block>,
}

#[derive(Debug, Clone)]
struct IfftDit8Plan {
    mtrunc: usize,
    m: usize,
    initial_blocks: Vec<Stage4Block>,
    later_blocks: Vec<Stage4Block>,
    clear_start: usize,
    final_stage: Option<Stage2Block>,
}

#[derive(Debug, Default)]
pub(crate) struct LeopardGf8ProfileMetrics {
    encode_calls: AtomicUsize,
    encode_chunks: AtomicUsize,
    encode_full_groups: AtomicUsize,
    encode_remainder_groups: AtomicUsize,
    encode_later_group_calls: AtomicUsize,
    fft_stage_calls: AtomicUsize,
    ifft_stage_calls: AtomicUsize,
}

#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeopardGf8ProfileStats {
    pub encode_calls: usize,
    pub encode_chunks: usize,
    pub encode_full_groups: usize,
    pub encode_remainder_groups: usize,
    pub encode_later_group_calls: usize,
    pub fft_stage_calls: usize,
    pub ifft_stage_calls: usize,
}

static TABLES8: Once<LeopardGf8Tables> = Once::new();
const LEOPARD_GF8_XOR_CLONE_ENV: &str = "RSE_LEOPARD_GF8_XOR_CLONE";
#[cfg(feature = "std")]
static PROFILE8: LeopardGf8ProfileMetrics = LeopardGf8ProfileMetrics {
    encode_calls: AtomicUsize::new(0),
    encode_chunks: AtomicUsize::new(0),
    encode_full_groups: AtomicUsize::new(0),
    encode_remainder_groups: AtomicUsize::new(0),
    encode_later_group_calls: AtomicUsize::new(0),
    fft_stage_calls: AtomicUsize::new(0),
    ifft_stage_calls: AtomicUsize::new(0),
};

pub(crate) fn init_leopard_gf8_tables() -> &'static LeopardGf8Tables {
    TABLES8.call_once(tables::build_tables8)
}

#[cfg(feature = "std")]
pub(crate) fn leopard_gf8_profile_stats() -> LeopardGf8ProfileStats {
    LeopardGf8ProfileStats {
        encode_calls: PROFILE8.encode_calls.load(Ordering::Relaxed),
        encode_chunks: PROFILE8.encode_chunks.load(Ordering::Relaxed),
        encode_full_groups: PROFILE8.encode_full_groups.load(Ordering::Relaxed),
        encode_remainder_groups: PROFILE8.encode_remainder_groups.load(Ordering::Relaxed),
        encode_later_group_calls: PROFILE8.encode_later_group_calls.load(Ordering::Relaxed),
        fft_stage_calls: PROFILE8.fft_stage_calls.load(Ordering::Relaxed),
        ifft_stage_calls: PROFILE8.ifft_stage_calls.load(Ordering::Relaxed),
    }
}

#[cfg(feature = "std")]
pub(crate) fn reset_leopard_gf8_profile_stats() {
    PROFILE8.encode_calls.store(0, Ordering::Relaxed);
    PROFILE8.encode_chunks.store(0, Ordering::Relaxed);
    PROFILE8.encode_full_groups.store(0, Ordering::Relaxed);
    PROFILE8.encode_remainder_groups.store(0, Ordering::Relaxed);
    PROFILE8
        .encode_later_group_calls
        .store(0, Ordering::Relaxed);
    PROFILE8.fft_stage_calls.store(0, Ordering::Relaxed);
    PROFILE8.ifft_stage_calls.store(0, Ordering::Relaxed);
}

pub(crate) fn build_leopard_gf8_encode_driver(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> Result<LeopardGf8EncodeDriver, Error> {
    driver::build_leopard_gf8_encode_driver(data_shards, parity_shards, shard_size)
}

fn build_fft_dit8_plan(mtrunc: usize, m: usize, skew_lut: &[u8; MODULUS8]) -> FftDit8Plan {
    let mut stage4_blocks = Vec::new();
    let mut dist4 = m;
    let mut dist = m >> 2;
    while dist != 0 {
        let mut r = 0usize;
        while r < mtrunc {
            let i_end = r + dist;
            stage4_blocks.push(Stage4Block {
                r,
                dist,
                log_m01: skew_lut[i_end - 1],
                log_m02: skew_lut[i_end + dist - 1],
                log_m23: skew_lut[i_end + dist * 2 - 1],
            });
            r += dist4;
        }
        dist4 = dist;
        dist >>= 2;
    }

    let final_stage = if dist4 == 2 {
        Some(Stage2Block {
            dist: 1,
            log_m: skew_lut[0],
        })
    } else {
        None
    };

    FftDit8Plan {
        mtrunc,
        stage4_blocks,
        final_stage,
    }
}

fn build_ifft_dit8_plan(mtrunc: usize, m: usize, skew_lut: &[u8]) -> IfftDit8Plan {
    let mut initial_blocks = Vec::new();
    let mut later_blocks = Vec::new();
    let mut dist = 1usize;
    let mut dist4 = 4usize;

    if dist4 <= m {
        let full_groups = mtrunc & !3usize;
        let mut r = 0usize;
        while r < full_groups {
            let i_end = r + dist;
            initial_blocks.push(Stage4Block {
                r,
                dist,
                log_m01: skew_lut[i_end],
                log_m02: skew_lut[i_end + dist],
                log_m23: skew_lut[i_end + dist * 2],
            });
            r += dist4;
        }

        if full_groups < mtrunc {
            let r = full_groups;
            let i_end = r + dist;
            initial_blocks.push(Stage4Block {
                r,
                dist,
                log_m01: skew_lut[i_end],
                log_m02: skew_lut[i_end + dist],
                log_m23: skew_lut[i_end + dist * 2],
            });
        }

        dist = dist4;
        dist4 <<= 2;
        while dist4 <= m {
            let mut r = 0usize;
            while r < mtrunc {
                let i_end = r + dist;
                later_blocks.push(Stage4Block {
                    r,
                    dist,
                    log_m01: skew_lut[i_end],
                    log_m02: skew_lut[i_end + dist],
                    log_m23: skew_lut[i_end + dist * 2],
                });
                r += dist4;
            }
            dist = dist4;
            dist4 <<= 2;
        }
    }

    let final_stage = if dist < m {
        Some(Stage2Block {
            dist,
            log_m: skew_lut[dist],
        })
    } else {
        None
    };

    IfftDit8Plan {
        mtrunc,
        m,
        initial_blocks,
        later_blocks,
        clear_start: (mtrunc + 3) & !3usize,
        final_stage,
    }
}

pub(crate) fn encode_skeleton<T: AsRef<[u8]>, U: AsRef<[u8]> + AsMut<[u8]>>(
    data_shards: usize,
    parity_shards: usize,
    data: &[T],
    parity: &mut [U],
) -> Result<LeopardGf8EncodeDriver, Error> {
    encode::encode_skeleton(data_shards, parity_shards, data, parity)
}

pub(crate) fn encode_with_tables<T: AsRef<[u8]>, U: AsRef<[u8]> + AsMut<[u8]>>(
    data_shards: usize,
    parity_shards: usize,
    data: &[T],
    parity: &mut [U],
) -> Result<LeopardGf8EncodeDriver, Error> {
    encode::encode_with_tables(data_shards, parity_shards, data, parity)
}

fn ceil_pow2(n: usize) -> usize {
    n.next_power_of_two()
}
