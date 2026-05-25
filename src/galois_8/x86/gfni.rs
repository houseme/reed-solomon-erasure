#[cfg(test)]
extern crate alloc;

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const GFNI_ISOMORPHISM_ROWS: [u8; 8] = [0xff, 0xaa, 0xcc, 0x88, 0xf0, 0xa0, 0xc0, 0x80];

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[inline]
fn gfni_isomorphism_bytes() -> [u8; 16] {
    let word = [
        GFNI_ISOMORPHISM_ROWS[7],
        GFNI_ISOMORPHISM_ROWS[6],
        GFNI_ISOMORPHISM_ROWS[5],
        GFNI_ISOMORPHISM_ROWS[4],
        GFNI_ISOMORPHISM_ROWS[3],
        GFNI_ISOMORPHISM_ROWS[2],
        GFNI_ISOMORPHISM_ROWS[1],
        GFNI_ISOMORPHISM_ROWS[0],
    ];
    let mut bytes = [0u8; 16];
    // GF2P8AFFINE interprets each 64-bit word in byte-reversed row order.
    bytes[..8].copy_from_slice(&word);
    bytes[8..].copy_from_slice(&word);
    bytes
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[inline]
fn coeff_table_avx2(c: u8) -> [u8; 32] {
    [c; 32]
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[inline]
fn coeff_table_avx512(c: u8) -> [u8; 64] {
    [c; 64]
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[inline]
#[target_feature(enable = "gfni,avx2")]
unsafe fn gfni_avx2_constants(c: u8) -> (core::arch::x86_64::__m256i, core::arch::x86_64::__m256i) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_loadu_si128, _mm256_broadcastsi128_si256,
        _mm256_gf2p8affine_epi64_epi8, _mm256_loadu_si256,
    };

    let iso_bytes = gfni_isomorphism_bytes();
    let iso128: __m128i = unsafe { _mm_loadu_si128(iso_bytes.as_ptr().cast()) };
    let iso256: __m256i = _mm256_broadcastsi128_si256(iso128);
    let coeff_bytes = coeff_table_avx2(c);
    let coeff_vec: __m256i = unsafe { _mm256_loadu_si256(coeff_bytes.as_ptr().cast()) };
    let coeff_mapped = _mm256_gf2p8affine_epi64_epi8(coeff_vec, iso256, 0);

    (iso256, coeff_mapped)
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[inline]
#[target_feature(enable = "gfni,avx512f,avx512bw")]
unsafe fn gfni_avx512_constants(
    c: u8,
) -> (core::arch::x86_64::__m512i, core::arch::x86_64::__m512i) {
    use core::arch::x86_64::{
        __m128i, __m512i, _mm_loadu_si128, _mm512_broadcast_i32x4, _mm512_gf2p8affine_epi64_epi8,
        _mm512_loadu_si512,
    };

    let iso_bytes = gfni_isomorphism_bytes();
    let iso128: __m128i = unsafe { _mm_loadu_si128(iso_bytes.as_ptr().cast()) };
    let iso512: __m512i = _mm512_broadcast_i32x4(iso128);
    let coeff_bytes = coeff_table_avx512(c);
    let coeff_vec: __m512i = unsafe { _mm512_loadu_si512(coeff_bytes.as_ptr().cast()) };
    let coeff_mapped = _mm512_gf2p8affine_epi64_epi8::<0>(coeff_vec, iso512);

    (iso512, coeff_mapped)
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_gfni_avx2_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_gfni_avx2_mul_slice_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_gfni_avx2_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_gfni_avx2_mul_slice_xor_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_gfni_avx512_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_gfni_avx512_mul_slice_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_gfni_avx512_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_gfni_avx512_mul_slice_xor_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "gfni,avx2")]
unsafe fn rust_gfni_avx2_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_loadu_si128, _mm256_broadcastsi128_si256,
        _mm256_gf2p8affine_epi64_epi8, _mm256_gf2p8mul_epi8, _mm256_loadu_si256,
        _mm256_storeu_si256,
    };

    let (iso256, coeff_mapped): (__m256i, __m256i) = unsafe { gfni_avx2_constants(c) };

    let bytes_done = input.len() & !31usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm256_loadu_si256(input.as_ptr().add(offset).cast()) };
        let mapped_input = _mm256_gf2p8affine_epi64_epi8(input_vec, iso256, 0);
        let product = _mm256_gf2p8mul_epi8(mapped_input, coeff_mapped);
        let restored = _mm256_gf2p8affine_epi64_epi8(product, iso256, 0);
        unsafe { _mm256_storeu_si256(out.as_mut_ptr().add(offset).cast(), restored) };
        offset += 32;
    }

    super::super::scalar::mul_slice_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "gfni,avx2")]
unsafe fn rust_gfni_avx2_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_loadu_si128, _mm256_broadcastsi128_si256,
        _mm256_gf2p8affine_epi64_epi8, _mm256_gf2p8mul_epi8, _mm256_loadu_si256,
        _mm256_storeu_si256, _mm256_xor_si256,
    };

    let (iso256, coeff_mapped): (__m256i, __m256i) = unsafe { gfni_avx2_constants(c) };

    let bytes_done = input.len() & !31usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm256_loadu_si256(input.as_ptr().add(offset).cast()) };
        let mapped_input = _mm256_gf2p8affine_epi64_epi8(input_vec, iso256, 0);
        let product = _mm256_gf2p8mul_epi8(mapped_input, coeff_mapped);
        let restored = _mm256_gf2p8affine_epi64_epi8(product, iso256, 0);
        let out_vec = unsafe { _mm256_loadu_si256(out.as_ptr().add(offset).cast()) };
        unsafe {
            _mm256_storeu_si256(
                out.as_mut_ptr().add(offset).cast(),
                _mm256_xor_si256(out_vec, restored),
            )
        };
        offset += 32;
    }

    super::super::scalar::mul_slice_xor_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "gfni,avx512f,avx512bw")]
