extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::errors::Error;

use super::ops::{
    fft_dit2, fft_dit4_full_lut_scratch, fwht_variable, ifft_dit2,
    ifft_dit4_full_lut_scratch, mulgf8, slice_xor,
};
use super::work::FlatWork;
use super::{
    FftDit8Plan, IfftDit8Plan, LeopardGf8Tables, MODULUS8, WORK_SIZE8, build_fft_dit8_plan,
    build_ifft_dit8_plan, ceil_pow2, init_leopard_gf8_tables,
};

/// Leopard GF8 decode driver — precomputed parameters for reconstruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LeopardGf8DecodeDriver {
    pub(crate) data_shards: usize,
    pub(crate) parity_shards: usize,
    pub(crate) shard_size: usize,
    pub(crate) m: usize,
    pub(crate) n: usize,
    pub(crate) work_size: usize,
    pub(crate) chunk_size: usize,
}

pub(crate) fn build_leopard_gf8_decode_driver(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> Result<LeopardGf8DecodeDriver, Error> {
    if shard_size == 0 || shard_size % 64 != 0 {
        return Err(Error::IncorrectShardSize);
    }
    let _tables = init_leopard_gf8_tables();

    let m = ceil_pow2(parity_shards.max(1));
    if m > MODULUS8 {
        return Err(Error::TooManyShards);
    }
    let work_size = m + data_shards;
    let n = ceil_pow2(work_size);
    if n > MODULUS8 {
        return Err(Error::TooManyShards);
    }

    Ok(LeopardGf8DecodeDriver {
        data_shards,
        parity_shards,
        shard_size,
        m,
        n,
        work_size,
        chunk_size: WORK_SIZE8,
    })
}

/// Compute error locator values using FWHT (Fast Walsh-Hadamard Transform).
///
/// Returns an array of `work_size` error locator values in log domain.
fn compute_error_locs(
    erasure_indices: &[usize],
    driver: &LeopardGf8DecodeDriver,
    tables: &LeopardGf8Tables,
) -> Vec<u8> {
    // Step 1: Initialize — 1 at erasure positions, 0 elsewhere.
    let mut err_locs = vec![0u8; driver.work_size];
    for &idx in erasure_indices {
        if idx < driver.work_size {
            err_locs[idx] = 1;
        }
    }

    // Step 2: FWHT on errLocs (size = work_size).
    fwht_variable(&mut err_locs);

    // Step 3: Multiply by log_walsh in log domain.
    // After FWHT, values are treated as log-domain indices (Lin et al. 2016 basis).
    for i in 0..driver.work_size {
        if err_locs[i] == 0 {
            continue;
        }
        let product = (err_locs[i] as usize * tables.log_walsh[i] as usize) % MODULUS8;
        err_locs[i] = tables.exp_lut[product];
    }

    // Step 4: Extend to ORDER8 size and apply second FWHT.
    err_locs.resize(super::ORDER8, 0);
    fwht_variable(&mut err_locs[..super::ORDER8]);

    // Convert from field domain to log domain for later use.
    for v in err_locs.iter_mut() {
        *v = tables.log_lut[*v as usize];
    }

    err_locs
}

/// Perform Leopard GF8 reconstruction using Forney's algorithm.
///
/// Recovery formula: `Original = -ErrLocator * FFT(Derivative(IFFT(ErrLocator * ReceivedData)))`
pub(crate) fn reconstruct_with_tables(
    shards: &mut [Option<&mut [u8]>],
    data_shards: usize,
    parity_shards: usize,
    tables: &LeopardGf8Tables,
) -> Result<(), Error> {
    let total_shards = data_shards + parity_shards;
    if shards.len() != total_shards {
        return Err(Error::IncorrectShardSize);
    }

    // Count present shards and find shard size.
    let mut number_present = 0usize;
    let mut shard_len = None::<usize>;
    for shard in shards.iter() {
        if let Some(s) = shard.as_ref() {
            let len = s.len();
            if len == 0 {
                return Err(Error::EmptyShard);
            }
            if let Some(old_len) = shard_len {
                if len != old_len {
                    return Err(Error::IncorrectShardSize);
                }
            }
            shard_len = Some(len);
            number_present += 1;
        }
    }

    if number_present == total_shards {
        return Ok(());
    }
    if number_present < data_shards {
        return Err(Error::TooFewShardsPresent);
    }

    let shard_size = shard_len.expect("at least one shard present");
    let driver = build_leopard_gf8_decode_driver(data_shards, parity_shards, shard_size)?;

    // Collect erasure indices (parity shards first, then data shards — matching Go order).
    let mut erasure_indices = Vec::new();
    for i in data_shards..total_shards {
        if shards[i].is_none() {
            erasure_indices.push(i);
        }
    }
    for i in 0..data_shards {
        if shards[i].is_none() {
            erasure_indices.push(i);
        }
    }

    if erasure_indices.len() > parity_shards {
        return Err(Error::TooFewShardsPresent);
    }
    if erasure_indices.is_empty() {
        return Ok(());
    }

    // Compute error locator values.
    let err_locs = compute_error_locs(&erasure_indices, &driver, tables);

    // Build FFT and IFFT plans for the decode work size.
    let ifft_plan = build_ifft_dit8_plan(driver.work_size, driver.n, &*tables.fft_skew);
    let fft_plan = build_fft_dit8_plan(driver.work_size, driver.n, &*tables.fft_skew);

    // Allocate work buffers and scratch.
    let chunk_cap = core::cmp::min(driver.shard_size, driver.chunk_size);
    let mut work = FlatWork::new(driver.n, chunk_cap);
    let mut scratch: Vec<u8> = Vec::new();

    // Process in chunks.
    let mut offset = 0usize;
    while offset < driver.shard_size {
        let end = core::cmp::min(offset + driver.chunk_size, driver.shard_size);
        let size = end - offset;

        if scratch.len() < size {
            scratch.resize(size, 0);
        }

        // Step 1: Multiply received data by error locator values.
        // Parity shards go into work[0..m], data shards go into work[m..m+data_shards].
        for i in 0..data_shards {
            let work_idx = driver.m + i;
            if let Some(shard) = shards[i].as_ref() {
                mulgf8(
                    work.lane_mut(work_idx),
                    &shard[offset..end],
                    err_locs[work_idx],
                    tables,
                );
            } else {
                work.lane_mut(work_idx)[..size].fill(0);
            }
        }
        for i in 0..parity_shards {
            let shard_idx = data_shards + i;
            if let Some(shard) = shards[shard_idx].as_ref() {
                mulgf8(
                    work.lane_mut(i),
                    &shard[offset..end],
                    err_locs[i],
                    tables,
                );
            } else {
                work.lane_mut(i)[..size].fill(0);
            }
        }
        // Zero remaining work slots.
        for i in driver.work_size..driver.n {
            work.lane_mut(i)[..size].fill(0);
        }

        // Step 2: IFFT on work buffer.
        ifft_dit_decode8_with_plan(&mut work, size, &ifft_plan, tables, driver.n, &mut scratch);

        // Step 3: Formal derivative.
        compute_formal_derivative(&mut work, driver.n, size);

        // Step 4: FFT on work buffer.
        fft_dit_decode8_with_plan(&mut work, size, &fft_plan, tables, driver.n, &mut scratch);

        // Step 5: Recover missing shards.
        for i in 0..total_shards {
            if shards[i].is_some() {
                continue;
            }
            let (work_idx, err_idx) = if i >= data_shards {
                (i - data_shards, i - data_shards)
            } else {
                (driver.m + i, driver.m + i)
            };
            let inv_err = (MODULUS8 as u8).wrapping_sub(err_locs[err_idx]);
            mulgf8(
                shards[i].as_mut().unwrap(),
                work.lane(work_idx),
                inv_err,
                tables,
            );
        }
        offset = end;
    }

    Ok(())
}

/// IFFT for the decode path using `FlatWork`.
fn ifft_dit_decode8_with_plan(
    work: &mut FlatWork,
    size: usize,
    plan: &IfftDit8Plan,
    tables: &LeopardGf8Tables,
    n: usize,
    scratch: &mut [u8],
) {
    if plan.initial_blocks.is_empty() {
        // Zero trailing lanes.
        for idx in plan.mtrunc..plan.m {
            work.lane_mut(idx)[..size].fill(0);
        }
    } else {
        for block in &plan.initial_blocks {
            let available = core::cmp::min(plan.mtrunc.saturating_sub(block.r), 4);
            // Zero unused slots in this block.
            for slot_idx in (block.r + available)..(block.r + 4) {
                if slot_idx < n {
                    work.lane_mut(slot_idx)[..size].fill(0);
                }
            }

            dit4_decode_at(
                TransformDir::Inverse,
                work,
                size,
                block.r,
                block.dist,
                block.log_m01,
                block.log_m23,
                block.log_m02,
                tables,
                scratch,
            );
        }

        // Zero trailing lanes.
        for idx in plan.clear_start..plan.m {
            if idx < n {
                work.lane_mut(idx)[..size].fill(0);
            }
        }

        for block in &plan.later_blocks {
            let i_end = block.r + block.dist;
            let mut i = block.r;
            while i < i_end {
                dit4_decode_at(
                    TransformDir::Inverse,
                    work,
                    size,
                    i,
                    block.dist,
                    block.log_m01,
                    block.log_m23,
                    block.log_m02,
                    tables,
                    scratch,
                );
                i += 1;
            }
        }
    }

    if let Some(stage) = plan.final_stage {
        for i in 0..stage.dist {
            let a = i;
            let b = i + stage.dist;
            if b < n {
                if let Some((ra, rb)) = get_pair_mut_flat(work, a, b) {
                    ifft_dit2(ra, rb, stage.log_m, tables);
                }
            }
        }
    }
}

/// FFT for the decode path using `FlatWork`.
fn fft_dit_decode8_with_plan(
    work: &mut FlatWork,
    size: usize,
    plan: &FftDit8Plan,
    tables: &LeopardGf8Tables,
    n: usize,
    scratch: &mut [u8],
) {
    for block in &plan.stage4_blocks {
        let i_end = block.r + block.dist;
        let mut i = block.r;
        while i < i_end {
            dit4_decode_at(
                TransformDir::Forward,
                work,
                size,
                i,
                block.dist,
                block.log_m01,
                block.log_m23,
                block.log_m02,
                tables,
                scratch,
            );
            i += 1;
        }
    }

    if let Some(stage) = plan.final_stage {
        let mut r = 0usize;
        while r < plan.mtrunc {
            let a = r;
            let b = r + stage.dist;
            if b < n {
                if let Some((ra, rb)) = get_pair_mut_flat(work, a, b) {
                    fft_dit2(ra, rb, stage.log_m, tables);
                }
            }
            r += stage.dist * 2;
        }
    }
}

/// Compute the formal derivative of the polynomial represented by work slots.
///
/// For i in 1..n: XOR work[i-width..i] into work[i..i+width]
/// where width = ((i ^ (i-1)) + 1) >> 1.
fn compute_formal_derivative(work: &mut FlatWork, n: usize, _size: usize) {
    for i in 1..n {
        let width = ((i ^ (i - 1)) + 1) >> 1;
        for j in 0..width {
            let src_idx = i - width + j;
            let dst_idx = i + j;
            if src_idx < n && dst_idx < n {
                if let Some((s, d)) = get_pair_mut_flat(work, src_idx, dst_idx) {
                    slice_xor(s, d);
                }
            }
        }
    }
}

/// Helper to get two mutable lane references from FlatWork.
fn get_pair_mut_flat<'a>(
    work: &'a mut FlatWork,
    i: usize,
    j: usize,
) -> Option<(&'a mut [u8], &'a mut [u8])> {
    if i == j || i >= work.lanes() || j >= work.lanes() {
        return None;
    }
    // SAFETY: i != j, both in bounds, and FlatWork lanes don't overlap.
    unsafe {
        let ptr = work as *mut FlatWork;
        let lane_i = (*ptr).lane_mut(i);
        let lane_j = (*ptr).lane_mut(j);
        Some((lane_i, lane_j))
    }
}

