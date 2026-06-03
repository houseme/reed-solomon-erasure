#[cfg(all(feature = "simd-vsx", target_arch = "powerpc64"))]
pub(crate) mod vsx;
