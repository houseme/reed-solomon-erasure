extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::errors::Error;

use super::ops::{
    fft_dit2_16, fft_dit4_16, get_pair_mut_16, ifft_dit2_16, ifft_dit4_16, slice_xor_u16,
};
use super::work::FlatWork16;

use super::{
    FftDit16Plan, IfftDit16Plan, LeopardGf16EncodeDriver, LeopardGf16Tables, build_fft_dit16_plan,
    build_ifft_dit16_plan, build_leopard_gf16_encode_driver, init_leopard_gf16_tables,
};

#[cfg(feature = "std")]
thread_local! {
    static FLAT_WORK16_CACHE: std::cell::RefCell<Option<FlatWork16>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(feature = "std")]
thread_local! {
    static SCRATCH16_CACHE: std::cell::RefCell<Option<Vec<u16>>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn encode_with_tables16<T: AsRef<[u8]>, U: AsRef<[u8]> + AsMut<[u8]>>(
    data_shards: usize,
    parity_shards: usize,
    data: &[T],
    parity: &mut [U],
) -> Result<LeopardGf16EncodeDriver, Error> {
    let tables = init_leopard_gf16_tables();
    if data.len() != data_shards || parity.len() != parity_shards {
        return Err(Error::TooFewShards);
    }
    let shard_size = data
        .first()
        .map(|shard| shard.as_ref().len())
        .ok_or(Error::TooFewShards)?;
    let driver = build_leopard_gf16_encode_driver(data_shards, parity_shards, shard_size)?;

    let shard_u16_len = shard_size / 2;

    let skew = &tables.fft_skew[driver.skew_offset..];
    let first_ifft_plan = build_ifft_dit16_plan(driver.mtrunc, driver.m, skew);
    let fft_plan = build_fft_dit16_plan(parity_shards, driver.m, &tables.fft_skew);
    let mut later_ifft_plans = Vec::new();
    let mut remainder_ifft_plan = None;
    if driver.m < data_shards {
        let mut group_offset = driver.m;
        let mut skew_offset = driver.m;
        while group_offset + driver.m <= data_shards {
            later_ifft_plans.push(build_ifft_dit16_plan(
                driver.m,
                driver.m,
                &skew[skew_offset..],
            ));
            group_offset += driver.m;
            skew_offset += driver.m;
        }
        if driver.last_count != 0 {
            remainder_ifft_plan = Some(build_ifft_dit16_plan(
                driver.last_count,
                driver.m,
                &skew[skew_offset..],
            ));
        }
    }

    let chunk_cap = core::cmp::min(shard_u16_len, driver.chunk_size);
    let needed_lanes = driver.work_slices;
    let needed_lane_len = chunk_cap;

    #[cfg(feature = "std")]
    let mut flat_work = FLAT_WORK16_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(fw) = cache.take() {
            if fw.can_reuse(needed_lanes, needed_lane_len) {
                return fw;
            }
            drop(fw);
        }
        FlatWork16::new(needed_lanes, needed_lane_len)
    });
    #[cfg(not(feature = "std"))]
    let mut flat_work = FlatWork16::new(needed_lanes, needed_lane_len);

    #[cfg(feature = "std")]
    let mut scratch = SCRATCH16_CACHE.with(|cache| cache.take().unwrap_or_default());
    #[cfg(not(feature = "std"))]
    let mut scratch: Vec<u16> = Vec::new();

    // Zero all work lanes to prevent stale data from previous encode calls.
    for i in 0..flat_work.lanes() {
        flat_work.lane_mut(i).fill(0);
    }

    // Convert data from user byte layout to Go's GF16 split layout.
    // Go interprets each 64-byte chunk as: element i = byte[i] | (byte[i+32] << 8).
    // We rearrange bytes so that `as u16 LE` gives the same elements.
    // Convert each shard from user byte layout into aligned split-layout `u16`
    // elements (alignment- and endian-safe; see `user_bytes_to_work_u16`).
    let converted_data: Vec<Vec<u16>> = data
        .iter()
        .map(|d| super::ops::user_bytes_to_work_u16(d.as_ref()))
        .collect();

    let data_u16: Vec<&[u16]> = converted_data.iter().map(|d| d.as_slice()).collect();

    let mut offset = 0usize;
    while offset < shard_u16_len {
        let end = core::cmp::min(offset + driver.chunk_size, shard_u16_len);
        let size = end - offset;
        let work_size = core::cmp::min(driver.m * 2, flat_work.lanes());

        if scratch.len() < size {
            scratch.resize(size, 0);
        }

        // Build mutable views into flat work using with_lane_views.
        flat_work.with_lane_views(work_size, size, |work_views| {
            ifft_dit_encoder16_with_plan(
                &data_u16,
                &first_ifft_plan,
                &mut work_views[..driver.m],
                None,
                offset,
                end,
                tables,
                &mut scratch,
            );

            let mut group_offset = driver.m;
            for plan in &later_ifft_plans {
                let (xor_dst, temp_work) = work_views[..work_size].split_at_mut(driver.m);
                ifft_dit_encoder16_with_plan(
                    &data_u16[group_offset..],
                    plan,
                    temp_work,
                    Some(xor_dst),
                    offset,
                    end,
                    tables,
                    &mut scratch,
                );
                group_offset += driver.m;
            }

            if let Some(plan) = remainder_ifft_plan.as_ref() {
                let (xor_dst, temp_work) = work_views[..work_size].split_at_mut(driver.m);
                ifft_dit_encoder16_with_plan(
                    &data_u16[group_offset..],
                    plan,
                    temp_work,
                    Some(xor_dst),
                    offset,
                    end,
                    tables,
                    &mut scratch,
                );
            }

            fft_dit16_with_plan(&mut work_views[..driver.m], &fft_plan, tables, &mut scratch);

            // Write back parity shards as split-layout little-endian bytes.
            for (idx, output) in parity.iter_mut().enumerate() {
                let out_bytes = output.as_mut();
                super::ops::u16_to_work_bytes(
                    &work_views[idx][..size],
                    &mut out_bytes[offset * 2..end * 2],
                );
            }
        });
        offset = end;
    }

    #[cfg(feature = "std")]
    SCRATCH16_CACHE.with(|cache| {
        *cache.borrow_mut() = Some(scratch);
    });
    #[cfg(feature = "std")]
    FLAT_WORK16_CACHE.with(|cache| {
        *cache.borrow_mut() = Some(flat_work);
    });

    // Convert parity output from split layout back to user byte layout.
    // The parity was written as u16 values in split layout; convert bytes to contiguous.
    for output in parity.iter_mut() {
        let out_bytes = output.as_mut();
        let mut contiguous = vec![0u8; out_bytes.len()];
        super::ops::work_bytes_to_user_bytes(out_bytes, &mut contiguous);
        out_bytes.copy_from_slice(&contiguous);
    }

    Ok(driver)
}

