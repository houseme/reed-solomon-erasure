use super::scalar::{mul_slice_pure_rust, mul_slice_xor_pure_rust};
use spin::Once;

pub type MulSliceFn = fn(u8, &[u8], &mut [u8]);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BackendKind {
    Scalar,
    SimdC,
    RustSimd,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BackendId {
    ScalarRust,
    SimdC,
    RustNeon,
    RustSsse3,
    RustAvx2,
    RustAvx512,
    RustGfniAvx2,
    RustGfniAvx512,
}

#[derive(Copy, Clone)]
pub struct GaloisBackend {
    pub id: BackendId,
    pub mul_slice: MulSliceFn,
    pub mul_slice_xor: MulSliceFn,
    pub name: &'static str,
    pub kind: BackendKind,
}

const SCALAR_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::ScalarRust,
    mul_slice: mul_slice_pure_rust,
    mul_slice_xor: mul_slice_xor_pure_rust,
    name: "scalar-rust",
    kind: BackendKind::Scalar,
};

#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_NEON_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::RustNeon,
    mul_slice: super::aarch64::neon::rust_neon_mul_slice,
    mul_slice_xor: super::aarch64::neon::rust_neon_mul_slice_xor,
    name: "rust-neon",
    kind: BackendKind::RustSimd,
};

#[cfg(all(
    feature = "simd-avx2",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_AVX2_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::RustAvx2,
    mul_slice: super::x86::avx2::rust_avx2_mul_slice,
    mul_slice_xor: super::x86::avx2::rust_avx2_mul_slice_xor,
    name: "rust-avx2",
    kind: BackendKind::RustSimd,
};

#[cfg(all(
    feature = "simd-avx512",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_AVX512_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::RustAvx512,
    mul_slice: super::x86::avx512::rust_avx512_mul_slice,
    mul_slice_xor: super::x86::avx512::rust_avx512_mul_slice_xor,
    name: "rust-avx512",
    kind: BackendKind::RustSimd,
};

#[cfg(all(
    feature = "simd-gfni",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_GFNI_AVX2_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::RustGfniAvx2,
    mul_slice: super::x86::gfni::rust_gfni_avx2_mul_slice,
    mul_slice_xor: super::x86::gfni::rust_gfni_avx2_mul_slice_xor,
    name: "rust-gfni-avx2",
    kind: BackendKind::RustSimd,
};

#[cfg(all(
    feature = "simd-gfni",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_GFNI_AVX512_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::RustGfniAvx512,
    mul_slice: super::x86::gfni::rust_gfni_avx512_mul_slice,
    mul_slice_xor: super::x86::gfni::rust_gfni_avx512_mul_slice_xor,
    name: "rust-gfni-avx512",
    kind: BackendKind::RustSimd,
};

#[cfg(all(
    feature = "simd-ssse3",
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_SSSE3_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::RustSsse3,
    mul_slice: super::x86::ssse3::rust_ssse3_mul_slice,
    mul_slice_xor: super::x86::ssse3::rust_ssse3_mul_slice_xor,
    name: "rust-ssse3",
    kind: BackendKind::RustSimd,
};

static ACTIVE_BACKEND: Once<GaloisBackend> = Once::new();

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
const SIMD_C_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::SimdC,
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
    RustSsse3,
    RustAvx2,
    RustAvx512,
    RustGfniAvx2,
    RustGfniAvx512,
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
struct X86FeatureSet {
    sse2: bool,
    ssse3: bool,
    avx2: bool,
    avx512f: bool,
    avx512bw: bool,
    gfni: bool,
}

#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
struct Aarch64FeatureSet {
    neon: bool,
    sve: bool,
}

#[cfg(any(
    all(
        any(
            feature = "simd-ssse3",
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-gfni"
        ),
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    ),
    all(
        feature = "simd-neon",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    )
))]
fn runtime_select_backend() -> GaloisBackend {
    #[cfg(feature = "std")]
    if let Some(backend) = runtime_override_backend() {
        return backend;
    }

    auto_select_backend()
}

#[cfg(feature = "std")]
fn parse_backend_override(value: &str) -> Option<BackendOverride> {
    match value {
        "auto" => Some(BackendOverride::Auto),
        "scalar" | "scalar-rust" => Some(BackendOverride::Scalar),
        "simd-c" => Some(BackendOverride::SimdC),
        "rust-neon" => Some(BackendOverride::RustNeon),
        "rust-ssse3" => Some(BackendOverride::RustSsse3),
        "rust-avx2" => Some(BackendOverride::RustAvx2),
        "rust-avx512" => Some(BackendOverride::RustAvx512),
        "rust-gfni-avx2" => Some(BackendOverride::RustGfniAvx2),
        "rust-gfni-avx512" => Some(BackendOverride::RustGfniAvx512),
        _ => None,
    }
}

