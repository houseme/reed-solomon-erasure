extern crate alloc;

use alloc::vec::Vec;

use super::{BITWIDTH8, LeopardGf8Tables, MODULUS8, ORDER8, init_leopard_gf8_tables};

pub(super) fn mul_log8(a: u8, log_b: u8, log_lut: &[u8; ORDER8], exp_lut: &[u8; ORDER8]) -> u8 {
    if a == 0 {
        return 0;
    }

    exp_lut[add_mod8(log_lut[a as usize], log_b) as usize]
}

pub(super) fn add_mod8(a: u8, b: u8) -> u8 {
    let sum = a as usize + b as usize;
    (sum + (sum >> BITWIDTH8)) as u8
}

pub(super) fn sub_mod8(a: u8, b: u8) -> u8 {
    let dif = (a as isize) - (b as isize);
    let dif = if dif < 0 { dif + ORDER8 as isize } else { dif };
    let dif = dif as usize;
    (dif + (dif >> BITWIDTH8)) as u8
}

pub(super) fn fwht8(data: &mut [u8; ORDER8]) {
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

pub(super) fn slice_xor(input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());

    let (input64, input_tail64) = input.as_chunks::<64>();
    let (out64, out_tail64) = out.as_chunks_mut::<64>();

    for (src, dst) in input64.iter().zip(out64.iter_mut()) {
        dst[0] ^= src[0];
        dst[1] ^= src[1];
        dst[2] ^= src[2];
        dst[3] ^= src[3];
        dst[4] ^= src[4];
        dst[5] ^= src[5];
        dst[6] ^= src[6];
        dst[7] ^= src[7];
        dst[8] ^= src[8];
        dst[9] ^= src[9];
        dst[10] ^= src[10];
        dst[11] ^= src[11];
        dst[12] ^= src[12];
        dst[13] ^= src[13];
        dst[14] ^= src[14];
        dst[15] ^= src[15];
        dst[16] ^= src[16];
        dst[17] ^= src[17];
        dst[18] ^= src[18];
        dst[19] ^= src[19];
        dst[20] ^= src[20];
        dst[21] ^= src[21];
        dst[22] ^= src[22];
        dst[23] ^= src[23];
        dst[24] ^= src[24];
        dst[25] ^= src[25];
        dst[26] ^= src[26];
        dst[27] ^= src[27];
        dst[28] ^= src[28];
        dst[29] ^= src[29];
        dst[30] ^= src[30];
        dst[31] ^= src[31];
        dst[32] ^= src[32];
        dst[33] ^= src[33];
        dst[34] ^= src[34];
        dst[35] ^= src[35];
        dst[36] ^= src[36];
        dst[37] ^= src[37];
        dst[38] ^= src[38];
        dst[39] ^= src[39];
        dst[40] ^= src[40];
        dst[41] ^= src[41];
        dst[42] ^= src[42];
        dst[43] ^= src[43];
        dst[44] ^= src[44];
        dst[45] ^= src[45];
        dst[46] ^= src[46];
        dst[47] ^= src[47];
        dst[48] ^= src[48];
        dst[49] ^= src[49];
        dst[50] ^= src[50];
        dst[51] ^= src[51];
        dst[52] ^= src[52];
        dst[53] ^= src[53];
        dst[54] ^= src[54];
        dst[55] ^= src[55];
        dst[56] ^= src[56];
        dst[57] ^= src[57];
        dst[58] ^= src[58];
        dst[59] ^= src[59];
        dst[60] ^= src[60];
        dst[61] ^= src[61];
        dst[62] ^= src[62];
        dst[63] ^= src[63];
    }

    let (input8, input_tail) = input_tail64.as_chunks::<8>();
    let (out8, out_tail) = out_tail64.as_chunks_mut::<8>();

    for (src, dst) in input8.iter().zip(out8.iter_mut()) {
        dst[0] ^= src[0];
        dst[1] ^= src[1];
        dst[2] ^= src[2];
        dst[3] ^= src[3];
        dst[4] ^= src[4];
        dst[5] ^= src[5];
        dst[6] ^= src[6];
        dst[7] ^= src[7];
    }

    for (src, dst) in input_tail.iter().zip(out_tail.iter_mut()) {
        *dst ^= *src;
    }
}

pub(super) fn slices_xor(input: &[Vec<u8>], out: &mut [Vec<u8>]) {
    assert_eq!(input.len(), out.len());
    for (src, dst) in input.iter().zip(out.iter_mut()) {
        slice_xor(src, dst);
    }
}

pub(super) fn mul_slice_xor_reference(c: u8, input: &[u8], out: &mut [u8]) {
    let tables = init_leopard_gf8_tables();
    let lut = &tables.mul_luts[c as usize];
    assert_eq!(input.len(), out.len());
    for (value, slot) in input.iter().zip(out.iter_mut()) {
        *slot ^= lut.value[*value as usize];
    }
}

pub(super) fn mulgf8(out: &mut [u8], input: &[u8], log_m: u8, tables: &LeopardGf8Tables) {
    let lut = &tables.mul_luts[log_m as usize];
    assert_eq!(input.len(), out.len());
    for (src, dst) in input.iter().zip(out.iter_mut()) {
        *dst = lut.value[*src as usize];
    }
}

