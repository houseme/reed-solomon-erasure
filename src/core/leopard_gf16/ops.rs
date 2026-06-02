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
    let result = (sum + (sum >> BITWIDTH16)) as u16;
    // When sum == 65535 exactly, the carry-free path yields 65535 instead of 0.
    if result == MODULUS16 as u16 { 0 } else { result }
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

/// Multiply each element of `input` by `g^log_m` and XOR into `out` (first arg).
/// Matches GF8 `lut_xor(dst, src, lut)` convention: first arg is modified.
#[inline]
pub(super) fn mulgf16_xor(out: &mut [u16], input: &[u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(input.len(), out.len());
    if log_m == MODULUS16 as u16 {
        slice_xor_u16(out, input);
    } else {
        for (dst, &src) in out.iter_mut().zip(input.iter()) {
            *dst ^= mul_log16(src, log_m, &tables.log_lut, &tables.exp_lut);
        }
    }
}

/// XOR two u16 slices: `dst[i] ^= src[i]`.
/// Matches GF8 `slice_xor(dst, src)` convention: first arg is modified.
#[inline]
pub(super) fn slice_xor_u16(dst: &mut [u16], src: &[u16]) {
    debug_assert_eq!(dst.len(), src.len());
    for (d, &s) in dst.iter_mut().zip(src.iter()) {
        *d ^= s;
    }
}

/// FWHT (Fast Walsh-Hadamard Transform) for GF(2^16) log-domain values.
///
/// Same structure as Go's `fwht`: sequential radix-2 butterflies within each block.
pub(super) fn fwht16(data: &mut [u16; ORDER16]) {
    let mut dist = 1usize;
    let mut dist4 = 4usize;
    while dist4 <= ORDER16 {
        let mut r = 0usize;
        while r < ORDER16 {
            let mut off = r;
            for _ in 0..dist {
                let (t0, t1) = fwht2_alt16(data[off], data[off + dist]);
                data[off] = t0;
                data[off + dist] = t1;
                let (t2, t3) = fwht2_alt16(data[off + dist * 2], data[off + dist * 3]);
                data[off + dist * 2] = t2;
                data[off + dist * 3] = t3;
                let (t0, t2) = fwht2_alt16(data[off], data[off + dist * 2]);
                data[off] = t0;
                data[off + dist * 2] = t2;
                let (t1, t3) = fwht2_alt16(data[off + dist], data[off + dist * 3]);
                data[off + dist] = t1;
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
///
/// Matches Go's `fwht(data, mtrunc)` — sequential radix-2 butterflies within each block.
pub(super) fn fwht16_mtrunc(data: &mut [u16], mtrunc: usize) {
    debug_assert_eq!(data.len(), ORDER16);
    let mut dist = 1usize;
    let mut dist4 = 4usize;
    while dist4 <= ORDER16 {
        let mut r = 0usize;
        while r < mtrunc {
            let mut off = r;
            for _ in 0..dist {
                let (t0, t1) = fwht2_alt16(data[off], data[off + dist]);
                data[off] = t0;
                data[off + dist] = t1;
                let (t2, t3) = fwht2_alt16(data[off + dist * 2], data[off + dist * 3]);
                data[off + dist * 2] = t2;
                data[off + dist * 3] = t3;
                let (t0, t2) = fwht2_alt16(data[off], data[off + dist * 2]);
                data[off] = t0;
                data[off + dist * 2] = t2;
                let (t1, t3) = fwht2_alt16(data[off + dist], data[off + dist * 3]);
                data[off + dist] = t1;
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
///
/// Matches Go's `fwht(data, len)` — sequential radix-2 butterflies within each block.
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
                    let (t0, t1) = fwht2_alt16(data[off], data[off + dist]);
                    data[off] = t0;
                    data[off + dist] = t1;
                    let (t2, t3) = fwht2_alt16(data[off + dist * 2], data[off + dist * 3]);
                    data[off + dist * 2] = t2;
                    data[off + dist * 3] = t3;
                    let (t0, t2) = fwht2_alt16(data[off], data[off + dist * 2]);
                    data[off] = t0;
                    data[off + dist * 2] = t2;
                    let (t1, t3) = fwht2_alt16(data[off + dist], data[off + dist * 3]);
                    data[off + dist] = t1;
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
                        let (t0, t1) = fwht2_alt16(data[off], data[off + dist]);
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

#[cfg(test)]
#[inline]
pub(super) fn fwht2_alt16_test(a: u16, b: u16) -> (u16, u16) {
    fwht2_alt16(a, b)
}

/// Forward butterfly step (FFT): `dst ^= mul(src, g^log_m); src ^= dst`.
/// Matches GF8 `dit2_step(dst, src)`.
#[inline]
pub(super) fn dit2_step16(dst: &mut [u16], src: &mut [u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(dst.len(), src.len());
    if log_m == MODULUS16 as u16 {
        slice_xor_u16(dst, src);
    } else {
        mulgf16_xor(dst, src, log_m, tables);
        slice_xor_u16(src, dst);
    }
}

/// Inverse butterfly step (IFFT): `src ^= dst; dst ^= mul(src, g^log_m)`.
/// Matches GF8 `dit2_step_inv(dst, src)`.
///
/// Note: no MODULUS16 shortcut — the general path handles g^m=1 correctly
/// via mulgf16_xor's own shortcut, but the operation order matters for the inverse.
#[inline]
pub(super) fn dit2_step_inv16(dst: &mut [u16], src: &mut [u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(dst.len(), src.len());
    slice_xor_u16(src, dst);
    mulgf16_xor(dst, src, log_m, tables);
}

/// Forward radix-2 FFT butterfly: `x ^= mul(y, m); y ^= x`.
/// Matches GF8 `fft_dit2_lut(x, y)`.
pub(super) fn fft_dit2_16(x: &mut [u16], y: &mut [u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(x.len(), y.len());
    if log_m == MODULUS16 as u16 {
        slice_xor_u16(x, y);
    } else {
        mulgf16_xor(x, y, log_m, tables);
        slice_xor_u16(y, x);
    }
}

/// Inverse radix-2 IFFT butterfly: `y ^= x; x ^= mul(y, m)`.
/// Matches GF8 `ifft_dit2_lut(x, y)`.
///
/// Note: no MODULUS16 shortcut — the general path handles g^m=1 correctly.
pub(super) fn ifft_dit2_16(x: &mut [u16], y: &mut [u16], log_m: u16, tables: &LeopardGf16Tables) {
    debug_assert_eq!(x.len(), y.len());
    slice_xor_u16(y, x);
    mulgf16_xor(x, y, log_m, tables);
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
