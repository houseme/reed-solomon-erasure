//! Reserved aarch64 SVE backend slot.
//!
//! This file intentionally does not provide an active implementation yet.
//! Its purpose is to make the aarch64 backend layout explicit so that a future
//! SVE backend can be added without reworking the NEON-oriented module split.

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub(crate) struct SveFeatureSet {
    pub available: bool,
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
pub(crate) fn detect_sve_features() -> SveFeatureSet {
    // SVE detection and backend enablement are intentionally deferred until a
    // concrete implementation is ready. Keeping this stub localized makes the
    // future extension point explicit without changing current runtime behavior.
    SveFeatureSet { available: false }
}

#[cfg(all(
    test,
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios")),
    feature = "std"
))]
mod tests {
    use super::{SveFeatureSet, detect_sve_features};

    #[test]
    fn test_detect_sve_features_stub_reports_unavailable() {
        assert_eq!(detect_sve_features(), SveFeatureSet { available: false });
    }

    #[test]
    fn test_sve_feature_set_default_matches_stub_contract() {
        assert_eq!(SveFeatureSet::default(), detect_sve_features());
    }
}
