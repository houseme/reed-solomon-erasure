//! SIMD-accelerated Fast Walsh–Hadamard Transform butterflies for the GF(2^16)
//! Leopard error-locator (rustfs/backlog#1248, Phase A).
//!
//! The decode error locator runs a full-length (65536-element) FWHT whose
//! radix-2 butterfly is `(a, b) -> (add_mod16(a, b), sub_mod16(a, b))` in the
//! log domain (modular add/sub with an end-around carry, modulus 65535). That is
//! the fixed per-call cost that dominates *cold* (first-seen erasure pattern)
//! reconstructs — the memoised path (#19) already covers repeats.
//!
//! The two hot inner loops of [`super::ops::fwht16_variable`] apply the same
//! butterfly to `dist` consecutive positions at a fixed stride. When `dist` is at
//! least the SIMD width, those positions are independent, so we process a whole
//! vector at once. Modular add/sub vectorise cleanly:
//!
//! * `add_mod16(a,b)`: `s = a +w b`; if it overflowed (`s <u a`) add 1 — i.e.
//!   `s - mask` with `mask = (s <u a) ? 0xFFFF : 0`.
//! * `sub_mod16(a,b)`: `d = a -w b`; if it borrowed (`a <u b`) subtract 1 — i.e.
//!   `d + mask` with `mask = (a <u b) ? 0xFFFF : 0`.
//!
//! Unsigned 16-bit `<` has no direct SSE/AVX intrinsic, so it is done with the
//! `x ^ 0x8000` bias + signed `cmpgt`. NEON has `vcltq_u16` directly. `dist < W`
//! stages and non-x86/aarch64 (or big-endian) targets use the scalar path.

use super::ops::fwht2_alt16;

/// Radix-4 FWHT block: apply the 4-way butterfly network to every `off` in
/// `[r, r + dist)`, at strides `dist`, `2*dist`, `3*dist`. Mirrors the scalar
/// inner loop of `fwht16_variable`; SIMD when `dist >= WIDTH`.
#[inline]
pub(super) fn radix4_block(data: &mut [u16], r: usize, dist: usize) {
    #[cfg(all(target_arch = "x86_64", target_endian = "little"))]
    {
        // AVX2 is chosen by runtime detection, which is `std`-only; under
        // `no_std` this arm is compiled out and we fall through to the SSE2
        // baseline path below (still SIMD, just 128-bit).
        #[cfg(feature = "std")]
        if dist >= 16 && std::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 confirmed; `dist >= 16` makes the four stride-`dist`
            // 16-lane windows disjoint and in bounds within `[r, r+4*dist)`.
            unsafe {
                x86::radix4_avx2(data, r, dist);
            }
            return;
        }
        if dist >= 8 {
            // SAFETY: SSE2 is x86_64 baseline; `dist >= 8` makes the four
            // stride-`dist` 8-lane windows disjoint and in bounds.
            unsafe {
                x86::radix4_sse2(data, r, dist);
            }
            return;
        }
    }
    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    if dist >= 8 {
        // SAFETY: NEON is baseline on aarch64; `dist >= 8` makes the four
        // stride-`dist` 8-lane windows disjoint and in bounds.
        unsafe {
            aarch64::radix4_neon(data, r, dist);
        }
        return;
    }
    radix4_scalar(data, r, dist);
}

/// Radix-2 FWHT block: butterfly every `off` in `[r, r + dist)` at stride `dist`.
#[inline]
pub(super) fn radix2_block(data: &mut [u16], r: usize, dist: usize) {
    #[cfg(all(target_arch = "x86_64", target_endian = "little"))]
    {
        // `std`-only runtime AVX2 detection; see `radix4_block`. `no_std` falls
        // through to the SSE2 baseline path below.
        #[cfg(feature = "std")]
        if dist >= 16 && std::is_x86_feature_detected!("avx2") {
            // SAFETY: see `radix4_block`; two stride-`dist` 16-lane windows.
            unsafe {
                x86::radix2_avx2(data, r, dist);
            }
            return;
        }
        if dist >= 8 {
            // SAFETY: see `radix4_block`; two stride-`dist` 8-lane windows.
            unsafe {
                x86::radix2_sse2(data, r, dist);
            }
            return;
        }
    }
    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    if dist >= 8 {
        // SAFETY: see `radix4_block`; two stride-`dist` 8-lane windows.
        unsafe {
            aarch64::radix2_neon(data, r, dist);
        }
        return;
    }
    radix2_scalar(data, r, dist);
}

