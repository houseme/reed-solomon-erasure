extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Once;

use crate::errors::Error;

pub(crate) mod decode;
mod driver;
pub(crate) mod encode;
mod mul_simd;
pub(crate) mod ops;
pub(crate) mod tables;
#[cfg(test)]
mod tests;
pub(crate) mod work;

pub(crate) const BITWIDTH16: usize = 16;
pub(crate) const ORDER16: usize = 1 << BITWIDTH16;
pub(crate) const MODULUS16: usize = ORDER16 - 1;
pub(crate) const POLYNOMIAL16: u32 = 0x1002D;
pub(crate) const WORK_SIZE16: usize = 32 << 10;

#[derive(Debug)]
pub(crate) struct LeopardGf16Tables {
    pub(crate) log_lut: Box<[u16; ORDER16]>,
    pub(crate) exp_lut: Box<[u16; ORDER16 * 2]>,
    pub(crate) fft_skew: Box<[u16; MODULUS16]>,
    pub(crate) log_walsh: Box<[u16; ORDER16]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LeopardGf16EncodeDriver {
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
    log_m01: u16,
    log_m23: u16,
    log_m02: u16,
}

#[derive(Debug, Clone, Copy)]
struct Stage2Block {
    r: usize,
    dist: usize,
    log_m: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct FftDit16Plan {
    mtrunc: usize,
    stage4_blocks: Vec<Stage4Block>,
    final_stage: Vec<Stage2Block>,
}

#[derive(Debug, Clone)]
pub(crate) struct IfftDit16Plan {
    mtrunc: usize,
    m: usize,
    initial_blocks: Vec<Stage4Block>,
    later_blocks: Vec<Stage4Block>,
    clear_start: usize,
    final_stage: Option<Stage2Block>,
}

static TABLES16: Once<LeopardGf16Tables> = Once::new();

pub(crate) fn init_leopard_gf16_tables() -> &'static LeopardGf16Tables {
    TABLES16.call_once(tables::build_tables16)
}

pub(crate) fn build_leopard_gf16_encode_driver(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> Result<LeopardGf16EncodeDriver, Error> {
    driver::build_leopard_gf16_encode_driver(data_shards, parity_shards, shard_size)
}

fn build_fft_dit16_plan(mtrunc: usize, m: usize, skew_lut: &[u16; MODULUS16]) -> FftDit16Plan {
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
        let mut blocks = Vec::new();
        let mut r = 0usize;
        while r < mtrunc {
            blocks.push(Stage2Block {
                r,
                dist: 1,
                log_m: skew_lut[r],
            });
            r += 2;
        }
        blocks
    } else {
        Vec::new()
    };

    FftDit16Plan {
        mtrunc,
        stage4_blocks,
        final_stage,
    }
}

/// Build FFT plan for the decode path.
///
/// Matches Go's `fftDIT` loop structure exactly:
///   dist4 = n; dist = n >> 2;
///   for dist != 0:
///     for r = 0; r < mtrunc; r += dist4:
///       for i = r; i < r+dist; i++:
///         stage4 block at (i, dist)
///     dist4 = dist; dist >>= 2
///   final stage: for r = 0; r < mtrunc; r += 2:
///     stage2 block at (r, 1)
pub(crate) fn build_fft_decode_dit16_plan(
    mtrunc: usize,
    n: usize,
    skew_lut: &[u16; MODULUS16],
) -> FftDit16Plan {
    // Go: skewLUT := fftSkew[m-1:]
    // skewLUT[i] = fftSkew[m-1+i], so we need base = n/2 - 1 (= m-1 in Go)
    // Actually, the caller passes n (the FFT size), but Go uses m = n/2 for the base.
    // Wait - let me check: Go's fftDIT is called with m=n for the decode path.
    // Actually no, Go passes `fftSkew[:]` (full table) and `m=n` to fftDIT.
    // Inside fftDIT: skewLUT = fftSkew[:] and m=n.
    // skewLUT[iend-1] = fftSkew[0 + iend - 1] = fft_skew[iend-1].
    // Wait, that can't be right - let me re-check.
    // Go: fftDIT(work, outputCount, n, fftSkew[:], &r.o)
    //   → mtrunc=outputCount, m=n, skewLUT=fftSkew[:]
    //   → skewLUT[iend-1] = fftSkew[iend-1]
    // So for the decode FFT, skew_lut IS the full table, and no base offset is needed!
    // But wait, the stage4 loop: dist4=m=n; dist=m>>2=n>>2.
    //   inner: for r=0; r<mtrunc; r+=dist4: iend=r+dist; skewLUT[iend-1]...
    // This is just fft_skew[iend-1]. So our original code was correct for stage4.
    // But what about the final stage? skewLUT[r] = fft_skew[r].
    // Hmm, but the test trace shows the wrong values...

    // Actually, let me re-examine: in the DECODE path, Go calls:
    //   fftDIT(work, outputCount, n, fftSkew[:], &r.o)
    // where outputCount = m + r.dataShards and n = ceilPow2(m + dataShards).
    // So mparam = n, skewLUT = fftSkew[:], mtrunc = outputCount.
    // Inside fftDIT: dist4 = mparam = n; dist = mparam >> 2 = n >> 2.
    // Final stage: skewLUT[r] = fftSkew[r]. Since mparam = n (not m), this is just fft_skew[r].
    //
    // But wait - for the ENCODER path, Go calls:
    //   fftDIT(work, parityShards, m, fftSkew[m-1:], &r.o)
    // So mparam = m, skewLUT = fftSkew[m-1:], mtrunc = parityShards.
    // Final stage: skewLUT[r] = fftSkew[m-1+r].
    //
    // The key difference: decode passes the FULL fftSkew table, encode passes fftSkew[m-1:].
    // So for the decode FFT plan, we use fft_skew[i] directly (no base offset).

    let mut stage4_blocks = Vec::new();
    let mut dist4 = n;
    let mut dist = n >> 2;
    while dist != 0 {
        let mut r = 0usize;
        while r < mtrunc {
            let i_end = r + dist;
            for i in r..i_end {
                stage4_blocks.push(Stage4Block {
                    r: i,
                    dist,
                    log_m01: skew_lut[i_end - 1],
                    log_m02: skew_lut[i_end + dist - 1],
                    log_m23: skew_lut[i_end + dist * 2 - 1],
                });
            }
            r += dist4;
        }
        dist4 = dist;
        dist >>= 2;
    }

    let final_stage = if dist4 == 2 {
        let mut blocks = Vec::new();
        let mut r = 0usize;
        while r < mtrunc {
            blocks.push(Stage2Block {
                r,
                dist: 1,
                log_m: skew_lut[r],
            });
            r += 2;
        }
        blocks
    } else {
        Vec::new()
    };

    FftDit16Plan {
        mtrunc,
        stage4_blocks,
        final_stage,
    }
}

fn build_ifft_dit16_plan(mtrunc: usize, m: usize, skew_lut: &[u16]) -> IfftDit16Plan {
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
            r: 0,
            dist,
            log_m: skew_lut[dist],
        })
    } else {
        None
    };

    IfftDit16Plan {
        mtrunc,
        m,
        initial_blocks,
        later_blocks,
        clear_start: (mtrunc + 3) & !3usize,
        final_stage,
    }
}

