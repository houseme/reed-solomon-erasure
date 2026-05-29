extern crate alloc;

use alloc::vec::Vec;

use crate::errors::Error;
#[cfg(feature = "std")]
use std::sync::atomic::Ordering;

use super::ops::{
    fft_dit2, fft_dit2_lut, fft_dit4_full_lut, get_pair_mut, ifft_dit2, ifft_dit2_lut,
    ifft_dit4_full_lut, slice_xor,
};
use super::work::FlatWork;
use super::{
    FftDit8Plan, IfftDit8Plan, LEOPARD_GF8_XOR_CLONE_ENV, LeopardGf8EncodeDriver, LeopardGf8Tables,
    MODULUS8, PROFILE8, build_fft_dit8_plan, build_ifft_dit8_plan, build_leopard_gf8_encode_driver,
    init_leopard_gf8_tables,
};

pub(super) fn encode_skeleton<T: AsRef<[u8]>, U: AsRef<[u8]> + AsMut<[u8]>>(
    data_shards: usize,
    parity_shards: usize,
    data: &[T],
    parity: &mut [U],
) -> Result<LeopardGf8EncodeDriver, Error> {
    if data.len() != data_shards || parity.len() != parity_shards {
        return Err(Error::TooFewShards);
    }

    let shard_size = data
        .first()
        .map(|shard| shard.as_ref().len())
        .ok_or(Error::TooFewShards)?;
    build_leopard_gf8_encode_driver(data_shards, parity_shards, shard_size)
}

pub(super) fn encode_with_tables<T: AsRef<[u8]>, U: AsRef<[u8]> + AsMut<[u8]>>(
    data_shards: usize,
    parity_shards: usize,
    data: &[T],
    parity: &mut [U],
) -> Result<LeopardGf8EncodeDriver, Error> {
    let tables = init_leopard_gf8_tables();
    if data.len() != data_shards || parity.len() != parity_shards {
        return Err(Error::TooFewShards);
    }
    #[cfg(feature = "std")]
    PROFILE8.encode_calls.fetch_add(1, Ordering::Relaxed);
    let shard_size = data
        .first()
        .map(|shard| shard.as_ref().len())
        .ok_or(Error::TooFewShards)?;
    let driver = build_leopard_gf8_encode_driver(data_shards, parity_shards, shard_size)?;
    let skew = &tables.fft_skew[driver.skew_offset..];
    let first_ifft_plan = build_ifft_dit8_plan(driver.mtrunc, driver.m, skew);
    let fft_plan = build_fft_dit8_plan(parity_shards, driver.m, &tables.fft_skew);
    let mut later_ifft_plans = Vec::new();
    let mut remainder_ifft_plan = None;
    if driver.m < data_shards {
        let mut group_offset = driver.m;
        let mut skew_offset = driver.m;
        while group_offset + driver.m <= data_shards {
            later_ifft_plans.push(build_ifft_dit8_plan(
                driver.m,
                driver.m,
                &skew[skew_offset..],
            ));
            group_offset += driver.m;
            skew_offset += driver.m;
        }
        if driver.last_count != 0 {
            remainder_ifft_plan = Some(build_ifft_dit8_plan(
                driver.last_count,
                driver.m,
                &skew[skew_offset..],
            ));
        }
    }

    let chunk_cap = core::cmp::min(driver.shard_size, driver.chunk_size);
    let mut flat_work = FlatWork::new(driver.work_slices, chunk_cap);
    let zero = vec![0u8; chunk_cap];
    let mut offset = 0usize;

    while offset < driver.shard_size {
        #[cfg(feature = "std")]
        PROFILE8.encode_chunks.fetch_add(1, Ordering::Relaxed);
        let end = core::cmp::min(offset + driver.chunk_size, driver.shard_size);
        let size = end - offset;
        let zero_slice = &zero[..(end - offset)];
        let work_size = core::cmp::min(driver.m * 2, flat_work.lanes());

        flat_work.with_lane_views(work_size, size, |work| {
            #[cfg(feature = "std")]
            if first_ifft_plan.mtrunc == first_ifft_plan.m {
                PROFILE8.encode_full_groups.fetch_add(1, Ordering::Relaxed);
            }
            ifft_dit_encoder8_with_plan(
                data,
                &first_ifft_plan,
                &mut work[..driver.m],
                None,
                offset,
                end,
                tables,
                zero_slice,
                false,
            );

            let mut group_offset = driver.m;
            for plan in &later_ifft_plans {
                #[cfg(feature = "std")]
                {
                    PROFILE8
                        .encode_later_group_calls
                        .fetch_add(1, Ordering::Relaxed);
                    PROFILE8.encode_full_groups.fetch_add(1, Ordering::Relaxed);
                }
                let (xor_dst, temp_work) = work[..work_size].split_at_mut(driver.m);
                ifft_dit_encoder8_with_plan(
                    &data[group_offset..],
                    plan,
                    temp_work,
                    Some(xor_dst),
                    offset,
                    end,
                    tables,
                    zero_slice,
                    false,
                );
                group_offset += driver.m;
            }

            if let Some(plan) = remainder_ifft_plan.as_ref() {
                #[cfg(feature = "std")]
                PROFILE8
                    .encode_remainder_groups
                    .fetch_add(1, Ordering::Relaxed);
                let (xor_dst, temp_work) = work[..work_size].split_at_mut(driver.m);
                ifft_dit_encoder8_with_plan(
                    &data[group_offset..],
                    plan,
                    temp_work,
                    Some(xor_dst),
                    offset,
                    end,
                    tables,
                    zero_slice,
                    false,
                );
            }

            fft_dit8_with_plan(&mut work[..driver.m], &fft_plan, tables);

            for (idx, output) in parity.iter_mut().enumerate() {
                output.as_mut()[offset..end].copy_from_slice(&work[idx][..size]);
            }
        });
        offset = end;
    }

    Ok(driver)
}