pub(super) fn fft_dit2(x: &mut [u8], y: &mut [u8], log_m: u8, tables: &LeopardGf8Tables) {
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

pub(super) fn fft_dit2_lut(x: &mut [u8], y: &mut [u8], log_m: u8, lut: &[u8; 256]) {
    if log_m == MODULUS8 as u8 {
        slice_xor(x, y);
    } else {
        assert_eq!(x.len(), y.len());
        for (dst, src) in x.iter_mut().zip(y.iter()) {
            *dst ^= lut[*src as usize];
        }
    }
}

pub(super) fn ifft_dit2(x: &mut [u8], y: &mut [u8], log_m: u8, tables: &LeopardGf8Tables) {
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

pub(super) fn ifft_dit2_lut(x: &mut [u8], y: &mut [u8], log_m: u8, lut: &[u8; 256]) {
    if log_m == MODULUS8 as u8 {
        slice_xor(x, y);
    } else {
        assert_eq!(x.len(), y.len());
        for (dst, src) in y.iter_mut().zip(x.iter()) {
            *dst ^= lut[*src as usize];
        }
    }
}

#[inline(always)]
pub(super) fn fft_dit4_full_lut(
    a: &mut [u8],
    b: &mut [u8],
    c: &mut [u8],
    d: &mut [u8],
    lut01: &[u8; 256],
    lut23: &[u8; 256],
    lut02: &[u8; 256],
) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), c.len());
    assert_eq!(a.len(), d.len());

    #[inline(always)]
    fn step(
        a: &mut u8,
        b: &mut u8,
        c: &mut u8,
        d: &mut u8,
        lut01: &[u8; 256],
        lut23: &[u8; 256],
        lut02: &[u8; 256],
    ) {
        let c0 = *c;
        let d0 = *d;
        let b1 = *b ^ lut02[d0 as usize];
        let a1 = *a ^ lut02[c0 as usize];
        *a = a1 ^ lut01[b1 as usize];
        *b = b1;
        *c = c0 ^ lut23[d0 as usize];
    }

    let (a4, a_tail) = a.as_chunks_mut::<4>();
    let (b4, b_tail) = b.as_chunks_mut::<4>();
    let (c4, c_tail) = c.as_chunks_mut::<4>();
    let (d4, d_tail) = d.as_chunks_mut::<4>();

    for (((a_chunk, b_chunk), c_chunk), d_chunk) in a4
        .iter_mut()
        .zip(b4.iter_mut())
        .zip(c4.iter_mut())
        .zip(d4.iter_mut())
    {
        step(
            &mut a_chunk[0],
            &mut b_chunk[0],
            &mut c_chunk[0],
            &mut d_chunk[0],
            lut01,
            lut23,
            lut02,
        );
        step(
            &mut a_chunk[1],
            &mut b_chunk[1],
            &mut c_chunk[1],
            &mut d_chunk[1],
            lut01,
            lut23,
            lut02,
        );
        step(
            &mut a_chunk[2],
            &mut b_chunk[2],
            &mut c_chunk[2],
            &mut d_chunk[2],
            lut01,
            lut23,
            lut02,
        );
        step(
            &mut a_chunk[3],
            &mut b_chunk[3],
            &mut c_chunk[3],
            &mut d_chunk[3],
            lut01,
            lut23,
            lut02,
        );
    }

    for (((a_byte, b_byte), c_byte), d_byte) in a_tail
        .iter_mut()
        .zip(b_tail.iter_mut())
        .zip(c_tail.iter_mut())
        .zip(d_tail.iter_mut())
    {
        step(a_byte, b_byte, c_byte, d_byte, lut01, lut23, lut02);
    }
}

#[inline(always)]
pub(super) fn ifft_dit4_full_lut(
    a: &mut [u8],
    b: &mut [u8],
    c: &mut [u8],
    d: &mut [u8],
    lut01: &[u8; 256],
    lut23: &[u8; 256],
    lut02: &[u8; 256],
) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), c.len());
    assert_eq!(a.len(), d.len());

    #[inline(always)]
    fn step(
        a: &mut u8,
        b: &mut u8,
        c: &mut u8,
        d: &mut u8,
        lut01: &[u8; 256],
        lut23: &[u8; 256],
        lut02: &[u8; 256],
    ) {
        let a0 = *a;
        let c0 = *c;
        let b1 = *b ^ lut01[a0 as usize];
        *c = c0 ^ lut02[a0 as usize];
        *b = b1;
        *d ^= lut23[c0 as usize] ^ lut02[b1 as usize];
    }

    let (a4, a_tail) = a.as_chunks_mut::<4>();
    let (b4, b_tail) = b.as_chunks_mut::<4>();
    let (c4, c_tail) = c.as_chunks_mut::<4>();
    let (d4, d_tail) = d.as_chunks_mut::<4>();

    for (((a_chunk, b_chunk), c_chunk), d_chunk) in a4
        .iter_mut()
        .zip(b4.iter_mut())
        .zip(c4.iter_mut())
        .zip(d4.iter_mut())
    {
        step(
            &mut a_chunk[0],
            &mut b_chunk[0],
            &mut c_chunk[0],
            &mut d_chunk[0],
            lut01,
            lut23,
            lut02,
        );
        step(
            &mut a_chunk[1],
            &mut b_chunk[1],
            &mut c_chunk[1],
            &mut d_chunk[1],
            lut01,
            lut23,
            lut02,
        );
        step(
            &mut a_chunk[2],
            &mut b_chunk[2],
            &mut c_chunk[2],
            &mut d_chunk[2],
            lut01,
            lut23,
            lut02,
        );
        step(
            &mut a_chunk[3],
            &mut b_chunk[3],
            &mut c_chunk[3],
            &mut d_chunk[3],
            lut01,
            lut23,
            lut02,
        );
    }

    for (((a_byte, b_byte), c_byte), d_byte) in a_tail
        .iter_mut()
        .zip(b_tail.iter_mut())
        .zip(c_tail.iter_mut())
        .zip(d_tail.iter_mut())
    {
        step(a_byte, b_byte, c_byte, d_byte, lut01, lut23, lut02);
    }
}

pub(super) fn get_pair_mut<T>(slice: &mut [T], i: usize, j: usize) -> Option<(&mut T, &mut T)> {
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