#[derive(Clone, Copy)]
enum TransformDir {
    Forward,
    Inverse,
}

/// DIT-4 butterfly for the decode path using `FlatWork`.
#[allow(clippy::too_many_arguments)]
fn dit4_decode_at(
    dir: TransformDir,
    work: &mut FlatWork,
    size: usize,
    base: usize,
    dist: usize,
    log_m01: u8,
    log_m23: u8,
    log_m02: u8,
    tables: &LeopardGf8Tables,
    scratch: &mut [u8],
) {
    let mul01 = &tables.mul_luts[log_m01 as usize];
    let mul23 = &tables.mul_luts[log_m23 as usize];
    let mul02 = &tables.mul_luts[log_m02 as usize];

    for i in 0..dist {
        let a = base + i;
        let b = a + dist;
        let c = a + dist * 2;
        let d = a + dist * 3;
        if d >= work.lanes() {
            // Boundary: pairwise fallback.
            dit4_decode_pairwise_one(dir, work, a, dist, log_m01, log_m23, log_m02, tables);
            continue;
        }

        // Use split_at_mut on FlatWork to get non-overlapping mutable references.
        // This is equivalent to the encode path's approach.
        dit4_decode_direct(dir, work, a, dist, mul01, mul23, mul02, scratch);
    }
}