fn fft_dit4_at<W: AsMut<[u8]>>(
    work: &mut [W],
    base: usize,
    dist: usize,
    log_m01: u8,
    log_m23: u8,
    log_m02: u8,
    tables: &LeopardGf8Tables,
) {
    let lut01 = &tables.mul_luts[log_m01 as usize].value;
    let lut23 = &tables.mul_luts[log_m23 as usize].value;
    let lut02 = &tables.mul_luts[log_m02 as usize].value;

    if base + dist * 4 <= work.len() {
        let ptr = work.as_mut_ptr();
        for i in 0..dist {
            let a = base + i;
            let b = a + dist;
            let c = a + dist * 2;
            let d = a + dist * 3;
            // SAFETY: full 4-lane window is in-bounds and indices are distinct by construction.
            unsafe {
                let a_ref = (*ptr.add(a)).as_mut();
                let b_ref = (*ptr.add(b)).as_mut();
                let c_ref = (*ptr.add(c)).as_mut();
                let d_ref = (*ptr.add(d)).as_mut();
                fft_dit4_full_lut(a_ref, b_ref, c_ref, d_ref, lut01, lut23, lut02);
            }
        }
        return;
    }

    for i in 0..dist {
        let a = base + i;
        let b = a + dist;
        let c = a + dist * 2;
        let d = a + dist * 3;
        let has_a = a < work.len();
        let has_b = b < work.len();
        let has_c = c < work.len();
        let has_d = d < work.len();

        let available = has_a as usize + has_b as usize + has_c as usize + has_d as usize;
        if available < 2 {
            return;
        }

        if has_a
            && has_c
            && let Some((a_ref, c_ref)) = get_pair_mut(work, a, c)
        {
            fft_dit2_lut(a_ref.as_mut(), c_ref.as_mut(), log_m02, lut02);
        }
        if has_b
            && has_d
            && let Some((b_ref, d_ref)) = get_pair_mut(work, b, d)
        {
            fft_dit2_lut(b_ref.as_mut(), d_ref.as_mut(), log_m02, lut02);
        }
        if has_a
            && has_b
            && let Some((a_ref, b_ref)) = get_pair_mut(work, a, b)
        {
            fft_dit2_lut(a_ref.as_mut(), b_ref.as_mut(), log_m01, lut01);
        }
        if has_c
            && has_d
            && let Some((c_ref, d_ref)) = get_pair_mut(work, c, d)
        {
            fft_dit2_lut(c_ref.as_mut(), d_ref.as_mut(), log_m23, lut23);
        }
    }
}

