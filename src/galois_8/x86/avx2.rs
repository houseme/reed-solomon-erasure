#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_avx2_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_avx2_mul_slice_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_avx2_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_avx2_mul_slice_xor_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "avx2")]
unsafe fn rust_avx2_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm256_and_si256, _mm256_broadcastsi128_si256, _mm256_loadu_si256,
        _mm256_set1_epi8, _mm256_shuffle_epi8, _mm256_srli_epi64, _mm256_storeu_si256,
        _mm256_xor_si256, _mm_loadu_si128,
    };

    let low128: __m128i =
        unsafe { _mm_loadu_si128(super::super::MUL_TABLE_LOW[c as usize].as_ptr().cast()) };
    let high128: __m128i =
        unsafe { _mm_loadu_si128(super::super::MUL_TABLE_HIGH[c as usize].as_ptr().cast()) };
    let low_tbl: __m256i = _mm256_broadcastsi128_si256(low128);
    let high_tbl: __m256i = _mm256_broadcastsi128_si256(high128);
    let nibble_mask: __m256i = _mm256_set1_epi8(0x0f);

    let bytes_done = input.len() & !31usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm256_loadu_si256(input.as_ptr().add(offset).cast()) };
        let low = _mm256_and_si256(input_vec, nibble_mask);
        let high = _mm256_and_si256(_mm256_srli_epi64::<4>(input_vec), nibble_mask);
        let result = _mm256_xor_si256(
            _mm256_shuffle_epi8(low_tbl, low),
            _mm256_shuffle_epi8(high_tbl, high),
        );
        unsafe { _mm256_storeu_si256(out.as_mut_ptr().add(offset).cast(), result) };
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
#[target_feature(enable = "avx2")]
unsafe fn rust_avx2_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm256_and_si256, _mm256_broadcastsi128_si256, _mm256_loadu_si256,
        _mm256_set1_epi8, _mm256_shuffle_epi8, _mm256_srli_epi64, _mm256_storeu_si256,
        _mm256_xor_si256, _mm_loadu_si128,
    };

    let low128: __m128i =
        unsafe { _mm_loadu_si128(super::super::MUL_TABLE_LOW[c as usize].as_ptr().cast()) };
    let high128: __m128i =
        unsafe { _mm_loadu_si128(super::super::MUL_TABLE_HIGH[c as usize].as_ptr().cast()) };
    let low_tbl: __m256i = _mm256_broadcastsi128_si256(low128);
    let high_tbl: __m256i = _mm256_broadcastsi128_si256(high128);
    let nibble_mask: __m256i = _mm256_set1_epi8(0x0f);

    let bytes_done = input.len() & !31usize;
    let mut offset = 0usize;
    while offset < bytes_done {
        let input_vec = unsafe { _mm256_loadu_si256(input.as_ptr().add(offset).cast()) };
        let low = _mm256_and_si256(input_vec, nibble_mask);
        let high = _mm256_and_si256(_mm256_srli_epi64::<4>(input_vec), nibble_mask);
        let product = _mm256_xor_si256(
            _mm256_shuffle_epi8(low_tbl, low),
            _mm256_shuffle_epi8(high_tbl, high),
        );
        let out_vec = unsafe { _mm256_loadu_si256(out.as_ptr().add(offset).cast()) };
        unsafe {
            _mm256_storeu_si256(
                out.as_mut_ptr().add(offset).cast(),
                _mm256_xor_si256(out_vec, product),
            )
        };
        offset += 32;
    }

    super::super::scalar::mul_slice_xor_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}