fn radix4_scalar(data: &mut [u16], r: usize, dist: usize) {
    for off in (r..).take(dist) {
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
    }
}

fn radix2_scalar(data: &mut [u16], r: usize, dist: usize) {
    for off in (r..).take(dist) {
        let (t0, t1) = fwht2_alt16(data[off], data[off + dist]);
        data[off] = t0;
        data[off + dist] = t1;
    }
}

// ---------------------------------------------------------------------- x86 --

#[cfg(all(target_arch = "x86_64", target_endian = "little"))]
mod x86 {
    use core::arch::x86_64::*;

    // 256-bit AVX2 (16 u16/lane) modular add/sub. AVX2 is dispatched via
    // runtime detection, which is `std`-only, so the whole AVX2 path is gated on
    // `feature = "std"` to avoid dead code (and a build break) under `no_std`.
    #[cfg(feature = "std")]
    #[target_feature(enable = "avx2")]
    unsafe fn add_mod_avx2(a: __m256i, b: __m256i, bias: __m256i) -> __m256i {
        // Pure register arithmetic; the AVX2 intrinsics are safe under the
        // enabled target feature.
        let s = _mm256_add_epi16(a, b);
        // mask = (s <u a) ? 0xFFFF : 0  ==  (a >u s)
        let mask = _mm256_cmpgt_epi16(_mm256_xor_si256(a, bias), _mm256_xor_si256(s, bias));
        _mm256_sub_epi16(s, mask)
    }
    #[cfg(feature = "std")]
    #[target_feature(enable = "avx2")]
    unsafe fn sub_mod_avx2(a: __m256i, b: __m256i, bias: __m256i) -> __m256i {
        // Pure register arithmetic; safe under the enabled target feature.
        let d = _mm256_sub_epi16(a, b);
        // mask = (a <u b) ? 0xFFFF : 0  ==  (b >u a)
        let mask = _mm256_cmpgt_epi16(_mm256_xor_si256(b, bias), _mm256_xor_si256(a, bias));
        _mm256_add_epi16(d, mask)
    }

    #[cfg(feature = "std")]
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn radix4_avx2(data: &mut [u16], r: usize, dist: usize) {
        // SAFETY: caller guarantees `dist >= 16`, so the four windows at `off`,
        // `off+dist`, `off+2*dist`, `off+3*dist` (each 16 lanes) are disjoint and
        // lie within `[r, r+4*dist) <= data.len()`. `dist` is a power of four
        // (>=16), hence a multiple of 16, so the loop tiles exactly.
        unsafe {
            let bias = _mm256_set1_epi16(i16::MIN); // 0x8000
            let p = data.as_mut_ptr();
            let mut off = r;
            let end = r + dist;
            while off < end {
                let a0 = p.add(off).cast();
                let a1 = p.add(off + dist).cast();
                let a2 = p.add(off + dist * 2).cast();
                let a3 = p.add(off + dist * 3).cast();
                let v0 = _mm256_loadu_si256(a0);
                let v1 = _mm256_loadu_si256(a1);
                let v2 = _mm256_loadu_si256(a2);
                let v3 = _mm256_loadu_si256(a3);
                let t0 = add_mod_avx2(v0, v1, bias);
                let t1 = sub_mod_avx2(v0, v1, bias);
                let t2 = add_mod_avx2(v2, v3, bias);
                let t3 = sub_mod_avx2(v2, v3, bias);
                let u0 = add_mod_avx2(t0, t2, bias);
                let u2 = sub_mod_avx2(t0, t2, bias);
                let u1 = add_mod_avx2(t1, t3, bias);
                let u3 = sub_mod_avx2(t1, t3, bias);
                _mm256_storeu_si256(a0, u0);
                _mm256_storeu_si256(a1, u1);
                _mm256_storeu_si256(a2, u2);
                _mm256_storeu_si256(a3, u3);
                off += 16;
            }
        }
    }

