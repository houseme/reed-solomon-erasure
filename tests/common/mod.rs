#[cfg(test)]
use reed_solomon_erasure::galois_8::active_backend_name;

#[cfg(test)]
fn expected_backend_name(override_value: &str) -> Option<&str> {
    match override_value {
        "auto" => None,
        "scalar" | "scalar-rust" => Some("scalar-rust"),
        "simd-c" => Some("simd-c"),
        "rust-neon" => Some("rust-neon"),
        "rust-ssse3" => Some("rust-ssse3"),
        "rust-avx2" => Some("rust-avx2"),
        "rust-avx512" => Some("rust-avx512"),
        "rust-gfni-avx2" => Some("rust-gfni-avx2"),
        "rust-gfni-avx512" => Some("rust-gfni-avx512"),
        _ => None,
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub fn override_honored() -> bool {
    let override_value =
        std::env::var("RSE_BACKEND_OVERRIDE").unwrap_or_else(|_| "auto".to_string());
    match expected_backend_name(override_value.trim()) {
        Some(expected) => active_backend_name() == expected,
        None => true,
    }
}

#[cfg(test)]
pub fn assert_backend_override_honored_if_strict() {
    if std::env::var_os("RSE_STRICT_BACKEND_OVERRIDE").is_none() {
        return;
    }

    let override_value =
        std::env::var("RSE_BACKEND_OVERRIDE").unwrap_or_else(|_| "auto".to_string());
    if let Some(expected) = expected_backend_name(override_value.trim()) {
        let actual = active_backend_name();
        assert_eq!(
            expected, actual,
            "requested backend override '{}' was not honored; actual backend was '{}'",
            override_value, actual
        );
    }
}
