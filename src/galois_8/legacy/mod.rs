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
pub(crate) mod simd_c;
