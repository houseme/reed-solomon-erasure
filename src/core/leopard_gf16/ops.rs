use super::{BITWIDTH16, LeopardGf16Tables, MODULUS16, ORDER16};

/// GF(2^16) multiply using log/exp tables.
///
/// `log_b` is the log of the second operand (used when one operand's log is precomputed).
#[inline]
pub(super) fn mul_log16(a: u16, log_b: u16, log_lut: &[u16; ORDER16], exp_lut: &[u16; ORDER16 * 2]) -> u16 {
    if a == 0 {
        return 0;
    }
    exp_lut[add_mod16(log_lut[a as usize], log_b) as usize]
}

/// GF(2^16) multiply: `a * b` using log/exp tables.
#[inline]
pub(super) fn gf16_mul(a: u16, b: u16, log_lut: &[u16; ORDER16], exp_lut: &[u16; ORDER16 * 2]) -> u16 {
    if a == 0 || b == 0 {
        return 0;
    }
    exp_lut[add_mod16(log_lut[a as usize], log_lut[b as usize]) as usize]
}

/// Modular addition in GF(2^16) log domain: `(a + b) % 65535`.
#[inline]
pub(super) fn add_mod16(a: u16, b: u16) -> u16 {
    let sum = a as u32 + b as u32;
    (sum + (sum >> BITWIDTH16)) as u16
}

/// Modular subtraction in GF(2^16) log domain: `(a - b) % 65535`.
#[inline]
pub(super) fn sub_mod16(a: u16, b: u16) -> u16 {
    let dif = (a as u32).wrapping_sub(b as u32);
    (dif.wrapping_add(dif >> BITWIDTH16)) as u16
}

