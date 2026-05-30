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
    debug_assert_eq!(input.len(), out.len());

    // Process 64 bytes per iteration using u64 blocks.
    // The compiler auto-vectorizes u64 XOR to SIMD (NEON/AVX2) more reliably
    // than byte-level unrolled XOR.
    let (input64, input_tail64) = input.as_chunks::<64>();
    let (out64, out_tail64) = out.as_chunks_mut::<64>();

    for (src, dst) in input64.iter().zip(out64.iter_mut()) {
        let src_u64: &[u64; 8] = unsafe { &*(src.as_ptr() as *const [u64; 8]) };
        let dst_u64: &mut [u64; 8] = unsafe { &mut *(dst.as_mut_ptr() as *mut [u64; 8]) };
        dst_u64[0] ^= src_u64[0];
        dst_u64[1] ^= src_u64[1];
        dst_u64[2] ^= src_u64[2];
        dst_u64[3] ^= src_u64[3];
        dst_u64[4] ^= src_u64[4];
        dst_u64[5] ^= src_u64[5];
        dst_u64[6] ^= src_u64[6];
        dst_u64[7] ^= src_u64[7];
    }

    // Process remaining bytes in 8-byte chunks.
    let (input8, input_tail) = input_tail64.as_chunks::<8>();
    let (out8, out_tail) = out_tail64.as_chunks_mut::<8>();

    for (src, dst) in input8.iter().zip(out8.iter_mut()) {
        let s = u64::from_ne_bytes(*src);
        let d = u64::from_ne_bytes(*dst);
        *dst = (d ^ s).to_ne_bytes();
    }

    // Scalar tail.
    for (src, dst) in input_tail.iter().zip(out_tail.iter_mut()) {
        *dst ^= *src;
    }
}

pub(super) fn slices_xor(input: &[Vec<u8>], out: &mut [Vec<u8>]) {
    debug_assert_eq!(input.len(), out.len());
    for (src, dst) in input.iter().zip(out.iter_mut()) {
        slice_xor(src, dst);
    }
}

pub(super) fn mul_slice_xor_reference(c: u8, input: &[u8], out: &mut [u8]) {
    let tables = init_leopard_gf8_tables();
    let lut = &tables.mul_luts[c as usize];
    debug_assert_eq!(input.len(), out.len());
    for (value, slot) in input.iter().zip(out.iter_mut()) {
        *slot ^= lut.value[*value as usize];
    }
}

pub(super) fn mulgf8(out: &mut [u8], input: &[u8], log_m: u8, tables: &LeopardGf8Tables) {
    let lut = &tables.mul_luts[log_m as usize];
    debug_assert_eq!(input.len(), out.len());
    for (src, dst) in input.iter().zip(out.iter_mut()) {
        *dst = lut.value[*src as usize];
    }
}

pub(super) fn fft_dit2(x: &mut [u8], y: &mut [u8], log_m: u8, tables: &LeopardGf8Tables) {
    fft_dit2_lut(x, y, log_m, &tables.mul_luts[log_m as usize].value);
}

pub(super) fn fft_dit2_lut(x: &mut [u8], y: &mut [u8], log_m: u8, lut: &[u8; 256]) {
    if log_m == MODULUS8 as u8 {
        slice_xor(x, y);
    } else {
        debug_assert_eq!(x.len(), y.len());
        for (dst, src) in x.iter_mut().zip(y.iter()) {
            *dst ^= lut[*src as usize];
        }
    }
}

pub(super) fn ifft_dit2(x: &mut [u8], y: &mut [u8], log_m: u8, tables: &LeopardGf8Tables) {
    ifft_dit2_lut(x, y, log_m, &tables.mul_luts[log_m as usize].value);
}

pub(super) fn ifft_dit2_lut(x: &mut [u8], y: &mut [u8], log_m: u8, lut: &[u8; 256]) {
    if log_m == MODULUS8 as u8 {
        slice_xor(x, y);
    } else {
        debug_assert_eq!(x.len(), y.len());
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
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), c.len());
    debug_assert_eq!(a.len(), d.len());

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
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), c.len());
    debug_assert_eq!(a.len(), d.len());

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