unsafe fn rust_gfni_avx512_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m512i, _mm_loadu_si128, _mm512_broadcast_i32x4, _mm512_gf2p8affine_epi64_epi8,
        _mm512_gf2p8mul_epi8, _mm512_loadu_si512, _mm512_storeu_si512,
    };

    let (iso512, coeff_mapped): (__m512i, __m512i) = unsafe { gfni_avx512_constants(c) };

    let bytes_done = input.len() & !63usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm512_loadu_si512(input.as_ptr().add(offset).cast()) };
        let mapped_input = _mm512_gf2p8affine_epi64_epi8::<0>(input_vec, iso512);
        let product = _mm512_gf2p8mul_epi8(mapped_input, coeff_mapped);
        let restored = _mm512_gf2p8affine_epi64_epi8::<0>(product, iso512);
        unsafe { _mm512_storeu_si512(out.as_mut_ptr().add(offset).cast(), restored) };
        offset += 64;
    }

    super::super::scalar::mul_slice_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "gfni,avx512f,avx512bw")]
unsafe fn rust_gfni_avx512_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m512i, _mm_loadu_si128, _mm512_broadcast_i32x4, _mm512_gf2p8affine_epi64_epi8,
        _mm512_gf2p8mul_epi8, _mm512_loadu_si512, _mm512_storeu_si512, _mm512_xor_si512,
    };

    let (iso512, coeff_mapped): (__m512i, __m512i) = unsafe { gfni_avx512_constants(c) };

    let bytes_done = input.len() & !63usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm512_loadu_si512(input.as_ptr().add(offset).cast()) };
        let mapped_input = _mm512_gf2p8affine_epi64_epi8::<0>(input_vec, iso512);
        let product = _mm512_gf2p8mul_epi8(mapped_input, coeff_mapped);
        let restored = _mm512_gf2p8affine_epi64_epi8::<0>(product, iso512);
        let out_vec = unsafe { _mm512_loadu_si512(out.as_ptr().add(offset).cast()) };
        unsafe {
            _mm512_storeu_si512(
                out.as_mut_ptr().add(offset).cast(),
                _mm512_xor_si512(out_vec, restored),
            )
        };
        offset += 64;
    }

    super::super::scalar::mul_slice_xor_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    test,
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
mod tests {
    use alloc::vec;

    use super::*;
    use crate::galois_8::{mul_slice_scalar_for_test, mul_slice_xor_scalar_for_test, x86};
    use crate::tests::fill_random;
    use rand;

    const LENGTHS: [usize; 8] = [0usize, 1, 31, 32, 33, 255, 1024, 10_003];

    #[test]
    fn gfni_avx2_matches_scalar_mul_slice() {
        if !(std::is_x86_feature_detected!("gfni") && std::is_x86_feature_detected!("avx2")) {
            return;
        }
        for &len in &LENGTHS {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut gfni = vec![0; len];

                mul_slice_scalar_for_test(c, &input, &mut scalar);
                rust_gfni_avx2_mul_slice(c, &input, &mut gfni);

                assert_eq!(scalar, gfni);
            }
        }
    }

    #[test]
    fn gfni_avx2_matches_scalar_mul_slice_xor() {
        if !(std::is_x86_feature_detected!("gfni") && std::is_x86_feature_detected!("avx2")) {
            return;
        }
        for &len in &LENGTHS {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut gfni = vec![0; len];
                fill_random(&mut scalar);
                gfni.copy_from_slice(&scalar);

                mul_slice_xor_scalar_for_test(c, &input, &mut scalar);
                rust_gfni_avx2_mul_slice_xor(c, &input, &mut gfni);

                assert_eq!(scalar, gfni);
            }
        }
    }

    #[test]
    fn gfni_avx2_matches_avx2_mul_slice() {
        if !(std::is_x86_feature_detected!("gfni") && std::is_x86_feature_detected!("avx2")) {
            return;
        }
        for &len in &LENGTHS {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut avx2 = vec![0; len];
                let mut gfni = vec![0; len];

                x86::avx2::rust_avx2_mul_slice(c, &input, &mut avx2);
                rust_gfni_avx2_mul_slice(c, &input, &mut gfni);

                assert_eq!(avx2, gfni);
            }
        }
    }

    #[test]
    fn gfni_avx512_matches_scalar_mul_slice() {
        if !(std::is_x86_feature_detected!("gfni")
            && std::is_x86_feature_detected!("avx512f")
            && std::is_x86_feature_detected!("avx512bw"))
        {
            return;
        }
        for &len in &[0usize, 1, 63, 64, 65, 255, 1024, 10_003] {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut gfni = vec![0; len];

                mul_slice_scalar_for_test(c, &input, &mut scalar);
                rust_gfni_avx512_mul_slice(c, &input, &mut gfni);

                assert_eq!(scalar, gfni);
            }
        }
    }

    #[test]
    fn gfni_avx512_matches_avx512_mul_slice() {
        if !(std::is_x86_feature_detected!("gfni")
            && std::is_x86_feature_detected!("avx512f")
            && std::is_x86_feature_detected!("avx512bw"))
        {
            return;
        }
        for &len in &[0usize, 1, 63, 64, 65, 255, 1024, 10_003] {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut avx512 = vec![0; len];
                let mut gfni = vec![0; len];

                x86::avx512::rust_avx512_mul_slice(c, &input, &mut avx512);
                rust_gfni_avx512_mul_slice(c, &input, &mut gfni);

                assert_eq!(avx512, gfni);
            }
        }
    }
}
