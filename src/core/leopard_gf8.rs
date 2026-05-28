extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use smallvec::SmallVec;
use spin::Once;

use crate::errors::Error;

use super::leopard::validate_leopard_shard_len;

const BITWIDTH8: usize = 8;
const ORDER8: usize = 1 << BITWIDTH8;
const MODULUS8: usize = ORDER8 - 1;
const POLYNOMIAL8: usize = 0x11D;
pub(crate) const WORK_SIZE8: usize = 32 << 10;

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

#[derive(Debug)]
pub(crate) struct FlatWork {
    lanes: usize,
    lane_len: usize,
    buf: Box<[u8]>,
}

impl FlatWork {
    pub(crate) fn new(lanes: usize, lane_len: usize) -> Self {
        Self {
            lanes,
            lane_len,
            buf: vec![0u8; lanes * lane_len].into_boxed_slice(),
        }
    }

    pub(crate) fn lanes(&self) -> usize {
        self.lanes
    }

    pub(crate) fn lane_len(&self) -> usize {
        self.lane_len
    }

    pub(crate) fn lane(&self, idx: usize) -> &[u8] {
        let start = idx * self.lane_len;
        let end = start + self.lane_len;
        &self.buf[start..end]
    }

    pub(crate) fn lane_mut(&mut self, idx: usize) -> &mut [u8] {
        let start = idx * self.lane_len;
        let end = start + self.lane_len;
        &mut self.buf[start..end]
    }

    pub(crate) fn lane_views(&mut self, lanes: usize, size: usize) -> Vec<&mut [u8]> {
        self.buf
            .chunks_mut(self.lane_len)
            .take(lanes)
            .map(|lane| &mut lane[..size])
            .collect()
    }

    pub(crate) fn with_lane_views<R>(
        &mut self,
        lanes: usize,
        size: usize,
        f: impl FnOnce(&mut [&mut [u8]]) -> R,
    ) -> R {
        let mut views: SmallVec<[&mut [u8]; 96]> = self
            .buf
            .chunks_mut(self.lane_len)
            .take(lanes)
            .map(|lane| &mut lane[..size])
            .collect();
        f(&mut views)
    }
}

static TABLES8: Once<LeopardGf8Tables> = Once::new();
const LEOPARD_GF8_XOR_CLONE_ENV: &str = "RSE_LEOPARD_GF8_XOR_CLONE";

pub(crate) fn init_leopard_gf8_tables() -> &'static LeopardGf8Tables {
    TABLES8.call_once(build_tables8)
}