#[cfg(feature = "std")]
fn runtime_override_backend() -> Option<GaloisBackend> {
    let value = std::env::var("RSE_BACKEND_OVERRIDE").ok()?;
    select_override_backend(parse_backend_override(value.trim())?)
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn auto_select_backend() -> GaloisBackend {
    select_x86_backend(detect_x86_features())
}

#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn auto_select_backend() -> GaloisBackend {
    select_aarch64_backend(detect_aarch64_features())
}

#[cfg(any(
    all(
        any(
            feature = "simd-ssse3",
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-gfni"
        ),
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        not(feature = "std")
    ),
    all(
        feature = "simd-neon",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        not(feature = "std")
    )
))]
fn auto_select_backend() -> GaloisBackend {
    SCALAR_BACKEND
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn detect_x86_features() -> X86FeatureSet {
    X86FeatureSet {
        sse2: std::is_x86_feature_detected!("sse2"),
        ssse3: std::is_x86_feature_detected!("ssse3"),
        avx2: std::is_x86_feature_detected!("avx2"),
        avx512f: std::is_x86_feature_detected!("avx512f"),
        avx512bw: std::is_x86_feature_detected!("avx512bw"),
        gfni: std::is_x86_feature_detected!("gfni"),
    }
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn supports_rust_avx2(features: X86FeatureSet) -> bool {
    features.avx2
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn supports_rust_avx512(features: X86FeatureSet) -> bool {
    features.avx512f && features.avx512bw
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
/// GFNI+AVX2 backend. Auto-selected with highest priority when available (Ice Lake+)
/// because `_gf2p8mul` provides native GF(2^8) multiplication, eliminating nibble-lookup overhead.
/// Priority: GFNI+AVX-512 > GFNI+AVX2 > AVX2 > AVX-512 > SSSE3 > SIMD-C > Scalar.
fn supports_rust_gfni_avx2(features: X86FeatureSet) -> bool {
    features.gfni && features.avx2
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
/// GFNI+AVX-512 backend. Auto-selected with highest priority when available.
/// See `select_x86_backend` for full priority rationale.
fn supports_rust_gfni_avx512(features: X86FeatureSet) -> bool {
    features.gfni && features.avx512f && features.avx512bw
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn supports_rust_ssse3(features: X86FeatureSet) -> bool {
    features.ssse3
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn supports_simd_c_x86(features: X86FeatureSet) -> bool {
    if cfg!(rse_simd_c_build_baseline) {
        return features.sse2;
    }
    false
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn select_x86_override_backend(
    backend_override: BackendOverride,
    features: X86FeatureSet,
) -> Option<GaloisBackend> {
    match backend_override {
        BackendOverride::Auto => None,
        BackendOverride::Scalar => Some(SCALAR_BACKEND),
        BackendOverride::SimdC => supports_simd_c_x86(features).then_some(SIMD_C_BACKEND),
        BackendOverride::RustSsse3 => supports_rust_ssse3(features).then_some(RUST_SSSE3_BACKEND),
        BackendOverride::RustAvx2 => supports_rust_avx2(features).then_some(RUST_AVX2_BACKEND),
        BackendOverride::RustAvx512 => {
            supports_rust_avx512(features).then_some(RUST_AVX512_BACKEND)
        }
        BackendOverride::RustGfniAvx2 => {
            supports_rust_gfni_avx2(features).then_some(RUST_GFNI_AVX2_BACKEND)
        }
        BackendOverride::RustGfniAvx512 => {
            supports_rust_gfni_avx512(features).then_some(RUST_GFNI_AVX512_BACKEND)
        }
        BackendOverride::RustNeon => None,
    }
}

#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
/// Selects the best available x86_64 backend via runtime feature detection.
///
/// **Priority order**: GFNI+AVX-512 > GFNI+AVX2 > AVX2 > AVX-512 > SSSE3 > SIMD-C > Scalar.
///
/// GFNI backends are preferred when available (Ice Lake+) because they provide
/// native GF(2^8) multiplication via `_gf2p8mul`, eliminating the nibble-lookup
/// overhead. AVX2 is ranked above AVX-512 for non-GFNI because AVX-512 can
/// cause frequency throttling on some microarchitectures.
fn select_x86_backend(features: X86FeatureSet) -> GaloisBackend {
    if supports_rust_gfni_avx512(features) {
        return RUST_GFNI_AVX512_BACKEND;
    }
    if supports_rust_gfni_avx2(features) {
        return RUST_GFNI_AVX2_BACKEND;
    }
    if supports_rust_avx2(features) {
        return RUST_AVX2_BACKEND;
    }
    if supports_rust_avx512(features) {
        return RUST_AVX512_BACKEND;
    }
    if supports_rust_ssse3(features) {
        return RUST_SSSE3_BACKEND;
    }
    if supports_simd_c_x86(features) {
        return SIMD_C_BACKEND;
    }
    SCALAR_BACKEND
}

#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn detect_aarch64_features() -> Aarch64FeatureSet {
    let sve = super::aarch64::sve::detect_sve_features().available;
    Aarch64FeatureSet {
        neon: std::arch::is_aarch64_feature_detected!("neon"),
        sve,
    }
}

#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn supports_rust_neon(features: Aarch64FeatureSet) -> bool {
    features.neon
}

#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn supports_simd_c_aarch64(features: Aarch64FeatureSet) -> bool {
    !cfg!(rse_simd_c_build_unknown) && features.neon
}

#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn select_aarch64_override_backend(
    backend_override: BackendOverride,
    features: Aarch64FeatureSet,
) -> Option<GaloisBackend> {
    match backend_override {
        BackendOverride::Auto => None,
        BackendOverride::Scalar => Some(SCALAR_BACKEND),
        BackendOverride::SimdC => supports_simd_c_aarch64(features).then_some(SIMD_C_BACKEND),
        BackendOverride::RustNeon => supports_rust_neon(features).then_some(RUST_NEON_BACKEND),
        BackendOverride::RustSsse3
        | BackendOverride::RustAvx2
        | BackendOverride::RustAvx512
        | BackendOverride::RustGfniAvx2
        | BackendOverride::RustGfniAvx512 => None,
    }
}

#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
fn select_aarch64_backend(features: Aarch64FeatureSet) -> GaloisBackend {
    // SVE is detected but not yet used; reserved for future backend.
    let _sve = features.sve;
    if supports_rust_neon(features) {
        return RUST_NEON_BACKEND;
    }
    if supports_simd_c_aarch64(features) {
        return SIMD_C_BACKEND;
    }
    SCALAR_BACKEND
}

#[cfg(feature = "std")]
#[cfg(all(
    any(
        feature = "simd-ssse3",
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-gfni"
    ),
    target_arch = "x86_64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn select_override_backend(backend_override: BackendOverride) -> Option<GaloisBackend> {
    select_x86_override_backend(backend_override, detect_x86_features())
}

#[cfg(feature = "std")]
#[cfg(all(
    feature = "simd-neon",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
fn select_override_backend(backend_override: BackendOverride) -> Option<GaloisBackend> {
    select_aarch64_override_backend(backend_override, detect_aarch64_features())
}

#[cfg(feature = "std")]
#[cfg(not(any(
    all(
        any(
            feature = "simd-ssse3",
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-gfni"
        ),
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    ),
    all(
        feature = "simd-neon",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    )
)))]
fn select_override_backend(backend_override: BackendOverride) -> Option<GaloisBackend> {
    match backend_override {
        BackendOverride::Auto => None,
        BackendOverride::Scalar => Some(SCALAR_BACKEND),
        BackendOverride::SimdC
        | BackendOverride::RustNeon
        | BackendOverride::RustSsse3
        | BackendOverride::RustAvx2
        | BackendOverride::RustAvx512
        | BackendOverride::RustGfniAvx2
        | BackendOverride::RustGfniAvx512 => None,
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
pub(super) fn runtime_override_backend_name_for_test() -> Option<&'static str> {
    runtime_override_backend().map(|backend| backend.name)
}

#[cfg(test)]
#[cfg(feature = "std")]
pub(super) fn runtime_override_backend_id_for_test() -> Option<BackendId> {
    runtime_override_backend().map(|backend| backend.id)
}

#[cfg(any(
    all(
        any(
            feature = "simd-ssse3",
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-gfni"
        ),
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    ),
    all(
        feature = "simd-neon",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    )
))]
pub(super) fn active_backend() -> &'static GaloisBackend {
    ACTIVE_BACKEND.call_once(runtime_select_backend)
}