    #[cfg(feature = "std")]
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn radix2_avx2(data: &mut [u16], r: usize, dist: usize) {
        // SAFETY: `dist >= 16` -> two disjoint 16-lane windows, in bounds.
        unsafe {
            let bias = _mm256_set1_epi16(-0x8000i16);
            let p = data.as_mut_ptr();
            let mut off = r;
            let end = r + dist;
            while off < end {
                let a0 = p.add(off).cast();
                let a1 = p.add(off + dist).cast();
                let v0 = _mm256_loadu_si256(a0);
                let v1 = _mm256_loadu_si256(a1);
                _mm256_storeu_si256(a0, add_mod_avx2(v0, v1, bias));
                _mm256_storeu_si256(a1, sub_mod_avx2(v0, v1, bias));
                off += 16;
            }
        }
    }

    // 128-bit SSE2 (8 u16/lane); SSE2 is x86_64 baseline.
    #[target_feature(enable = "sse2")]
    unsafe fn add_mod_sse2(a: __m128i, b: __m128i, bias: __m128i) -> __m128i {
        // Pure register arithmetic; safe under the enabled target feature.
        let s = _mm_add_epi16(a, b);
        let mask = _mm_cmpgt_epi16(_mm_xor_si128(a, bias), _mm_xor_si128(s, bias));
        _mm_sub_epi16(s, mask)
    }
    #[target_feature(enable = "sse2")]
    unsafe fn sub_mod_sse2(a: __m128i, b: __m128i, bias: __m128i) -> __m128i {
        // Pure register arithmetic; safe under the enabled target feature.
        let d = _mm_sub_epi16(a, b);
        let mask = _mm_cmpgt_epi16(_mm_xor_si128(b, bias), _mm_xor_si128(a, bias));
        _mm_add_epi16(d, mask)
    }

    #[target_feature(enable = "sse2")]
    pub(super) unsafe fn radix4_sse2(data: &mut [u16], r: usize, dist: usize) {
        // SAFETY: `dist >= 8` -> four disjoint 8-lane windows, in bounds.
        unsafe {
            let bias = _mm_set1_epi16(i16::MIN);
            let p = data.as_mut_ptr();
            let mut off = r;
            let end = r + dist;
            while off < end {
                let a0 = p.add(off).cast();
                let a1 = p.add(off + dist).cast();
                let a2 = p.add(off + dist * 2).cast();
                let a3 = p.add(off + dist * 3).cast();
                let v0 = _mm_loadu_si128(a0);
                let v1 = _mm_loadu_si128(a1);
                let v2 = _mm_loadu_si128(a2);
                let v3 = _mm_loadu_si128(a3);
                let t0 = add_mod_sse2(v0, v1, bias);
                let t1 = sub_mod_sse2(v0, v1, bias);
                let t2 = add_mod_sse2(v2, v3, bias);
                let t3 = sub_mod_sse2(v2, v3, bias);
                let u0 = add_mod_sse2(t0, t2, bias);
                let u2 = sub_mod_sse2(t0, t2, bias);
                let u1 = add_mod_sse2(t1, t3, bias);
                let u3 = sub_mod_sse2(t1, t3, bias);
                _mm_storeu_si128(a0, u0);
                _mm_storeu_si128(a1, u1);
                _mm_storeu_si128(a2, u2);
                _mm_storeu_si128(a3, u3);
                off += 8;
            }
        }
    }

    #[target_feature(enable = "sse2")]
    pub(super) unsafe fn radix2_sse2(data: &mut [u16], r: usize, dist: usize) {
        // SAFETY: `dist >= 8` -> two disjoint 8-lane windows, in bounds.
        unsafe {
            let bias = _mm_set1_epi16(i16::MIN);
            let p = data.as_mut_ptr();
            let mut off = r;
            let end = r + dist;
            while off < end {
                let a0 = p.add(off).cast();
                let a1 = p.add(off + dist).cast();
                let v0 = _mm_loadu_si128(a0);
                let v1 = _mm_loadu_si128(a1);
                _mm_storeu_si128(a0, add_mod_sse2(v0, v1, bias));
                _mm_storeu_si128(a1, sub_mod_sse2(v0, v1, bias));
                off += 8;
            }
        }
    }
}

// ------------------------------------------------------------------ aarch64 --

#[cfg(all(target_arch = "aarch64", target_endian = "little"))]
mod aarch64 {
    use core::arch::aarch64::*;

