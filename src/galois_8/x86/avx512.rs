#[cfg(test)]
extern crate alloc;

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[inline]
fn load_tables(c: u8) -> (core::arch::x86_64::__m512i, core::arch::x86_64::__m512i) {
    use core::arch::x86_64::{__m128i, __m512i, _mm512_broadcast_i32x4, _mm_loadu_si128};

    let low128: __m128i =
        unsafe { _mm_loadu_si128(super::super::MUL_TABLE_LOW[c as usize].as_ptr().cast()) };
    let high128: __m128i =
        unsafe { _mm_loadu_si128(super::super::MUL_TABLE_HIGH[c as usize].as_ptr().cast()) };

    (
        _mm512_broadcast_i32x4(low128),
        _mm512_broadcast_i32x4(high128),
    )
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_avx512_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_avx512_mul_slice_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_avx512_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_avx512_mul_slice_xor_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn rust_avx512_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m512i, _mm512_and_si512, _mm512_loadu_si512, _mm512_set1_epi8, _mm512_shuffle_epi8,
        _mm512_srli_epi64, _mm512_storeu_si512, _mm512_xor_si512,
    };

    let (low_tbl, high_tbl): (__m512i, __m512i) = load_tables(c);
    let nibble_mask: __m512i = _mm512_set1_epi8(0x0f);

    let bytes_done = input.len() & !63usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm512_loadu_si512(input.as_ptr().add(offset).cast()) };
        let low = _mm512_and_si512(input_vec, nibble_mask);
        let high = _mm512_and_si512(_mm512_srli_epi64::<4>(input_vec), nibble_mask);
        let result = _mm512_xor_si512(
            _mm512_shuffle_epi8(low_tbl, low),
            _mm512_shuffle_epi8(high_tbl, high),
        );
        unsafe { _mm512_storeu_si512(out.as_mut_ptr().add(offset).cast(), result) };
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
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn rust_avx512_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m512i, _mm512_and_si512, _mm512_loadu_si512, _mm512_set1_epi8, _mm512_shuffle_epi8,
        _mm512_srli_epi64, _mm512_storeu_si512, _mm512_xor_si512,
    };

    let (low_tbl, high_tbl): (__m512i, __m512i) = load_tables(c);
    let nibble_mask: __m512i = _mm512_set1_epi8(0x0f);

    let bytes_done = input.len() & !63usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm512_loadu_si512(input.as_ptr().add(offset).cast()) };
        let low = _mm512_and_si512(input_vec, nibble_mask);
        let high = _mm512_and_si512(_mm512_srli_epi64::<4>(input_vec), nibble_mask);
        let product = _mm512_xor_si512(
            _mm512_shuffle_epi8(low_tbl, low),
            _mm512_shuffle_epi8(high_tbl, high),
        );
        let out_vec = unsafe { _mm512_loadu_si512(out.as_ptr().add(offset).cast()) };
        unsafe {
            _mm512_storeu_si512(
                out.as_mut_ptr().add(offset).cast(),
                _mm512_xor_si512(out_vec, product),
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

    const LENGTHS: [usize; 8] = [0usize, 1, 63, 64, 65, 255, 1024, 10_003];

    #[test]
    fn avx512_matches_scalar_mul_slice() {
        if !(std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512bw"))
        {
            return;
        }
        for &len in &LENGTHS {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut avx512 = vec![0; len];

                mul_slice_scalar_for_test(c, &input, &mut scalar);
                rust_avx512_mul_slice(c, &input, &mut avx512);

                assert_eq!(scalar, avx512);
            }
        }
    }

    #[test]
    fn avx512_matches_scalar_mul_slice_xor() {
        if !(std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512bw"))
        {
            return;
        }
        for &len in &LENGTHS {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut scalar = vec![0; len];
                let mut avx512 = vec![0; len];
                fill_random(&mut scalar);
                avx512.copy_from_slice(&scalar);

                mul_slice_xor_scalar_for_test(c, &input, &mut scalar);
                rust_avx512_mul_slice_xor(c, &input, &mut avx512);

                assert_eq!(scalar, avx512);
            }
        }
    }

    #[test]
    fn avx512_matches_avx2_mul_slice() {
        if !(std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512bw"))
        {
            return;
        }
        for &len in &LENGTHS {
            for _ in 0..16 {
                let c = rand::random::<u8>();
                let mut input = vec![0; len];
                fill_random(&mut input);
                let mut avx2 = vec![0; len];
                let mut avx512 = vec![0; len];

                x86::avx2::rust_avx2_mul_slice(c, &input, &mut avx2);
                rust_avx512_mul_slice(c, &input, &mut avx512);

                assert_eq!(avx2, avx512);
            }
        }
    }
}