fn zero_trailing_lanes_16(work: &mut [&mut [u16]], start_lane: usize, count: usize) {
    for i in start_lane..start_lane + count {
        if i < work.len() {
            work[i].fill(0);
        }
    }
}

fn fft_dit16_with_plan(
    work: &mut [&mut [u16]],
    plan: &FftDit16Plan,
    tables: &LeopardGf16Tables,
    scratch: &mut [u16],
) {
    for block in &plan.stage4_blocks {
        dit4_at_16(
            true,
            work,
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
        let r = stage.r;
        if r + stage.dist < work.len() {
            let (left, right) = work[r..r + stage.dist + 1].split_at_mut(stage.dist);
            fft_dit2_16(left[0], right[0], stage.log_m, tables);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn ifft_dit_encoder16_with_plan<T: AsRef<[u16]>>(
    data: &[T],
    plan: &IfftDit16Plan,
    work: &mut [&mut [u16]],
    mut xor_dst: Option<&mut [&mut [u16]]>,
    start: usize,
    end: usize,
    tables: &LeopardGf16Tables,
    scratch: &mut [u16],
) {
    let size = end - start;

    if plan.initial_blocks.is_empty() {
        for (idx, slot) in work.iter_mut().take(plan.mtrunc).enumerate() {
            slot[..size].copy_from_slice(&data[idx].as_ref()[start..end]);
        }
        zero_trailing_lanes_16(work, plan.mtrunc, plan.m - plan.mtrunc);
    } else {
        for block in &plan.initial_blocks {
            let available = core::cmp::min(plan.mtrunc.saturating_sub(block.r), 4);
            for i in 0..available {
                work[block.r + i][..size].copy_from_slice(&data[block.r + i].as_ref()[start..end]);
            }
            for slot in work
                .iter_mut()
                .skip(block.r + available)
                .take(4usize.saturating_sub(available))
            {
                slot[..size].fill(0);
            }

            dit4_at_16(
                false,
                work,
                block.r,
                block.dist,
                block.log_m01,
                block.log_m23,
                block.log_m02,
                tables,
                scratch,
            );
        }

        zero_trailing_lanes_16(
            work,
            plan.clear_start,
            plan.m.saturating_sub(plan.clear_start),
        );

        for block in &plan.later_blocks {
            dit4_at_16(
                false,
                work,
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
            if i + stage.dist < work.len() {
                let (left, right) = work[i..i + stage.dist + 1].split_at_mut(stage.dist);
                ifft_dit2_16(left[0], right[0], stage.log_m, tables);
            }
        }
    }

    if let Some(xor_dst) = xor_dst.as_mut() {
        for idx in 0..plan.m {
            if idx < work.len() && idx < xor_dst.len() {
                slice_xor_u16(xor_dst[idx], work[idx]);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn dit4_at_16(
    forward: bool,
    work: &mut [&mut [u16]],
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
        let b = a + dist;
        let c = a + dist * 2;
        let d = a + dist * 3;

        if d < work.len() {
            let (a_ref, b_ref, c_ref, d_ref) = unsafe {
                let ptr = work.as_mut_ptr();
                let a_ref = &mut *(*ptr.add(a));
                let b_ref = &mut *(*ptr.add(b));
                let c_ref = &mut *(*ptr.add(c));
                let d_ref = &mut *(*ptr.add(d));
                (a_ref, b_ref, c_ref, d_ref)
            };
            if forward {
                fft_dit4_16(
                    a_ref, b_ref, c_ref, d_ref, log_m01, log_m23, log_m02, tables,
                );
            } else {
                ifft_dit4_16(
                    a_ref, b_ref, c_ref, d_ref, log_m01, log_m23, log_m02, tables,
                );
            }
            let _ = scratch;
        } else {
            dit4_pairwise_16(forward, work, a, dist, log_m01, log_m23, log_m02, tables);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn dit4_pairwise_16(
    forward: bool,
    work: &mut [&mut [u16]],
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
    let has_a = a < work.len();
    let has_b = b < work.len();
    let has_c = c < work.len();
    let has_d = d < work.len();

    if forward {
        if has_a
            && has_c
            && let Some((r1, r2)) = get_pair_mut_16(work, a, c)
        {
            fft_dit2_16(r1, r2, log_m02, tables);
        }
        if has_b
            && has_d
            && let Some((r1, r2)) = get_pair_mut_16(work, b, d)
        {
            fft_dit2_16(r1, r2, log_m02, tables);
        }
        if has_a
            && has_b
            && let Some((r1, r2)) = get_pair_mut_16(work, a, b)
        {
            fft_dit2_16(r1, r2, log_m01, tables);
        }
        if has_c
            && has_d
            && let Some((r1, r2)) = get_pair_mut_16(work, c, d)
        {
            fft_dit2_16(r1, r2, log_m23, tables);
        }
    } else {
        if has_a
            && has_b
            && let Some((r1, r2)) = get_pair_mut_16(work, a, b)
        {
            ifft_dit2_16(r1, r2, log_m01, tables);
        }
        if has_c
            && has_d
            && let Some((r1, r2)) = get_pair_mut_16(work, c, d)
        {
            ifft_dit2_16(r1, r2, log_m23, tables);
        }
        if has_a
            && has_c
            && let Some((r1, r2)) = get_pair_mut_16(work, a, c)
        {
            ifft_dit2_16(r1, r2, log_m02, tables);
        }
        if has_b
            && has_d
            && let Some((r1, r2)) = get_pair_mut_16(work, b, d)
        {
            ifft_dit2_16(r1, r2, log_m02, tables);
        }
    }
}
