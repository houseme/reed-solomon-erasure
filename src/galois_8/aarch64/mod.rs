#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) mod neon;

// Reserved extension slot for future aarch64 SIMD backends such as SVE.
// The current phase keeps NEON as the only Rust aarch64 backend while making
// the module layout explicit enough that a future SVE backend will not need to
// re-open the top-level dispatch structure.
#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) mod sve;