/// Multiply each element of `input` by `g^log_m` in GF(2^16), writing to `out`.
#[inline]
pub(super) fn mulgf16(out: &mut [u16], input: &[u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(input.len(), out.len());
    if log_m == MODULUS16 as u16 {
        // g^MODULUS16 = 1, so output = input.
        out.copy_from_slice(input);
    } else {
        for (dst, &src) in out.iter_mut().zip(input.iter()) {
            *dst = mul_log16(src, log_m, &tables.log_lut, &tables.exp_lut);
        }
    }
}

/// Multiply each element of `input` by `g^log_m` and XOR into `out`.
#[inline]
pub(super) fn mulgf16_xor(out: &mut [u16], input: &[u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(input.len(), out.len());
    if log_m == MODULUS16 as u16 {
        slice_xor_u16(input, out);
    } else {
        for (dst, &src) in out.iter_mut().zip(input.iter()) {
            *dst ^= mul_log16(src, log_m, &tables.log_lut, &tables.exp_lut);
        }
    }
}

/// XOR two u16 slices: `out[i] ^= input[i]`.
#[inline]
pub(super) fn slice_xor_u16(input: &[u16], out: &mut [u16]) {
    debug_assert_eq!(input.len(), out.len());
    // Process 32 bytes (16 u16s) at a time via u64 blocks.
    let (in16, in_tail) = input.as_chunks::<16>();
    let (out16, out_tail) = out.as_chunks_mut::<16>();

    for (src, dst) in in16.iter().zip(out16.iter_mut()) {
        for i in 0..8 {
            let off = i * 2;
            let s = unsafe { core::ptr::read_unaligned(src[off..].as_ptr().cast::<u64>()) };
            let d = unsafe { core::ptr::read_unaligned(dst[off..].as_ptr().cast::<u64>()) };
            unsafe {
                core::ptr::write_unaligned(dst[off..].as_mut_ptr().cast::<u64>(), d ^ s);
            }
        }
    }

    // Scalar tail.
    for (src, dst) in in_tail.iter().zip(out_tail.iter_mut()) {
        *dst ^= *src;
    }
}

/// FWHT (Fast Walsh-Hadamard Transform) for GF(2^16) log-domain values.
///
/// Same structure as `fwht8` but with u16 elements and mod-65535 arithmetic.
pub(super) fn fwht16(data: &mut [u16; ORDER16]) {
    let mut dist = 1usize;
    let mut dist4 = 4usize;
    while dist4 <= ORDER16 {
        let mut r = 0usize;
        while r < ORDER16 {
            let mut off = r;
            for _ in 0..dist {
                let t0 = data[off];
                let t1 = data[off + dist];
                let t2 = data[off + dist * 2];
                let t3 = data[off + dist * 3];

                let (t0, t1) = fwht2_alt16(t0, t1);
                let (t2, t3) = fwht2_alt16(t2, t3);
                let (t0, t2) = fwht2_alt16(t0, t2);
                let (t1, t3) = fwht2_alt16(t1, t3);

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

/// FWHT with mtrunc: inner loop limited to mtrunc.
pub(super) fn fwht16_mtrunc(data: &mut [u16], mtrunc: usize) {
    debug_assert_eq!(data.len(), ORDER16);
    let mut dist = 1usize;
    let mut dist4 = 4usize;
    while dist4 <= ORDER16 {
        let mut r = 0usize;
        while r < mtrunc {
            let mut off = r;
            for _ in 0..dist {
                let t0 = data[off];
                let t1 = data[off + dist];
                let t2 = data[off + dist * 2];
                let t3 = data[off + dist * 3];

                let (t0, t1) = fwht2_alt16(t0, t1);
                let (t2, t3) = fwht2_alt16(t2, t3);
                let (t0, t2) = fwht2_alt16(t0, t2);
                let (t1, t3) = fwht2_alt16(t1, t3);

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

/// Flexible-size FWHT for slices whose length is a power of 2 and <= ORDER16.
pub(super) fn fwht16_variable(data: &mut [u16]) {
    let n = data.len();
    debug_assert!(n.is_power_of_two());
    debug_assert!(n <= ORDER16);

    let mut dist = 1usize;
    while dist < n {
        let dist4 = dist * 4;
        if dist4 <= n {
            let mut r = 0usize;
            while r < n {
                let mut off = r;
                for _ in 0..dist {
                    let t0 = data[off];
                    let t1 = data[off + dist];
                    let t2 = data[off + dist * 2];
                    let t3 = data[off + dist * 3];

                    let (t0, t1) = fwht2_alt16(t0, t1);
                    let (t2, t3) = fwht2_alt16(t2, t3);
                    let (t0, t2) = fwht2_alt16(t0, t2);
                    let (t1, t3) = fwht2_alt16(t1, t3);

                    data[off] = t0;
                    data[off + dist] = t1;
                    data[off + dist * 2] = t2;
                    data[off + dist * 3] = t3;
                    off += 1;
                }
                r += dist4;
            }
            dist = dist4;
        } else {
            let dist2 = dist * 2;
            if dist2 <= n {
                let mut r = 0usize;
                while r < n {
                    let mut off = r;
                    for _ in 0..dist {
                        let t0 = data[off];
                        let t1 = data[off + dist];
                        let (t0, t1) = fwht2_alt16(t0, t1);
                        data[off] = t0;
                        data[off + dist] = t1;
                        off += 1;
                    }
                    r += dist2;
                }
            }
            break;
        }
    }
}

#[inline]
fn fwht2_alt16(a: u16, b: u16) -> (u16, u16) {
    (add_mod16(a, b), sub_mod16(a, b))
}

/// Forward butterfly step (FFT): `dst ^= mul(src, g^log_m); src ^= dst`.
#[inline]
pub(super) fn dit2_step16(dst: &mut [u16], src: &mut [u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(dst.len(), src.len());
    if log_m == MODULUS16 as u16 {
        slice_xor_u16(dst, src);
    } else {
        mulgf16_xor(dst, src, log_m, tables);
        slice_xor_u16(dst, src);
    }
}

/// Inverse butterfly step (IFFT): `src ^= dst; dst ^= mul(src, g^log_m)`.
#[inline]
pub(super) fn dit2_step_inv16(dst: &mut [u16], src: &mut [u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(dst.len(), src.len());
    if log_m == MODULUS16 as u16 {
        slice_xor_u16(dst, src);
    } else {
        slice_xor_u16(dst, src);
        mulgf16_xor(dst, src, log_m, tables);
    }
}

/// Forward radix-2 FFT butterfly.
pub(super) fn fft_dit2_16(x: &mut [u16], y: &mut [u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(x.len(), y.len());
    // Go fftDIT2: x ^= mul(y, g^log_m); y ^= x
    if log_m == MODULUS16 as u16 {
        slice_xor_u16(x, y);
    } else {
        mulgf16_xor(x, y, log_m, tables);
        slice_xor_u16(x, y);
    }
}

/// Inverse radix-2 IFFT butterfly.
pub(super) fn ifft_dit2_16(x: &mut [u16], y: &mut [u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(x.len(), y.len());
    // Go ifftDIT2: y ^= x; x ^= mul(y, g^log_m)
    if log_m == MODULUS16 as u16 {
        slice_xor_u16(x, y);
    } else {
        slice_xor_u16(x, y);
        mulgf16_xor(x, y, log_m, tables);
    }
}

/// Forward radix-4 butterfly.
#[inline(always)]
pub(super) fn fft_dit4_16(
    a: &mut [u16],
    b: &mut [u16],
    c: &mut [u16],
    d: &mut [u16],
    log_m01: u16,
    log_m23: u16,
    log_m02: u16,
    tables: &LeopardGf16Tables,
) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), c.len());
    debug_assert_eq!(a.len(), d.len());

    dit2_step16(a, c, log_m02, tables);
    dit2_step16(b, d, log_m02, tables);
    dit2_step16(a, b, log_m01, tables);
    dit2_step16(c, d, log_m23, tables);
}

/// Inverse radix-4 butterfly.
#[inline(always)]
pub(super) fn ifft_dit4_16(
    a: &mut [u16],
    b: &mut [u16],
    c: &mut [u16],
    d: &mut [u16],
    log_m01: u16,
    log_m23: u16,
    log_m02: u16,
    tables: &LeopardGf16Tables,
) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), c.len());
    debug_assert_eq!(a.len(), d.len());

    dit2_step_inv16(a, b, log_m01, tables);
    dit2_step_inv16(c, d, log_m23, tables);
    dit2_step_inv16(b, d, log_m02, tables);
    dit2_step_inv16(a, c, log_m02, tables);
}

/// Helper to get two mutable references from a slice at indices i and j.
pub(super) fn get_pair_mut_16<T>(slice: &mut [T], i: usize, j: usize) -> Option<(&mut T, &mut T)> {
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
