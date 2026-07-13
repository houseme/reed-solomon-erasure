#[cfg(all(
    any(
        feature = "simd-neon",
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
unsafe extern "C" {
    fn reedsolomon_gal_mul(
        low: *const u8,
        high: *const u8,
        input: *const u8,
        out: *mut u8,
        len: libc::size_t,
    ) -> libc::size_t;

    fn reedsolomon_gal_mul_xor(
        low: *const u8,
        high: *const u8,
        input: *const u8,
        out: *mut u8,
        len: libc::size_t,
    ) -> libc::size_t;
}

#[cfg(all(
    any(
        feature = "simd-neon",
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn simd_c_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    let low: *const u8 = &super::super::MUL_TABLE_LOW[c as usize][0];
    let high: *const u8 = &super::super::MUL_TABLE_HIGH[c as usize][0];

    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    let input_ptr: *const u8 = &input[0];
    let out_ptr: *mut u8 = &mut out[0];
    let size: libc::size_t = input.len();

    // SAFETY: `low`/`high` point to 16-byte multiply-table rows; `input_ptr`/`out_ptr` are valid,
    // non-null (slices are non-empty), and both span `size` bytes (`assert_eq!(input.len(), out.len())`).
    // The C routine performs only unaligned accesses (built with USE_ALIGNED_ACCESS=0).
    let bytes_done: usize =
        unsafe { reedsolomon_gal_mul(low, high, input_ptr, out_ptr, size) as usize };

    if bytes_done == 0 {
        super::super::scalar::mul_slice_pure_rust(c, input, out);
    } else if bytes_done < size {
        super::super::scalar::mul_slice_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
    } else if bytes_done > size {
        super::super::scalar::mul_slice_pure_rust(c, input, out);
    }
}

#[cfg(all(
    any(
        feature = "simd-neon",
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn simd_c_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    let low: *const u8 = &super::super::MUL_TABLE_LOW[c as usize][0];
    let high: *const u8 = &super::super::MUL_TABLE_HIGH[c as usize][0];

    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    let input_ptr: *const u8 = &input[0];
    let out_ptr: *mut u8 = &mut out[0];
    let size: libc::size_t = input.len();

    // SAFETY: `low`/`high` point to 16-byte multiply-table rows; `input_ptr`/`out_ptr` are valid,
    // non-null (slices are non-empty), and both span `size` bytes (`assert_eq!(input.len(), out.len())`).
    // The C routine performs only unaligned accesses (built with USE_ALIGNED_ACCESS=0).
    let bytes_done: usize =
        unsafe { reedsolomon_gal_mul_xor(low, high, input_ptr, out_ptr, size) as usize };

    if bytes_done == 0 {
        super::super::scalar::mul_slice_xor_pure_rust(c, input, out);
    } else if bytes_done < size {
        super::super::scalar::mul_slice_xor_pure_rust(
            c,
            &input[bytes_done..],
            &mut out[bytes_done..],
        );
    } else if bytes_done > size {
        super::super::scalar::mul_slice_xor_pure_rust(c, input, out);
    }
}