    // These only call baseline NEON intrinsics (safe on aarch64), so no `unsafe`
    // block is needed even though they are `#[target_feature]` fns.
    #[target_feature(enable = "neon")]
    unsafe fn add_mod_neon(a: uint16x8_t, b: uint16x8_t) -> uint16x8_t {
        let s = vaddq_u16(a, b);
        let mask = vcltq_u16(s, a); // 0xFFFF where overflow: s <u a
        vsubq_u16(s, mask) // s - 0xFFFF == s + 1 on overflow
    }
    #[target_feature(enable = "neon")]
    unsafe fn sub_mod_neon(a: uint16x8_t, b: uint16x8_t) -> uint16x8_t {
        let d = vsubq_u16(a, b);
        let mask = vcltq_u16(a, b); // 0xFFFF where borrow: a <u b
        vaddq_u16(d, mask) // d + 0xFFFF == d - 1 on borrow
    }

    #[target_feature(enable = "neon")]
    pub(super) unsafe fn radix4_neon(data: &mut [u16], r: usize, dist: usize) {
        // SAFETY: `dist >= 8` -> four disjoint 8-lane windows, in bounds.
        unsafe {
            let p = data.as_mut_ptr();
            let mut off = r;
            let end = r + dist;
            while off < end {
                let v0 = vld1q_u16(p.add(off));
                let v1 = vld1q_u16(p.add(off + dist));
                let v2 = vld1q_u16(p.add(off + dist * 2));
                let v3 = vld1q_u16(p.add(off + dist * 3));
                let t0 = add_mod_neon(v0, v1);
                let t1 = sub_mod_neon(v0, v1);
                let t2 = add_mod_neon(v2, v3);
                let t3 = sub_mod_neon(v2, v3);
                vst1q_u16(p.add(off), add_mod_neon(t0, t2));
                vst1q_u16(p.add(off + dist * 2), sub_mod_neon(t0, t2));
                vst1q_u16(p.add(off + dist), add_mod_neon(t1, t3));
                vst1q_u16(p.add(off + dist * 3), sub_mod_neon(t1, t3));
                off += 8;
            }
        }
    }

    #[target_feature(enable = "neon")]
    pub(super) unsafe fn radix2_neon(data: &mut [u16], r: usize, dist: usize) {
        // SAFETY: `dist >= 8` -> two disjoint 8-lane windows, in bounds.
        unsafe {
            let p = data.as_mut_ptr();
            let mut off = r;
            let end = r + dist;
            while off < end {
                let v0 = vld1q_u16(p.add(off));
                let v1 = vld1q_u16(p.add(off + dist));
                vst1q_u16(p.add(off), add_mod_neon(v0, v1));
                vst1q_u16(p.add(off + dist), sub_mod_neon(v0, v1));
                off += 8;
            }
        }
    }
}

#[cfg(test)]
mod fwht_simd_tests {
    extern crate alloc;
    use super::{radix2_scalar, radix4_scalar};
    use crate::core::leopard_gf16::ops::fwht16_variable;
    use alloc::vec::Vec;

    // Pure-scalar reference FWHT (same structure as `fwht16_variable`, but always
    // the scalar radix blocks — never the SIMD dispatch).
    fn fwht_scalar_ref(data: &mut [u16]) {
        let n = data.len();
        let mut dist = 1usize;
        while dist < n {
            let dist4 = dist * 4;
            if dist4 <= n {
                let mut r = 0;
                while r < n {
                    radix4_scalar(data, r, dist);
                    r += dist4;
                }
                dist = dist4;
            } else {
                let dist2 = dist * 2;
                if dist2 <= n {
                    let mut r = 0;
                    while r < n {
                        radix2_scalar(data, r, dist);
                        r += dist2;
                    }
                }
                break;
            }
        }
    }

    #[test]
    fn simd_fwht_matches_scalar() {
        // Covers radix-4 stages (n a power of 4) and the radix-2 remainder
        // (n = 2 * power of 4), and dist< / >=SIMD-width transitions, up to the
        // full 65536-element error-locator transform.
        for &len in &[2usize, 4, 8, 16, 32, 64, 128, 256, 1024, 4096, 8192, 65536] {
            let input: Vec<u16> = (0..len)
                .map(|i| (i.wrapping_mul(40503).wrapping_add(12345) & 0xFFFF) as u16)
                .collect();
            let mut simd = input.clone();
            fwht16_variable(&mut simd);
            let mut scalar = input;
            fwht_scalar_ref(&mut scalar);
            assert_eq!(simd, scalar, "FWHT SIMD != scalar for len={len}");
        }
    }
}
