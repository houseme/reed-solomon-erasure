extern crate alloc;

use alloc::vec::Vec;

use crate::errors::Error;

use super::ops::{
    fft_dit2_16, fft_dit4_16, fwht16_mtrunc, fwht16_variable,
    ifft_dit2_16, ifft_dit4_16, mulgf16, slice_xor_u16,
};
use super::work::FlatWork16;
use super::{
    FftDit16Plan, IfftDit16Plan, LeopardGf16Tables, MODULUS16, WORK_SIZE16,
    build_fft_dit16_plan, build_ifft_decode_dit16_plan, ceil_pow2, init_leopard_gf16_tables,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LeopardGf16DecodeDriver {
    pub(crate) data_shards: usize,
    pub(crate) parity_shards: usize,
    pub(crate) shard_size: usize,
    pub(crate) m: usize,
    pub(crate) n: usize,
    pub(crate) work_size: usize,
    pub(crate) chunk_size: usize,
}

pub(crate) fn build_leopard_gf16_decode_driver(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> Result<LeopardGf16DecodeDriver, Error> {
    if shard_size == 0 || shard_size % 64 != 0 {
        return Err(Error::IncorrectShardSize);
    }
    let _tables = init_leopard_gf16_tables();

    let m = ceil_pow2(parity_shards.max(1));
    if m > MODULUS16 {
        return Err(Error::TooManyShards);
    }
    let work_size = m + data_shards;
    let n = ceil_pow2(work_size);
    if n > MODULUS16 {
        return Err(Error::TooManyShards);
    }

    Ok(LeopardGf16DecodeDriver {
        data_shards,
        parity_shards,
        shard_size,
        m,
        n,
        work_size,
        chunk_size: WORK_SIZE16,
    })
}

fn compute_error_locs16(
    missing_parity: &[usize],
    missing_data: &[usize],
    driver: &LeopardGf16DecodeDriver,
    tables: &LeopardGf16Tables,
) -> [u16; super::ORDER16] {
    let mut err_locs = [0u16; super::ORDER16];

    for &shard_idx in missing_parity {
        let pos = shard_idx - driver.data_shards;
        err_locs[pos] = 1;
    }

    for i in driver.parity_shards..driver.m {
        err_locs[i] = 1;
    }

    for &shard_idx in missing_data {
        err_locs[driver.m + shard_idx] = 1;
    }

    fwht16_mtrunc(&mut err_locs, driver.work_size);

    for i in 0..super::ORDER16 {
        if err_locs[i] == 0 {
            continue;
        }
        let product = (err_locs[i] as u64 * tables.log_walsh[i] as u64) % MODULUS16 as u64;
        err_locs[i] = product as u16;
    }

    fwht16_variable(&mut err_locs[..super::ORDER16]);

    err_locs
}

pub(crate) fn reconstruct_with_tables16(
    present: &[bool],
    outputs: &mut [&mut [u8]],
    input_data: &[Option<&[u8]>],
    data_shards: usize,
    parity_shards: usize,
    tables: &LeopardGf16Tables,
) -> Result<(), Error> {
    let total_shards = data_shards + parity_shards;
    if present.len() != total_shards || outputs.len() != total_shards || input_data.len() != total_shards {
        return Err(Error::IncorrectShardSize);
    }

    let shard_size = outputs.first().map(|s| s.len()).unwrap_or(0);
    if shard_size == 0 || shard_size % 64 != 0 {
        return Err(Error::IncorrectShardSize);
    }

    let number_present = present.iter().filter(|&&p| p).count();
    if number_present == total_shards {
        return Ok(());
    }
    if number_present < data_shards {
        return Err(Error::TooFewShardsPresent);
    }

    let driver = build_leopard_gf16_decode_driver(data_shards, parity_shards, shard_size)?;
    let shard_u16_len = shard_size / 2;

    let mut missing_parity = Vec::new();
    let mut missing_data = Vec::new();
    for i in data_shards..total_shards {
        if !present[i] {
            missing_parity.push(i);
        }
    }
    for i in 0..data_shards {
        if !present[i] {
            missing_data.push(i);
        }
    }

    let total_missing = missing_parity.len() + missing_data.len();
    if total_missing > parity_shards {
        return Err(Error::TooFewShardsPresent);
    }
    if total_missing == 0 {
        return Ok(());
    }

    let err_locs = compute_error_locs16(&missing_parity, &missing_data, &driver, tables);

    let ifft_plan = build_ifft_decode_dit16_plan(driver.work_size, driver.n, &*tables.fft_skew);
    let fft_plan = build_fft_dit16_plan(driver.work_size, driver.n, &*tables.fft_skew);

    let chunk_cap = core::cmp::min(shard_u16_len, driver.chunk_size);
    let mut work = FlatWork16::new(driver.n, chunk_cap);
    let mut scratch: Vec<u16> = Vec::new();

    let mut offset = 0usize;
    while offset < shard_u16_len {
        let end = core::cmp::min(offset + driver.chunk_size, shard_u16_len);
        let size = end - offset;

        if scratch.len() < size {
            scratch.resize(size, 0);
        }

        // Step 1: Multiply received data by error locator values.
        for i in 0..data_shards {
            let work_idx = driver.m + i;
            if present[i] {
                let data_bytes = input_data[i].as_ref().ok_or(Error::TooFewShardsPresent)?;
                let data_u16: &[u16] = unsafe {
                    core::slice::from_raw_parts(data_bytes.as_ptr().cast::<u16>(), shard_u16_len)
                };
                mulgf16(
                    work.lane_mut(work_idx),
                    &data_u16[offset..end],
                    err_locs[work_idx],
                    tables,
                );
            } else {
                work.lane_mut(work_idx)[..size].fill(0);
            }
        }
        for i in 0..parity_shards {
            let shard_idx = data_shards + i;
            if present[shard_idx] {
                let data_bytes = input_data[shard_idx].as_ref().ok_or(Error::TooFewShardsPresent)?;
                let data_u16: &[u16] = unsafe {
                    core::slice::from_raw_parts(data_bytes.as_ptr().cast::<u16>(), shard_u16_len)
                };
                mulgf16(
                    work.lane_mut(i),
                    &data_u16[offset..end],
                    err_locs[i],
                    tables,
                );
            } else {
                work.lane_mut(i)[..size].fill(0);
            }
        }
        for i in parity_shards..driver.m {
            work.lane_mut(i)[..size].fill(0);
        }
        for i in driver.work_size..driver.n {
            work.lane_mut(i)[..size].fill(0);
        }

        // Step 2: IFFT.
        ifft_dit_decode16_with_plan(&mut work, size, &ifft_plan, tables, driver.n, &mut scratch);

        // Step 3: Formal derivative.
        compute_formal_derivative16(&mut work, driver.n, size);

        // Step 4: FFT.
        fft_dit_decode16_with_plan(&mut work, size, &fft_plan, tables, driver.n, &mut scratch);

        // Step 5: Recover missing shards.
        for i in 0..total_shards {
            if present[i] {
                continue;
            }
            let (work_idx, err_idx) = if i >= data_shards {
                (i - data_shards, i - data_shards)
            } else {
                (driver.m + i, driver.m + i)
            };
            let inv_err = (MODULUS16 as u16).wrapping_sub(err_locs[err_idx]);

            let out_bytes = &mut *outputs[i];
            let out_u16: &mut [u16] = unsafe {
                core::slice::from_raw_parts_mut(out_bytes.as_mut_ptr().cast::<u16>(), shard_u16_len)
            };
            mulgf16(
                &mut out_u16[offset..end],
                work.lane(work_idx),
                inv_err,
                tables,
            );
        }
        offset = end;
    }

    Ok(())
}

fn ifft_dit_decode16_with_plan(
    work: &mut FlatWork16,
    size: usize,
    plan: &IfftDit16Plan,
    tables: &LeopardGf16Tables,
    n: usize,
    scratch: &mut [u16],
) {
    if plan.initial_blocks.is_empty() {
        for idx in plan.mtrunc..plan.m {
            work.lane_mut(idx)[..size].fill(0);
        }
    } else {
        for block in &plan.initial_blocks {
            let available = core::cmp::min(plan.mtrunc.saturating_sub(block.r), 4);
            for slot_idx in (block.r + available)..(block.r + 4) {
                if slot_idx < n {
                    work.lane_mut(slot_idx)[..size].fill(0);
                }
            }

            dit4_decode_at_16(
                false,
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

        for idx in plan.clear_start..plan.m {
            if idx < n {
                work.lane_mut(idx)[..size].fill(0);
            }
        }

        for block in &plan.later_blocks {
            dit4_decode_at_16(
                false,
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
    }

    if let Some(stage) = plan.final_stage {
        for i in 0..stage.dist {
            let a = i;
            let b = i + stage.dist;
            if b < n {
                if let Some((ra, rb)) = get_pair_mut_flat16(work, a, b) {
                    ifft_dit2_16(ra, rb, stage.log_m, tables);
                }
            }
        }
    }
}

fn fft_dit_decode16_with_plan(
    work: &mut FlatWork16,
    size: usize,
    plan: &FftDit16Plan,
    tables: &LeopardGf16Tables,
    n: usize,
    scratch: &mut [u16],
) {
    for block in &plan.stage4_blocks {
        dit4_decode_at_16(
            true,
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

    for stage in &plan.final_stage {
        let a = stage.r;
        let b = stage.r + stage.dist;
        if b < n {
            if let Some((ra, rb)) = get_pair_mut_flat16(work, a, b) {
                fft_dit2_16(ra, rb, stage.log_m, tables);
            }
        }
    }
}

fn compute_formal_derivative16(work: &mut FlatWork16, n: usize, _size: usize) {
    for i in 1..n {
        let width = ((i ^ (i - 1)) + 1) >> 1;
        for j in 0..width {
            let dst_idx = i - width + j;
            let src_idx = i + j;
            if src_idx < n && dst_idx < n {
                if let Some((s, d)) = get_pair_mut_flat16(work, src_idx, dst_idx) {
                    slice_xor_u16(s, d);
                }
            }
        }
    }
}

fn get_pair_mut_flat16<'a>(
    work: &'a mut FlatWork16,
    i: usize,
    j: usize,
) -> Option<(&'a mut [u16], &'a mut [u16])> {
    if i == j || i >= work.lanes() || j >= work.lanes() {
        return None;
    }
    unsafe {
        let ptr = work as *mut FlatWork16;
        let lane_i = (*ptr).lane_mut(i);
        let lane_j = (*ptr).lane_mut(j);
        Some((lane_i, lane_j))
    }
}

fn dit4_decode_at_16(
    forward: bool,
    work: &mut FlatWork16,
    _size: usize,
    base: usize,
    dist: usize,
    log_m01: u16,
    log_m23: u16,
    log_m02: u16,
    tables: &LeopardGf16Tables,
    scratch: &mut [u16],
) {
    for i in 0..dist {
        let a = base + i;
        let d = a + dist * 3;
        if d >= work.lanes() {
            dit4_decode_pairwise_16(forward, work, a, dist, log_m01, log_m23, log_m02, tables);
            continue;
        }

        let b = a + dist;
        let c = a + dist * 2;

        unsafe {
            let ptr = work as *mut FlatWork16;
            let a_ref = &mut *(*ptr).lane_mut(a);
            let b_ref = &mut *(*ptr).lane_mut(b);
            let c_ref = &mut *(*ptr).lane_mut(c);
            let d_ref = &mut *(*ptr).lane_mut(d);
            if forward {
                fft_dit4_16(a_ref, b_ref, c_ref, d_ref, log_m01, log_m23, log_m02, tables);
            } else {
                ifft_dit4_16(a_ref, b_ref, c_ref, d_ref, log_m01, log_m23, log_m02, tables);
            }
        }
        let _ = scratch;
    }
}

fn dit4_decode_pairwise_16(
    forward: bool,
    work: &mut FlatWork16,
    a: usize,
    dist: usize,
    log_m01: u16,
    log_m23: u16,
    log_m02: u16,
    tables: &LeopardGf16Tables,
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

    if forward {
        if has_a && has_c {
            if let Some((r1, r2)) = get_pair_mut_flat16(work, a, c) {
                fft_dit2_16(r1, r2, log_m02, tables);
            }
        }
        if has_b && has_d {
            if let Some((r1, r2)) = get_pair_mut_flat16(work, b, d) {
                fft_dit2_16(r1, r2, log_m02, tables);
            }
        }
        if has_a && has_b {
            if let Some((r1, r2)) = get_pair_mut_flat16(work, a, b) {
                fft_dit2_16(r1, r2, log_m01, tables);
            }
        }
        if has_c && has_d {
            if let Some((r1, r2)) = get_pair_mut_flat16(work, c, d) {
                fft_dit2_16(r1, r2, log_m23, tables);
            }
        }
    } else {
        if has_a && has_b {
            if let Some((r1, r2)) = get_pair_mut_flat16(work, a, b) {
                ifft_dit2_16(r1, r2, log_m01, tables);
            }
        }
        if has_c && has_d {
            if let Some((r1, r2)) = get_pair_mut_flat16(work, c, d) {
                ifft_dit2_16(r1, r2, log_m23, tables);
            }
        }
        if has_a && has_c {
            if let Some((r1, r2)) = get_pair_mut_flat16(work, a, c) {
                ifft_dit2_16(r1, r2, log_m02, tables);
            }
        }
        if has_b && has_d {
            if let Some((r1, r2)) = get_pair_mut_flat16(work, b, d) {
                ifft_dit2_16(r1, r2, log_m02, tables);
            }
        }
    }
}
