use super::scalar::{mul_slice_pure_rust, mul_slice_xor_pure_rust};
use spin::Once;

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
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_NEON_BACKEND: GaloisBackend = GaloisBackend {
    mul_slice: super::aarch64::neon::rust_neon_mul_slice,
    mul_slice_xor: super::aarch64::neon::rust_neon_mul_slice_xor,
    name: "rust-neon",
    kind: BackendKind::RustSimd,
};

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_AVX2_BACKEND: GaloisBackend = GaloisBackend {
    mul_slice: super::x86::avx2::rust_avx2_mul_slice,
    mul_slice_xor: super::x86::avx2::rust_avx2_mul_slice_xor,
    name: "rust-avx2",
    kind: BackendKind::RustSimd,
};

static ACTIVE_BACKEND: Once<GaloisBackend> = Once::new();

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const SIMD_C_BACKEND: GaloisBackend = GaloisBackend {
    mul_slice: super::legacy::simd_c::simd_c_mul_slice,
    mul_slice_xor: super::legacy::simd_c::simd_c_mul_slice_xor,
    name: "simd-c",
    kind: BackendKind::SimdC,
};

#[cfg(feature = "std")]
#[derive(Copy, Clone)]
enum BackendOverride {
    Auto,
    Scalar,
    SimdC,
    RustNeon,
    RustAvx2,
}

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn runtime_select_backend() -> GaloisBackend {
    #[cfg(feature = "std")]
    if let Some(backend) = runtime_override_backend() {
        return backend;
    }

    #[cfg(target_arch = "x86_64")]
    if rust_avx2_supported_at_runtime() {
        return RUST_AVX2_BACKEND;
    }

    #[cfg(target_arch = "aarch64")]
    if rust_neon_supported_at_runtime() {
        return RUST_NEON_BACKEND;
    }

    if simd_c_supported_at_runtime() {
        return SIMD_C_BACKEND;
    }

    SCALAR_BACKEND
}

#[cfg(feature = "std")]
fn parse_backend_override(value: &str) -> Option<BackendOverride> {
    match value {
        "auto" => Some(BackendOverride::Auto),
        "scalar" | "scalar-rust" => Some(BackendOverride::Scalar),
        "simd-c" => Some(BackendOverride::SimdC),
        "rust-neon" => Some(BackendOverride::RustNeon),
        "rust-avx2" => Some(BackendOverride::RustAvx2),
        _ => None,
    }
}

#[cfg(feature = "std")]
fn runtime_override_backend() -> Option<GaloisBackend> {
    let value = std::env::var("RSE_BACKEND_OVERRIDE").ok()?;
    match parse_backend_override(value.trim())? {
        BackendOverride::Auto => None,
        BackendOverride::Scalar => Some(SCALAR_BACKEND),
        BackendOverride::SimdC => simd_c_override_backend(),
        BackendOverride::RustNeon => rust_neon_override_backend(),
        BackendOverride::RustAvx2 => rust_avx2_override_backend(),
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
pub(super) fn runtime_override_backend_name_for_test() -> Option<&'static str> {
    runtime_override_backend().map(|backend| backend.name)
}

#[cfg(feature = "std")]
#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn simd_c_override_backend() -> Option<GaloisBackend> {
    simd_c_supported_at_runtime().then_some(SIMD_C_BACKEND)
}

#[cfg(feature = "std")]
#[cfg(not(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
)))]
fn simd_c_override_backend() -> Option<GaloisBackend> {
    None
}

#[cfg(feature = "std")]
#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn rust_neon_override_backend() -> Option<GaloisBackend> {
    rust_neon_supported_at_runtime().then_some(RUST_NEON_BACKEND)
}

#[cfg(feature = "std")]
#[cfg(not(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
)))]
fn rust_neon_override_backend() -> Option<GaloisBackend> {
    None
}

#[cfg(feature = "std")]
#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn rust_avx2_override_backend() -> Option<GaloisBackend> {
    rust_avx2_supported_at_runtime().then_some(RUST_AVX2_BACKEND)
}

#[cfg(feature = "std")]
#[cfg(not(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
)))]
fn rust_avx2_override_backend() -> Option<GaloisBackend> {
    None
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[cfg(feature = "std")]
fn rust_neon_supported_at_runtime() -> bool {
    std::arch::is_aarch64_feature_detected!("neon")
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[cfg(not(feature = "std"))]
fn rust_neon_supported_at_runtime() -> bool {
    false
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[cfg(feature = "std")]
fn rust_avx2_supported_at_runtime() -> bool {
    std::is_x86_feature_detected!("avx2")
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[cfg(not(feature = "std"))]
fn rust_avx2_supported_at_runtime() -> bool {
    false
}

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[cfg(feature = "std")]
fn simd_c_supported_at_runtime() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        if cfg!(rse_simd_c_build_haswell) {
            return std::is_x86_feature_detected!("avx2");
        }
        if cfg!(rse_simd_c_build_baseline) {
            return std::is_x86_feature_detected!("sse2");
        }
        false
    }

    #[cfg(target_arch = "aarch64")]
    {
        if cfg!(rse_simd_c_build_unknown) {
            return false;
        }
        std::arch::is_aarch64_feature_detected!("neon")
    }
}

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[cfg(not(feature = "std"))]
fn simd_c_supported_at_runtime() -> bool {
    false
}

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(super) fn active_backend() -> &'static GaloisBackend {
    ACTIVE_BACKEND.call_once(runtime_select_backend)
}

#[cfg(not(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
)))]
pub(super) fn active_backend() -> &'static GaloisBackend {
    ACTIVE_BACKEND.call_once(|| SCALAR_BACKEND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn test_parse_backend_override() {
        assert!(matches!(
            parse_backend_override("auto"),
            Some(BackendOverride::Auto)
        ));
        assert!(matches!(
            parse_backend_override("scalar"),
            Some(BackendOverride::Scalar)
        ));
        assert!(matches!(
            parse_backend_override("scalar-rust"),
            Some(BackendOverride::Scalar)
        ));
        assert!(matches!(
            parse_backend_override("simd-c"),
            Some(BackendOverride::SimdC)
        ));
        assert!(matches!(
            parse_backend_override("rust-neon"),
            Some(BackendOverride::RustNeon)
        ));
        assert!(matches!(
            parse_backend_override("rust-avx2"),
            Some(BackendOverride::RustAvx2)
        ));
        assert!(parse_backend_override("bogus").is_none());
    }
}
