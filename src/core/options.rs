#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecFamily {
    Classic,
    LeopardGF8,
    LeopardGF16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixMode {
    Vandermonde,
    Cauchy,
    JerasureLike,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecOptions {
    pub fast_one_parity: bool,
    pub inversion_cache: bool,
    pub inversion_cache_capacity: usize,
    pub codec_family: CodecFamily,
    pub matrix_mode: MatrixMode,
}

impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            fast_one_parity: false,
            inversion_cache: true,
            inversion_cache_capacity: 0,
            codec_family: CodecFamily::Classic,
            matrix_mode: MatrixMode::Vandermonde,
        }
    }
}