fn ifft_dit4_at<W: AsMut<[u8]>>(
    work: &mut [W],
    base: usize,
    dist: usize,
    log_m01: u8,
    log_m23: u8,
    log_m02: u8,
    tables: &LeopardGf8Tables,
) {
    let lut01 = &tables.mul_luts[log_m01 as usize].value;
    let lut23 = &tables.mul_luts[log_m23 as usize].value;
    let lut02 = &tables.mul_luts[log_m02 as usize].value;

    if base + dist * 4 <= work.len() {
        let ptr = work.as_mut_ptr();
        for i in 0..dist {
            let a = base + i;
            let b = a + dist;
            let c = a + dist * 2;
            let d = a + dist * 3;
            // SAFETY: full 4-lane window is in-bounds and indices are distinct by construction.
            unsafe {
                let a_ref = (*ptr.add(a)).as_mut();
                let b_ref = (*ptr.add(b)).as_mut();
                let c_ref = (*ptr.add(c)).as_mut();
                let d_ref = (*ptr.add(d)).as_mut();
                ifft_dit4_full_lut(a_ref, b_ref, c_ref, d_ref, lut01, lut23, lut02);
            }
        }
        return;
    }

    for i in 0..dist {
        let a = base + i;
        let b = a + dist;
        let c = a + dist * 2;
        let d = a + dist * 3;
        let has_a = a < work.len();
        let has_b = b < work.len();
        let has_c = c < work.len();
        let has_d = d < work.len();

        let available = has_a as usize + has_b as usize + has_c as usize + has_d as usize;
        if available < 2 {
            return;
        }

        if has_a
            && has_b
            && let Some((a_ref, b_ref)) = get_pair_mut(work, a, b)
        {
            ifft_dit2_lut(a_ref.as_mut(), b_ref.as_mut(), log_m01, lut01);
        }
        if has_c
            && has_d
            && let Some((c_ref, d_ref)) = get_pair_mut(work, c, d)
        {
            ifft_dit2_lut(c_ref.as_mut(), d_ref.as_mut(), log_m23, lut23);
        }
        if has_a
            && has_c
            && let Some((a_ref, c_ref)) = get_pair_mut(work, a, c)
        {
            ifft_dit2_lut(a_ref.as_mut(), c_ref.as_mut(), log_m02, lut02);
        }
        if has_b
            && has_d
            && let Some((b_ref, d_ref)) = get_pair_mut(work, b, d)
        {
            ifft_dit2_lut(b_ref.as_mut(), d_ref.as_mut(), log_m02, lut02);
        }
    }
}

fn fft_dit8_with_plan<W: AsMut<[u8]>>(
    work: &mut [W],
    plan: &FftDit8Plan,
    tables: &LeopardGf8Tables,
) {
    #[cfg(feature = "std")]
    PROFILE8.fft_stage_calls.fetch_add(1, Ordering::Relaxed);
    for block in &plan.stage4_blocks {
        let i_end = block.r + block.dist;
        let mut i = block.r;
        while i < i_end {
            fft_dit4_at(
                work,
                i,
                block.dist,
                block.log_m01,
                block.log_m23,
                block.log_m02,
                tables,
            );
            i += 1;
        }
    }

    if let Some(stage) = plan.final_stage {
        let mut r = 0usize;
        while r < plan.mtrunc {
            let (left, right) = work[r..r + stage.dist + 1].split_at_mut(stage.dist);
            fft_dit2(left[0].as_mut(), right[0].as_mut(), stage.log_m, tables);
            r += stage.dist * 2;
        }
    }
}

