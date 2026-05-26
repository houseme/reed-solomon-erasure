#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) mod avx2;
#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) mod avx512;
#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) mod gfni;
#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) mod ssse3;

/// Shared helper: returns the low and high nibble multiplication table halves for coefficient `c`.
#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[inline]
fn load_table_halves(c: u8) -> (&'static [u8; 16], &'static [u8; 16]) {
    (
        &super::MUL_TABLE_LOW[c as usize],
        &super::MUL_TABLE_HIGH[c as usize],
    )
}