/// Direct 4-lane butterfly using FlatWork split.
fn dit4_decode_direct(
    dir: TransformDir,
    work: &mut FlatWork,
    a: usize,
    dist: usize,
    mul01: &super::Mul8Lut,
    mul23: &super::Mul8Lut,
    mul02: &super::Mul8Lut,
    scratch: &mut [u8],
) {
    let b = a + dist;
    let c = a + dist * 2;
    let d = a + dist * 3;

    // Get 4 non-overlapping mutable references using raw pointer arithmetic.
    // SAFETY: a, b, c, d are all distinct (a < b < c < d) and < work.lanes().
    let (a_ref, b_ref, c_ref, d_ref) = unsafe {
        let ptr = work as *mut FlatWork;
        let a_ref = &mut *(*ptr).lane_mut(a);
        let b_ref = &mut *(*ptr).lane_mut(b);
        let c_ref = &mut *(*ptr).lane_mut(c);
        let d_ref = &mut *(*ptr).lane_mut(d);
        (a_ref, b_ref, c_ref, d_ref)
    };

    match dir {
        TransformDir::Forward => {
            fft_dit4_full_lut_scratch(
                a_ref,
                b_ref,
                c_ref,
                d_ref,
                &mul01.value,
                &mul01.low,
                &mul01.high,
                &mul23.value,
                &mul23.low,
                &mul23.high,
                &mul02.value,
                &mul02.low,
                &mul02.high,
                scratch,
            );
        }
        TransformDir::Inverse => {
            ifft_dit4_full_lut_scratch(
                a_ref,
                b_ref,
                c_ref,
                d_ref,
                &mul01.value,
                &mul01.low,
                &mul01.high,
                &mul23.value,
                &mul23.low,
                &mul23.high,
                &mul02.value,
                &mul02.low,
                &mul02.high,
                scratch,
            );
        }
    }
}

