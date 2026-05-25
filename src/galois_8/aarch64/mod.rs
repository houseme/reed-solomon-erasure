#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) mod neon;
