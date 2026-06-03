/// Selects the codec algorithm family for encoding and reconstruction.
///
/// - [`Classic`](CodecFamily::Classic): Standard Reed-Solomon over GF(2^8) using
///   Vandermonde/Cauchy matrix multiplication. Supports all shard counts up to
///   `Field::ORDER`, incremental `update`, and `encode_single`.
///
/// - [`LeopardGF8`](CodecFamily::LeopardGF8): FFT-based Leopard codec over GF(2^8).
///   Uses Fermat-number FFT with Forney syndrome decoding. Requires shard lengths
///   that are multiples of 64 bytes. Does **not** support `encode_single` or `update`.
///   Supports up to 256 total shards (data + parity).
///
/// - [`LeopardGF16`](CodecFamily::LeopardGF16): FFT-based Leopard codec over GF(2^16).
///   Supports up to 65536 total shards (data + parity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecFamily {
    Classic,
    LeopardGF8,
    LeopardGF16,
}

/// Selects the encoding matrix construction strategy for [`CodecFamily::Classic`].
///
/// This option is ignored for Leopard families (they use FFT-based encoding).
///
/// - [`Vandermonde`](MatrixMode::Vandermonde): Standard Vandermonde matrix.
/// - [`Cauchy`](MatrixMode::Cauchy): Cauchy matrix construction.
/// - [`JerasureLike`](MatrixMode::JerasureLike): Jerasure-compatible matrix layout.
/// - [`Custom`](MatrixMode::Custom): User-supplied matrix via
///   [`ReedSolomon::with_custom_matrix`](crate::ReedSolomon::with_custom_matrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixMode {
    Vandermonde,
    Cauchy,
    JerasureLike,
    Custom,
}

/// Configuration options for constructing a [`ReedSolomon`](crate::ReedSolomon) codec.
///
/// Use `CodecOptions::default()` for sensible defaults, or the builder methods:
///
/// ```ignore
/// use reed_solomon_erasure::core::{CodecOptions, CodecFamily};
///
/// let opts = CodecOptions::builder()
///     .codec_family(CodecFamily::LeopardGF8)
///     .fast_one_parity(true)
///     .build();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodecOptions {
    /// When `true` and parity shard count is 1, use XOR-only fast path instead of
    /// full matrix multiplication. Default: `false`.
    pub fast_one_parity: bool,
    /// When `true`, cache the inverted decode matrix for repeated reconstruction
    /// with the same erasure pattern. Default: `true`.
    pub inversion_cache: bool,
    /// Capacity of the inversion cache (LRU). `0` means automatic sizing based on
    /// shard counts. Default: `0`.
    pub inversion_cache_capacity: usize,
    /// The codec algorithm family to use. Default: [`CodecFamily::Classic`].
    pub codec_family: CodecFamily,
    /// The matrix construction strategy (only used for [`CodecFamily::Classic`]).
    /// Default: [`MatrixMode::Vandermonde`].
    pub matrix_mode: MatrixMode,
    /// Maximum number of parallel jobs for encode/decode operations.
    /// `0` means automatic (uses `available_parallelism()`). Default: `0`.
    pub max_parallel_jobs: usize,
}

impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            fast_one_parity: false,
            inversion_cache: true,
            inversion_cache_capacity: 0,
            codec_family: CodecFamily::Classic,
            matrix_mode: MatrixMode::Vandermonde,
            max_parallel_jobs: 0,
        }
    }
}

/// Builder for [`CodecOptions`].
///
/// Created via [`CodecOptions::builder()`]. All methods chain; call [`build()`](CodecOptionsBuilder::build) to obtain the final `CodecOptions`.
#[derive(Debug, Clone, Copy)]
pub struct CodecOptionsBuilder {
    options: CodecOptions,
}

impl CodecOptions {
    /// Create a new builder with default options.
    pub fn builder() -> CodecOptionsBuilder {
        CodecOptionsBuilder {
            options: CodecOptions::default(),
        }
    }
}

impl CodecOptionsBuilder {
    /// Set the codec algorithm family.
    pub fn codec_family(mut self, family: CodecFamily) -> Self {
        self.options.codec_family = family;
        self
    }

    /// Set the matrix construction strategy.
    pub fn matrix_mode(mut self, mode: MatrixMode) -> Self {
        self.options.matrix_mode = mode;
        self
    }

    /// Enable or disable the XOR-only fast path for single parity shards.
    pub fn fast_one_parity(mut self, enabled: bool) -> Self {
        self.options.fast_one_parity = enabled;
        self
    }

    /// Enable or disable the inversion cache.
    pub fn inversion_cache(mut self, enabled: bool) -> Self {
        self.options.inversion_cache = enabled;
        self
    }

    /// Set the inversion cache capacity. `0` for automatic sizing.
    pub fn inversion_cache_capacity(mut self, capacity: usize) -> Self {
        self.options.inversion_cache_capacity = capacity;
        self
    }

    /// Set the maximum number of parallel jobs. `0` for automatic.
    pub fn max_parallel_jobs(mut self, jobs: usize) -> Self {
        self.options.max_parallel_jobs = jobs;
        self
    }

    /// Build the final [`CodecOptions`].
    pub fn build(self) -> CodecOptions {
        self.options
    }
}