/// Pairwise fallback for boundary cases in decode path.
fn dit4_decode_pairwise_one(
    dir: TransformDir,
    work: &mut FlatWork,
    a: usize,
    dist: usize,
    log_m01: u8,
    log_m23: u8,
    log_m02: u8,
    tables: &LeopardGf8Tables,
) {
    let b = a + dist;
    let c = a + dist * 2;
    let d = a + dist * 3;
    let has_a = a < work.lanes();
    let has_b = b < work.lanes();
    let has_c = c < work.lanes();
    let has_d = d < work.lanes();
    let available = has_a as usize + has_b as usize + has_c as usize + has_d as usize;
    if available < 2 {
        return;
    }

    match dir {
        TransformDir::Forward => {
            if has_a && has_c {
                if let Some((r1, r2)) = get_pair_mut_flat(work, a, c) {
                    fft_dit2(r1, r2, log_m02, tables);
                }
            }
            if has_b && has_d {
                if let Some((r1, r2)) = get_pair_mut_flat(work, b, d) {
                    fft_dit2(r1, r2, log_m02, tables);
                }
            }
            if has_a && has_b {
                if let Some((r1, r2)) = get_pair_mut_flat(work, a, b) {
                    fft_dit2(r1, r2, log_m01, tables);
                }
            }
            if has_c && has_d {
                if let Some((r1, r2)) = get_pair_mut_flat(work, c, d) {
                    fft_dit2(r1, r2, log_m23, tables);
                }
            }
        }
        TransformDir::Inverse => {
            if has_a && has_b {
                if let Some((r1, r2)) = get_pair_mut_flat(work, a, b) {
                    ifft_dit2(r1, r2, log_m01, tables);
                }
            }
            if has_c && has_d {
                if let Some((r1, r2)) = get_pair_mut_flat(work, c, d) {
                    ifft_dit2(r1, r2, log_m23, tables);
                }
            }
            if has_a && has_c {
                if let Some((r1, r2)) = get_pair_mut_flat(work, a, c) {
                    ifft_dit2(r1, r2, log_m02, tables);
                }
            }
            if has_b && has_d {
                if let Some((r1, r2)) = get_pair_mut_flat(work, b, d) {
                    ifft_dit2(r1, r2, log_m02, tables);
                }
            }
        }
    }
}