#[cfg(not(any(
    all(
        any(
            feature = "simd-ssse3",
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-gfni"
        ),
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    ),
    all(
        feature = "simd-neon",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios"))
    )
)))]
pub(super) fn active_backend() -> &'static GaloisBackend {
    ACTIVE_BACKEND.call_once(|| SCALAR_BACKEND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_ids_are_stable() {
        assert_eq!(BackendId::ScalarRust, SCALAR_BACKEND.id);
    }

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
            parse_backend_override("rust-ssse3"),
            Some(BackendOverride::RustSsse3)
        ));
        assert!(matches!(
            parse_backend_override("rust-avx2"),
            Some(BackendOverride::RustAvx2)
        ));
        assert!(matches!(
            parse_backend_override("rust-avx512"),
            Some(BackendOverride::RustAvx512)
        ));
        assert!(matches!(
            parse_backend_override("rust-gfni-avx2"),
            Some(BackendOverride::RustGfniAvx2)
        ));
        assert!(matches!(
            parse_backend_override("rust-gfni-avx512"),
            Some(BackendOverride::RustGfniAvx512)
        ));
        assert!(parse_backend_override("bogus").is_none());
    }

    #[cfg(all(
        any(
            feature = "simd-ssse3",
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-gfni"
        ),
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_select_x86_backend_priority() {
        // GFNI backends are preferred when available (native GF multiplication).
        // Priority: GFNI+AVX-512 > GFNI+AVX2 > AVX2 > AVX-512 > SSSE3 > SIMD-C > Scalar.
        assert_eq!(
            BackendId::RustGfniAvx512,
            select_x86_backend(X86FeatureSet {
                gfni: true,
                avx512f: true,
                avx512bw: true,
                avx2: true,
                ..X86FeatureSet::default()
            })
            .id
        );

        assert_eq!(
            BackendId::RustGfniAvx2,
            select_x86_backend(X86FeatureSet {
                gfni: true,
                avx2: true,
                ..X86FeatureSet::default()
            })
            .id
        );

        assert_eq!(
            BackendId::RustAvx2,
            select_x86_backend(X86FeatureSet {
                avx2: true,
                ..X86FeatureSet::default()
            })
            .id
        );

        assert_eq!(
            BackendId::RustAvx512,
            select_x86_backend(X86FeatureSet {
                avx512f: true,
                avx512bw: true,
                ..X86FeatureSet::default()
            })
            .id
        );

        if cfg!(rse_simd_c_build_baseline) {
            assert_eq!(
                BackendId::RustSsse3,
                select_x86_backend(X86FeatureSet {
                    ssse3: true,
                    sse2: true,
                    ..X86FeatureSet::default()
                })
                .id
            );
        }

        assert_eq!(
            BackendId::ScalarRust,
            select_x86_backend(X86FeatureSet::default()).id
        );

        assert_eq!(
            BackendId::SimdC,
            select_x86_backend(X86FeatureSet {
                sse2: true,
                ..X86FeatureSet::default()
            })
            .id
        );
    }

    #[cfg(all(
        any(
            feature = "simd-ssse3",
            feature = "simd-avx2",
            feature = "simd-avx512",
            feature = "simd-gfni"
        ),
        target_arch = "x86_64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_select_x86_override_backend_allows_experimental_gfni() {
        assert_eq!(
            Some(BackendId::RustGfniAvx512),
            select_x86_override_backend(
                BackendOverride::RustGfniAvx512,
                X86FeatureSet {
                    gfni: true,
                    avx512f: true,
                    avx512bw: true,
                    ..X86FeatureSet::default()
                },
            )
            .map(|backend| backend.id)
        );

        assert_eq!(
            Some(BackendId::RustGfniAvx2),
            select_x86_override_backend(
                BackendOverride::RustGfniAvx2,
                X86FeatureSet {
                    gfni: true,
                    avx2: true,
                    ..X86FeatureSet::default()
                },
            )
            .map(|backend| backend.id)
        );

        assert_eq!(
            None,
            select_x86_override_backend(
                BackendOverride::RustGfniAvx512,
                X86FeatureSet {
                    gfni: true,
                    avx2: true,
                    ..X86FeatureSet::default()
                },
            )
            .map(|backend| backend.id)
        );
    }

    #[cfg(all(
        feature = "simd-neon",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_select_aarch64_backend_priority() {
        assert_eq!(
            BackendId::RustNeon,
            select_aarch64_backend(Aarch64FeatureSet {
                neon: true,
                sve: false,
            })
            .id
        );

        assert_eq!(
            BackendId::ScalarRust,
            select_aarch64_backend(Aarch64FeatureSet {
                neon: false,
                sve: false,
            })
            .id
        );
    }

    #[cfg(all(
        feature = "simd-neon",
        target_arch = "aarch64",
        not(target_env = "msvc"),
        not(any(target_os = "android", target_os = "ios")),
        feature = "std"
    ))]
    #[test]
    fn test_select_aarch64_backend_sve_placeholder_does_not_change_current_priority() {
        assert_eq!(
            BackendId::RustNeon,
            select_aarch64_backend(Aarch64FeatureSet {
                neon: true,
                sve: true,
            })
            .id
        );

        assert_eq!(
            BackendId::ScalarRust,
            select_aarch64_backend(Aarch64FeatureSet {
                neon: false,
                sve: true,
            })
            .id
        );
    }
}
