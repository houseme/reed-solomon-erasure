use super::{mul_slice_pure_rust, mul_slice_xor_pure_rust};

pub type MulSliceFn = fn(u8, &[u8], &mut [u8]);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BackendKind {
    Scalar,
    SimdC,
    RustSimd,
}

#[derive(Copy, Clone)]
pub struct GaloisBackend {
    pub mul_slice: MulSliceFn,
    pub mul_slice_xor: MulSliceFn,
    pub name: &'static str,
    pub kind: BackendKind,
}

const SCALAR_BACKEND: GaloisBackend = GaloisBackend {
    mul_slice: mul_slice_pure_rust,
    mul_slice_xor: mul_slice_xor_pure_rust,
    name: "scalar-rust",
    kind: BackendKind::Scalar,
};

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const SIMD_C_BACKEND: GaloisBackend = GaloisBackend {
    mul_slice: super::simd_c_mul_slice,
    mul_slice_xor: super::simd_c_mul_slice_xor,
    name: "simd-c",
    kind: BackendKind::SimdC,
};

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(super) fn active_backend() -> &'static GaloisBackend {
    &SIMD_C_BACKEND
}

#[cfg(not(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
)))]
pub(super) fn active_backend() -> &'static GaloisBackend {
    &SCALAR_BACKEND
}