fn ifft_dit_encoder8_with_plan<T: AsRef<[u8]>, W: AsMut<[u8]>>(
    data: &[T],
    plan: &IfftDit8Plan,
    work: &mut [W],
    mut xor_dst: Option<&mut [W]>,
    start: usize,
    end: usize,
    tables: &LeopardGf8Tables,
    zero: &[u8],
    use_xor_clone: bool,
) {
    #[cfg(feature = "std")]
    PROFILE8.ifft_stage_calls.fetch_add(1, Ordering::Relaxed);
    let size = end - start;

    if plan.initial_blocks.is_empty() {
        for (idx, slot) in work.iter_mut().take(plan.mtrunc).enumerate() {
            slot.as_mut()
                .copy_from_slice(&data[idx].as_ref()[start..end]);
        }
        for slot in work.iter_mut().take(plan.m).skip(plan.mtrunc) {
            slot.as_mut().fill(0);
        }
    } else {
        for block in &plan.initial_blocks {
            let available = core::cmp::min(plan.mtrunc.saturating_sub(block.r), 4);
            for i in 0..available {
                work[block.r + i]
                    .as_mut()
                    .copy_from_slice(&data[block.r + i].as_ref()[start..end]);
            }
            for slot in work
                .iter_mut()
                .skip(block.r + available)
                .take(4usize.saturating_sub(available))
            {
                slot.as_mut().copy_from_slice(&zero[..size]);
            }

            ifft_dit4_at(
                work,
                block.r,
                block.dist,
                block.log_m01,
                block.log_m23,
                block.log_m02,
                tables,
            );
        }

        for slot in work.iter_mut().take(plan.m).skip(plan.clear_start) {
            slot.as_mut().fill(0);
        }

        for block in &plan.later_blocks {
            let i_end = block.r + block.dist;
            let mut i = block.r;
            while i < i_end {
                ifft_dit4_at(
                    work,
                    i,
                    block.dist,
                    block.log_m01,
                    block.log_m23,
                    block.log_m02,
                    tables,
                );
                i += 1;
            }
        }
    }

    if let Some(stage) = plan.final_stage {
        for i in 0..stage.dist {
            let (left, right) = work[i..i + stage.dist + 1].split_at_mut(stage.dist);
            ifft_dit2(left[0].as_mut(), right[0].as_mut(), stage.log_m, tables);
        }
    }

    if let Some(xor_dst) = xor_dst.as_mut() {
        for idx in 0..plan.m {
            let src = &*work[idx].as_mut();
            if use_xor_clone && idx < xor_dst.len() {
                xor_dst[idx].as_mut().copy_from_slice(src);
                continue;
            }
            slice_xor(src, xor_dst[idx].as_mut());
        }
    }
}

#[allow(dead_code)]
pub(super) fn fft_dit8<W: AsMut<[u8]>>(
    work: &mut [W],
    mtrunc: usize,
    m: usize,
    skew_lut: &[u8; MODULUS8],
    tables: &LeopardGf8Tables,
) {
    let plan = build_fft_dit8_plan(mtrunc, m, skew_lut);
    fft_dit8_with_plan(work, &plan, tables);
}

#[allow(dead_code)]
pub(super) fn ifft_dit_encoder8<T: AsRef<[u8]>, W: AsMut<[u8]>>(
    data: &[T],
    mtrunc: usize,
    work: &mut [W],
    xor_dst: Option<&mut [W]>,
    m: usize,
    skew_lut: &[u8],
    start: usize,
    end: usize,
    tables: &LeopardGf8Tables,
    zero: &[u8],
    use_xor_clone: bool,
) {
    let plan = build_ifft_dit8_plan(mtrunc, m, skew_lut);
    ifft_dit_encoder8_with_plan(
        data,
        &plan,
        work,
        xor_dst,
        start,
        end,
        tables,
        zero,
        use_xor_clone,
    );
}

#[allow(dead_code)]
pub(super) fn leopard_env_enabled(key: &str) -> bool {
    #[cfg(feature = "std")]
    {
        return std::env::var(key)
            .ok()
            .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
    }

    #[allow(unreachable_code)]
    false
}

#[allow(dead_code)]
pub(super) fn should_use_xor_clone() -> bool {
    leopard_env_enabled(LEOPARD_GF8_XOR_CLONE_ENV)
}