fn build_ifft_decode_dit16_plan(
    mtrunc: usize,
    m: usize,
    skew_lut: &[u16; MODULUS16],
) -> IfftDit16Plan {
    // Go: ifftDITDecoder(mtrunc, work, n, fftSkew[:], &r.o)
    // skewLUT = fftSkew[:] (full table, no base offset)
    // skewLUT[iend-1] = fftSkew[iend-1]
    // Final stage: skewLUT[dist-1] = fftSkew[dist-1]
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
                log_m01: skew_lut[i_end - 1],
                log_m02: skew_lut[i_end + dist - 1],
                log_m23: skew_lut[i_end + dist * 2 - 1],
            });
            r += dist4;
        }

        if full_groups < mtrunc {
            let r = full_groups;
            let i_end = r + dist;
            initial_blocks.push(Stage4Block {
                r,
                dist,
                log_m01: skew_lut[i_end - 1],
                log_m02: skew_lut[i_end + dist - 1],
                log_m23: skew_lut[i_end + dist * 2 - 1],
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
                    log_m01: skew_lut[i_end - 1],
                    log_m02: skew_lut[i_end + dist - 1],
                    log_m23: skew_lut[i_end + dist * 2 - 1],
                });
                r += dist4;
            }
            dist = dist4;
            dist4 <<= 2;
        }
    }

    let final_stage = if dist < m {
        Some(Stage2Block {
            r: 0,
            dist,
            log_m: skew_lut[dist - 1],
        })
    } else {
        None
    };

    IfftDit16Plan {
        mtrunc,
        m,
        initial_blocks,
        later_blocks,
        clear_start: (mtrunc + 3) & !3usize,
        final_stage,
    }
}

fn ceil_pow2(n: usize) -> usize {
    n.next_power_of_two()
}
