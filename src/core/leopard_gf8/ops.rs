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

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: runtime feature detection confirmed AVX2 is available.
            unsafe {
                slice_xor_avx2(input, out);
            }
            return;
        }
    }

    slice_xor_u64(input, out);
}

/// u64-block XOR fallback (also used on aarch64 and non-AVX2 x86_64).
fn slice_xor_u64(input: &[u8], out: &mut [u8]) {
    // Process 64 bytes per iteration using u64 blocks.
    // Uses unaligned reads to avoid UB on sub-slice pointers.
    let (input64, input_tail64) = input.as_chunks::<64>();
    let (out64, out_tail64) = out.as_chunks_mut::<64>();

    for (src, dst) in input64.iter().zip(out64.iter_mut()) {
        for i in 0..8 {
            let off = i * 8;
            // SAFETY: 8 bytes guaranteed by as_chunks::<64>().
            let s = unsafe { core::ptr::read_unaligned(src[off..].as_ptr().cast::<u64>()) };
            let d = unsafe { core::ptr::read_unaligned(dst[off..].as_ptr().cast::<u64>()) };
            unsafe {
                core::ptr::write_unaligned(dst[off..].as_mut_ptr().cast::<u64>(), d ^ s);
            }
        }
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

/// AVX2 SIMD XOR: 32 bytes per iteration using `_mm256_xor_si256`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn slice_xor_avx2(input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{_mm256_loadu_si256, _mm256_storeu_si256, _mm256_xor_si256};

    let (in32, in_tail) = input.as_chunks::<32>();
    let (out32, out_tail) = out.as_chunks_mut::<32>();

    for (src, dst) in in32.iter().zip(out32.iter_mut()) {
        // SAFETY: chunks_exact(32) guarantees 32 valid bytes for load/store.
        let s = unsafe { _mm256_loadu_si256(src.as_ptr().cast()) };
        let d = unsafe { _mm256_loadu_si256(dst.as_ptr().cast()) };
        unsafe { _mm256_storeu_si256(dst.as_mut_ptr().cast(), _mm256_xor_si256(d, s)) };
    }

    // Scalar tail (0-31 bytes).
    for (src, dst) in in_tail.iter().zip(out_tail.iter_mut()) {
        *dst ^= *src;
    }
}

pub(super) fn slices_xor(input: &[Vec<u8>], out: &mut [Vec<u8>]) {
    debug_assert_eq!(input.len(), out.len());
    for (src, dst) in input.iter().zip(out.iter_mut()) {
        slice_xor(src, dst);
    }
}

/// SIMD-accelerated LUT-XOR: `dst[i] ^= lut[src[i]]` using nibble-lookup.
///
/// Uses AVX2 on x86_64 (32 bytes/iteration), scalar fallback otherwise.
/// Skips SIMD for small slices (< 32 bytes) where overhead exceeds benefit.
#[inline]
fn lut_xor(dst: &mut [u8], src: &[u8], lut: &[u8; 256]) {
    debug_assert_eq!(dst.len(), src.len());

    #[cfg(target_arch = "x86_64")]
    {
        if dst.len() >= 32 && is_x86_feature_detected!("avx2") {
            // SAFETY: runtime feature detection confirmed AVX2 is available, len >= 32.
            unsafe {
                lut_xor_avx2(dst, src, lut);
            }
            return;
        }
    }

    // Scalar fallback.
    for (d, s) in dst.iter_mut().zip(src.iter()) {
        *d ^= lut[*s as usize];
    }
}

/// AVX2 nibble-lookup: `dst[i] ^= lut[src[i]]`, 32 bytes per iteration.
///
/// Decomposes each byte into low/high nibbles, looks up in 16-byte tables,
/// and XORs the results. Same algorithm as galois_8 AVX2 mul_slice_xor.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn lut_xor_avx2(dst: &mut [u8], src: &[u8], lut: &[u8; 256]) {
    use core::arch::x86_64::{
        __m256i, _mm256_and_si256, _mm256_loadu_si256, _mm256_set1_epi8, _mm256_shuffle_epi8,
        _mm256_srli_epi64, _mm256_storeu_si256, _mm256_xor_si256,
    };

    // Build 16-byte nibble tables: lut_low[i] = lut[i], lut_high[i] = lut[i*16].
    let mut lut_low = [0u8; 16];
    let mut lut_high = [0u8; 16];
    lut_low.copy_from_slice(&lut[..16]);
    for i in 0..16 {
        lut_high[i] = lut[i * 16];
    }

    // Broadcast 16-byte tables to both 128-bit lanes of 256-bit registers.
    let low_tbl: __m256i = {
        use core::arch::x86_64::{__m128i, _mm_loadu_si128, _mm256_broadcastsi128_si256};
        // SAFETY: lut_low is a valid 16-byte array.
        let lo128: __m128i = unsafe { _mm_loadu_si128(lut_low.as_ptr().cast()) };
        _mm256_broadcastsi128_si256(lo128)
    };
    let high_tbl: __m256i = {
        use core::arch::x86_64::{__m128i, _mm_loadu_si128, _mm256_broadcastsi128_si256};
        // SAFETY: lut_high is a valid 16-byte array.
        let hi128: __m128i = unsafe { _mm_loadu_si128(lut_high.as_ptr().cast()) };
        _mm256_broadcastsi128_si256(hi128)
    };
    let nibble_mask: __m256i = _mm256_set1_epi8(0x0f);

    let (src32, src_tail) = src.as_chunks::<32>();
    let (dst32, dst_tail) = dst.as_chunks_mut::<32>();

    for (s_chunk, d_chunk) in src32.iter().zip(dst32.iter_mut()) {
        // SAFETY: chunks_exact(32) guarantees 32 valid bytes for load/store.
        let sv = unsafe { _mm256_loadu_si256(s_chunk.as_ptr().cast()) };
        let dv = unsafe { _mm256_loadu_si256(d_chunk.as_ptr().cast()) };
        let lo = _mm256_and_si256(sv, nibble_mask);
        let hi = _mm256_and_si256(_mm256_srli_epi64::<4>(sv), nibble_mask);
        let product = _mm256_xor_si256(
            _mm256_shuffle_epi8(low_tbl, lo),
            _mm256_shuffle_epi8(high_tbl, hi),
        );
        unsafe {
            _mm256_storeu_si256(d_chunk.as_mut_ptr().cast(), _mm256_xor_si256(dv, product));
        }
    }

    // Scalar tail (0-31 bytes).
    for (d, s) in dst_tail.iter_mut().zip(src_tail.iter()) {
        *d ^= lut[*s as usize];
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
        lut_xor(x, y, lut);
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
        lut_xor(y, x, lut);
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

    // Forward step per byte:
    //   b1 = b[i] ^ lut02[d[i]];  a_f = a[i] ^ lut02[c[i]] ^ lut01[b1];
    //   c_f = c[i] ^ lut23[d[i]];
    //   a=a_f, b=b1, c=c_f, d unchanged
    //
    // In-place order (1 allocation):
    //   1. b  ^= lut02[d]        (b is now b1, reads d unchanged)
    //   2. save a                (1 alloc — needed because a_f reads original a)
    //   3. a   = a_saved ^ lut02[c] ^ lut01[b]  (reads original c, b=b1)
    //   4. c  ^= lut23[d]        (c is now c_f, reads d unchanged)

    // Step 1: b1 in-place.
    lut_xor(b, d, lut02);

    // Step 2: save a before overwriting.
    let a_saved = a.to_vec();

    // Step 3: a_f = a_saved ^ lut02[c] ^ lut01[b1].
    a.copy_from_slice(&a_saved);
    lut_xor(a, c, lut02);
    lut_xor(a, b, lut01);

    // Step 4: c_f in-place.
    lut_xor(c, d, lut23);
    // d unchanged.
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

    // Inverse step per byte:
    //   b1 = b[i] ^ lut01[a[i]];  c_f = c[i] ^ lut02[a[i]];
    //   d_f = d[i] ^ lut23[c[i]] ^ lut02[b1];
    //   a unchanged, b=b1, c=c_f, d=d_f
    //
    // In-place order (1 allocation):
    //   1. b  ^= lut01[a]         (b is now b1, a unchanged)
    //   2. save d                 (1 alloc — d_f needs original c)
    //   3. d_f = d_saved ^ lut23[c] ^ lut02[b1]  (reads original c, b=b1)
    //   4. c  ^= lut02[a]         (c is now c_f, a unchanged)

    // Step 1: b1 in-place.
    lut_xor(b, a, lut01);

    // Step 2: save d before c is overwritten (d_f needs original c).
    let d_saved = d.to_vec();

    // Step 3: d_f = d_saved ^ lut23[c] ^ lut02[b1].
    d.copy_from_slice(&d_saved);
    lut_xor(d, c, lut23);
    lut_xor(d, b, lut02);

    // Step 4: c_f in-place.
    lut_xor(c, a, lut02);
    // a unchanged.
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