pub(crate) fn build_leopard_gf8_encode_driver(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> Result<LeopardGf8EncodeDriver, Error> {
    validate_leopard_shard_len(shard_size)?;
    let _tables = init_leopard_gf8_tables();

    let m = ceil_pow2(parity_shards.max(1));
    let mtrunc = core::cmp::min(data_shards, m);
    let last_count = data_shards % m;

    Ok(LeopardGf8EncodeDriver {
        shard_size,
        m,
        mtrunc,
        last_count,
        chunk_size: WORK_SIZE8,
        work_slices: m * 2,
        skew_offset: m.saturating_sub(1),
    })
}

pub(crate) fn encode_skeleton<T: AsRef<[u8]>, U: AsRef<[u8]> + AsMut<[u8]>>(
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
    let driver = build_leopard_gf8_encode_driver(data_shards, parity_shards, shard_size)?;

    let chunk_cap = core::cmp::min(shard_size, WORK_SIZE8);
    let mut work = vec![vec![0u8; chunk_cap]; driver.work_slices];
    let mut shard_views = vec![&[][..]; data_shards + parity_shards];
    let mut offset = 0usize;

    while offset < shard_size {
        let end = core::cmp::min(offset + driver.chunk_size, shard_size);
        for (idx, shard) in data.iter().enumerate() {
            shard_views[idx] = &shard.as_ref()[offset..end];
        }
        for (idx, shard) in parity.iter().enumerate() {
            shard_views[data_shards + idx] = &shard.as_ref()[offset..end];
        }
        for slice in &mut work {
            slice.resize(end - offset, 0);
        }
        offset = end;
    }

    Ok(driver)
}

pub(crate) fn encode_with_tables<T: AsRef<[u8]>, U: AsRef<[u8]> + AsMut<[u8]>>(
    data_shards: usize,
    parity_shards: usize,
    data: &[T],
    parity: &mut [U],
) -> Result<LeopardGf8EncodeDriver, Error> {
    let tables = init_leopard_gf8_tables();
    let driver = encode_skeleton(data_shards, parity_shards, data, parity)?;

    for output in parity.iter_mut() {
        output.as_mut().fill(0);
    }

    let chunk_cap = core::cmp::min(driver.shard_size, driver.chunk_size);
    let mut flat_work = FlatWork::new(driver.work_slices, chunk_cap);
    let zero = vec![0u8; chunk_cap];
    let mut offset = 0usize;

    while offset < driver.shard_size {
        let end = core::cmp::min(offset + driver.chunk_size, driver.shard_size);
        let size = end - offset;
        let skew = &tables.fft_skew[driver.skew_offset..];
        let zero_slice = &zero[..(end - offset)];
        let work_size = core::cmp::min(driver.m * 2, flat_work.lanes());

        flat_work.with_lane_views(work_size, size, |work| {
            ifft_dit_encoder8(
                data,
                driver.mtrunc,
                &mut work[..driver.m],
                None,
                driver.m,
                skew,
                offset,
                end,
                tables,
                zero_slice,
                false,
            );

            if driver.m < data_shards {
                let mut group_offset = driver.m;
                let mut skew_offset = driver.m;
                while group_offset + driver.m <= data_shards {
                    let (xor_dst, temp_work) = work[..work_size].split_at_mut(driver.m);
                    ifft_dit_encoder8(
                        &data[group_offset..],
                        driver.m,
                        temp_work,
                        Some(xor_dst),
                        driver.m,
                        &skew[skew_offset..],
                        offset,
                        end,
                        tables,
                        zero_slice,
                        false,
                    );
                    group_offset += driver.m;
                    skew_offset += driver.m;
                }

                if driver.last_count != 0 {
                    let (xor_dst, temp_work) = work[..work_size].split_at_mut(driver.m);
                    ifft_dit_encoder8(
                        &data[group_offset..],
                        driver.last_count,
                        temp_work,
                        Some(xor_dst),
                        driver.m,
                        &skew[skew_offset..],
                        offset,
                        end,
                        tables,
                        zero_slice,
                        false,
                    );
                }
            }

            fft_dit8(&mut work[..driver.m], parity_shards, driver.m, &tables.fft_skew, tables);

            for (idx, output) in parity.iter_mut().enumerate() {
                output.as_mut()[offset..end].copy_from_slice(&work[idx][..size]);
            }
        });
        offset = end;
    }

    Ok(driver)
}

fn build_tables8() -> LeopardGf8Tables {
    let (log_lut, exp_lut) = init_luts8();
    let log_lut_ref = &*log_lut;
    let exp_lut_ref = &*exp_lut;
    let (fft_skew, log_walsh) = init_fft_skew8(log_lut_ref, exp_lut_ref);
    let mul_luts = init_mul8_lut(log_lut_ref, exp_lut_ref);

    LeopardGf8Tables {
        fft_skew,
        log_walsh,
        log_lut,
        exp_lut,
        mul_luts,
    }
}

fn init_luts8() -> (Box<[u8; ORDER8]>, Box<[u8; ORDER8]>) {
    let cantor_basis = [1u8, 214, 152, 146, 86, 200, 88, 230];
    let mut exp_lut = Box::new([0u8; ORDER8]);
    let mut log_lut = Box::new([0u8; ORDER8]);

    let mut state = 1usize;
    for i in 0..MODULUS8 {
        exp_lut[state] = i as u8;
        state <<= 1;
        if state >= ORDER8 {
            state ^= POLYNOMIAL8;
        }
    }
    exp_lut[0] = MODULUS8 as u8;

    log_lut[0] = 0;
    for (i, basis) in cantor_basis.iter().copied().enumerate() {
        let width = 1usize << i;
        for j in 0..width {
            log_lut[j + width] = log_lut[j] ^ basis;
        }
    }

    for i in 0..ORDER8 {
        log_lut[i] = exp_lut[log_lut[i] as usize];
    }

    for i in 0..ORDER8 {
        exp_lut[log_lut[i] as usize] = i as u8;
    }
    exp_lut[MODULUS8] = exp_lut[0];

    (log_lut, exp_lut)
}

fn init_fft_skew8(
    log_lut: &[u8; ORDER8],
    exp_lut: &[u8; ORDER8],
) -> (Box<[u8; MODULUS8]>, Box<[u8; ORDER8]>) {
    let mut temp = [0u8; BITWIDTH8 - 1];
    for i in 1..BITWIDTH8 {
        temp[i - 1] = (1usize << i) as u8;
    }

    let mut fft_skew = Box::new([0u8; MODULUS8]);
    let mut log_walsh = Box::new([0u8; ORDER8]);

    for m in 0..(BITWIDTH8 - 1) {
        let step = 1usize << (m + 1);
        fft_skew[(1usize << m) - 1] = 0;

        for i in m..(BITWIDTH8 - 1) {
            let s = 1usize << (i + 1);
            let mut j = (1usize << m) - 1;
            while j < s {
                fft_skew[j + s] = fft_skew[j] ^ temp[i];
                j += step;
            }
        }

        temp[m] = (MODULUS8 - mul_log8(temp[m], log_lut[(temp[m] ^ 1) as usize], log_lut, exp_lut) as usize)
            as u8;

        for i in (m + 1)..(BITWIDTH8 - 1) {
            let sum = add_mod8(log_lut[(temp[i] ^ 1) as usize], temp[m]);
            temp[i] = mul_log8(temp[i], sum, log_lut, exp_lut);
        }
    }

    for i in 0..MODULUS8 {
        fft_skew[i] = log_lut[fft_skew[i] as usize];
    }

    for i in 0..ORDER8 {
        log_walsh[i] = log_lut[i];
    }
    log_walsh[0] = 0;
    fwht8(&mut log_walsh);

    (fft_skew, log_walsh)
}

fn init_mul8_lut(log_lut: &[u8; ORDER8], exp_lut: &[u8; ORDER8]) -> Box<[Mul8Lut; ORDER8]> {
    let mut mul_luts = Box::new([Mul8Lut { value: [0u8; 256] }; ORDER8]);

    for log_m in 0..ORDER8 {
        let mut tmp = [0u8; 64];
        let mut nibble = 0usize;
        let mut shift = 0usize;
        while nibble < 4 {
            let start = nibble * 16;
            for x_nibble in 0..16usize {
                tmp[start + x_nibble] =
                    mul_log8((x_nibble << shift) as u8, log_m as u8, log_lut, exp_lut);
            }
            nibble += 1;
            shift += 4;
        }

        let lut = &mut mul_luts[log_m];
        for i in 0..256usize {
            lut.value[i] = tmp[i & 15] ^ tmp[(i >> 4) + 16];
        }
    }

    mul_luts
}

fn mul_log8(a: u8, log_b: u8, log_lut: &[u8; ORDER8], exp_lut: &[u8; ORDER8]) -> u8 {
    if a == 0 {
        return 0;
    }

    exp_lut[add_mod8(log_lut[a as usize], log_b) as usize]
}

fn add_mod8(a: u8, b: u8) -> u8 {
    let sum = a as usize + b as usize;
    (sum + (sum >> BITWIDTH8)) as u8
}

fn sub_mod8(a: u8, b: u8) -> u8 {
    let dif = (a as isize) - (b as isize);
    let dif = if dif < 0 { dif + ORDER8 as isize } else { dif };
    let dif = dif as usize;
    (dif + (dif >> BITWIDTH8)) as u8
}

fn fwht8(data: &mut [u8; ORDER8]) {
    let mut dist = 1usize;
    let mut dist4 = 4usize;
    while dist4 <= ORDER8 {
        let mut r = 0usize;
        while r < ORDER8 {
            let mut off = r;
            for _ in 0..dist {
                let t0 = data[off];
                let t1 = data[off + dist];
                let t2 = data[off + dist * 2];
                let t3 = data[off + dist * 3];

                let (t0, t1) = fwht2_alt8(t0, t1);
                let (t2, t3) = fwht2_alt8(t2, t3);
                let (t0, t2) = fwht2_alt8(t0, t2);
                let (t1, t3) = fwht2_alt8(t1, t3);

                data[off] = t0;
                data[off + dist] = t1;
                data[off + dist * 2] = t2;
                data[off + dist * 3] = t3;
                off += 1;
            }
            r += dist4;
        }
        dist = dist4;
        dist4 <<= 2;
    }
}

fn fwht2_alt8(a: u8, b: u8) -> (u8, u8) {
    (add_mod8(a, b), sub_mod8(a, b))
}

fn slice_xor(input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    for (src, dst) in input.iter().zip(out.iter_mut()) {
        *dst ^= *src;
    }
}

fn slices_xor(input: &[Vec<u8>], out: &mut [Vec<u8>]) {
    assert_eq!(input.len(), out.len());
    for (src, dst) in input.iter().zip(out.iter_mut()) {
        slice_xor(src, dst);
    }
}

fn mul_slice_xor_reference(c: u8, input: &[u8], out: &mut [u8]) {
    let tables = init_leopard_gf8_tables();
    let lut = &tables.mul_luts[c as usize];
    assert_eq!(input.len(), out.len());
    for (value, slot) in input.iter().zip(out.iter_mut()) {
        *slot ^= lut.value[*value as usize];
    }
}

fn mulgf8(out: &mut [u8], input: &[u8], log_m: u8, tables: &LeopardGf8Tables) {
    let lut = &tables.mul_luts[log_m as usize];
    assert_eq!(input.len(), out.len());
    for (src, dst) in input.iter().zip(out.iter_mut()) {
        *dst = lut.value[*src as usize];
    }
}

fn fft_dit2(x: &mut [u8], y: &mut [u8], log_m: u8, tables: &LeopardGf8Tables) {
    if log_m == MODULUS8 as u8 {
        slice_xor(x, y);
    } else {
        let lut = &tables.mul_luts[log_m as usize];
        assert_eq!(x.len(), y.len());
        for (dst, src) in x.iter_mut().zip(y.iter()) {
            *dst ^= lut.value[*src as usize];
        }
    }
}

fn fft_dit2_lut(x: &mut [u8], y: &mut [u8], log_m: u8, lut: &[u8; 256]) {
    if log_m == MODULUS8 as u8 {
        slice_xor(x, y);
    } else {
        assert_eq!(x.len(), y.len());
        for (dst, src) in x.iter_mut().zip(y.iter()) {
            *dst ^= lut[*src as usize];
        }
    }
}

fn ifft_dit2(x: &mut [u8], y: &mut [u8], log_m: u8, tables: &LeopardGf8Tables) {
    if log_m == MODULUS8 as u8 {
        slice_xor(x, y);
    } else {
        let lut = &tables.mul_luts[log_m as usize];
        assert_eq!(x.len(), y.len());
        for (dst, src) in y.iter_mut().zip(x.iter()) {
            *dst ^= lut.value[*src as usize];
        }
    }
}

fn ifft_dit2_lut(x: &mut [u8], y: &mut [u8], log_m: u8, lut: &[u8; 256]) {
    if log_m == MODULUS8 as u8 {
        slice_xor(x, y);
    } else {
        assert_eq!(x.len(), y.len());
        for (dst, src) in y.iter_mut().zip(x.iter()) {
            *dst ^= lut[*src as usize];
        }
    }
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

        if has_a && has_c && let Some((a_ref, c_ref)) = get_pair_mut(work, a, c) {
            fft_dit2_lut(a_ref.as_mut(), c_ref.as_mut(), log_m02, lut02);
        }
        if has_b && has_d && let Some((b_ref, d_ref)) = get_pair_mut(work, b, d) {
            fft_dit2_lut(b_ref.as_mut(), d_ref.as_mut(), log_m02, lut02);
        }
        if has_a && has_b && let Some((a_ref, b_ref)) = get_pair_mut(work, a, b) {
            fft_dit2_lut(a_ref.as_mut(), b_ref.as_mut(), log_m01, lut01);
        }
        if has_c && has_d && let Some((c_ref, d_ref)) = get_pair_mut(work, c, d) {
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

        if has_a && has_b && let Some((a_ref, b_ref)) = get_pair_mut(work, a, b) {
            ifft_dit2_lut(a_ref.as_mut(), b_ref.as_mut(), log_m01, lut01);
        }
        if has_c && has_d && let Some((c_ref, d_ref)) = get_pair_mut(work, c, d) {
            ifft_dit2_lut(c_ref.as_mut(), d_ref.as_mut(), log_m23, lut23);
        }
        if has_a && has_c && let Some((a_ref, c_ref)) = get_pair_mut(work, a, c) {
            ifft_dit2_lut(a_ref.as_mut(), c_ref.as_mut(), log_m02, lut02);
        }
        if has_b && has_d && let Some((b_ref, d_ref)) = get_pair_mut(work, b, d) {
            ifft_dit2_lut(b_ref.as_mut(), d_ref.as_mut(), log_m02, lut02);
        }
    }
}

fn get_pair_mut<T>(slice: &mut [T], i: usize, j: usize) -> Option<(&mut T, &mut T)> {
    if i == j || i >= slice.len() || j >= slice.len() {
        return None;
    }

    let (lo, hi, swapped) = if i < j { (i, j, false) } else { (j, i, true) };
    let (left, right) = slice.split_at_mut(hi);
    let first = &mut left[lo];
    let second = &mut right[0];
    if swapped {
        Some((second, first))
    } else {
        Some((first, second))
    }
}

fn fft_dit8<W: AsMut<[u8]>>(
    work: &mut [W],
    mtrunc: usize,
    m: usize,
    skew_lut: &[u8; MODULUS8],
    tables: &LeopardGf8Tables,
) {
    let mut dist4 = m;
    let mut dist = m >> 2;
    while dist != 0 {
        let mut r = 0usize;
        while r < mtrunc {
            let i_end = r + dist;
            let log_m01 = skew_lut[i_end - 1];
            let log_m02 = skew_lut[i_end + dist - 1];
            let log_m23 = skew_lut[i_end + dist * 2 - 1];
            let mut i = r;
            while i < i_end {
                fft_dit4_at(work, i, dist, log_m01, log_m23, log_m02, tables);
                i += 1;
            }
            r += dist4;
        }
        dist4 = dist;
        dist >>= 2;
    }

    if dist4 == 2 {
        let mut r = 0usize;
        while r < mtrunc {
            let log_m = skew_lut[r];
            let (left, right) = work[r..r + 2].split_at_mut(1);
            fft_dit2(left[0].as_mut(), right[0].as_mut(), log_m, tables);
            r += 2;
        }
    }
}

fn ifft_dit_encoder8<T: AsRef<[u8]>, W: AsMut<[u8]>>(
    data: &[T],
    mtrunc: usize,
    work: &mut [W],
    mut xor_dst: Option<&mut [W]>,
    m: usize,
    skew_lut: &[u8],
    start: usize,
    end: usize,
    tables: &LeopardGf8Tables,
    zero: &[u8],
    use_xor_clone: bool,
) {
    let mut dist = 1usize;
    let mut dist4 = 4usize;
    let size = end - start;

    if dist4 <= m {
        let full_groups = mtrunc & !3usize;
        let mut r = 0usize;
        while r < full_groups {
            let i_end = r + dist;
            let log_m01 = skew_lut[i_end];
            let log_m02 = skew_lut[i_end + dist];
            let log_m23 = skew_lut[i_end + dist * 2];

            work[r].as_mut().copy_from_slice(&data[r].as_ref()[start..end]);
            work[r + 1]
                .as_mut()
                .copy_from_slice(&data[r + 1].as_ref()[start..end]);
            work[r + 2]
                .as_mut()
                .copy_from_slice(&data[r + 2].as_ref()[start..end]);
            work[r + 3]
                .as_mut()
                .copy_from_slice(&data[r + 3].as_ref()[start..end]);

            ifft_dit4_at(work, r, dist, log_m01, log_m23, log_m02, tables);
            r += dist4;
        }

        if full_groups < mtrunc {
            let r = full_groups;
            let rem = mtrunc - full_groups;
            for i in 0..rem {
                work[r + i]
                    .as_mut()
                    .copy_from_slice(&data[full_groups + i].as_ref()[start..end]);
            }
            for slot in work.iter_mut().skip(r + rem).take(4usize.saturating_sub(rem)) {
                slot.as_mut().copy_from_slice(&zero[..size]);
            }

            let i_end = r + dist;
            let log_m01 = skew_lut[i_end];
            let log_m02 = skew_lut[i_end + dist];
            let log_m23 = skew_lut[i_end + dist * 2];
            ifft_dit4_at(work, r, dist, log_m01, log_m23, log_m02, tables);
        }

        let clear_start = (mtrunc + 3) & !3usize;
        for slot in work.iter_mut().take(m).skip(clear_start) {
            slot.as_mut().fill(0);
        }

        dist = dist4;
        dist4 <<= 2;
        while dist4 <= m {
            let mut r = 0usize;
            while r < mtrunc {
                let i_end = r + dist;
                let log_m01 = skew_lut[i_end];
                let log_m02 = skew_lut[i_end + dist];
                let log_m23 = skew_lut[i_end + dist * 2];
                let mut i = r;
                while i < i_end {
                    ifft_dit4_at(work, i, dist, log_m01, log_m23, log_m02, tables);
                    i += 1;
                }
                r += dist4;
            }
            dist = dist4;
            dist4 <<= 2;
        }
    } else {
        for (idx, slot) in work.iter_mut().take(mtrunc).enumerate() {
            slot.as_mut().copy_from_slice(&data[idx].as_ref()[start..end]);
        }
        for slot in work.iter_mut().take(m).skip(mtrunc) {
            slot.as_mut().fill(0);
        }
    }

    if dist < m {
        let log_m = skew_lut[dist];
        for i in 0..dist {
            let (left, right) = work[i..i + dist + 1].split_at_mut(dist);
            ifft_dit2(left[0].as_mut(), right[0].as_mut(), log_m, tables);
        }
    }

    if let Some(xor_dst) = xor_dst.as_mut() {
        for idx in 0..m {
            let src = &*work[idx].as_mut();
            slice_xor(src, xor_dst[idx].as_mut());
        }
    }
}

fn ceil_pow2(n: usize) -> usize {
    n.next_power_of_two()
}

fn leopard_env_enabled(key: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leopard_gf8_tables_initialize_expected_shapes() {
        let tables = init_leopard_gf8_tables();
        assert_eq!(MODULUS8, tables.fft_skew.len());
        assert_eq!(ORDER8, tables.log_walsh.len());
        assert_eq!(ORDER8, tables.log_lut.len());
        assert_eq!(ORDER8, tables.exp_lut.len());
        assert_eq!(ORDER8, tables.mul_luts.len());
        assert_eq!(255, tables.log_lut[0]);
        assert_eq!(1, tables.exp_lut[0]);
    }

    #[test]
    fn test_leopard_gf8_encode_driver_expected_parameters() {
        let driver = build_leopard_gf8_encode_driver(64, 32, 1024 * 1024).unwrap();
        assert_eq!(32, driver.m);
        assert_eq!(32, driver.mtrunc);
        assert_eq!(0, driver.last_count);
        assert_eq!(WORK_SIZE8, driver.chunk_size);
        assert_eq!(64, driver.work_slices);
        assert_eq!(31, driver.skew_offset);
    }
}
