# reed-solomon-erasure

[![CI](https://github.com/houseme/reed-solomon-erasure/actions/workflows/ci.yml/badge.svg)](https://github.com/houseme/reed-solomon-erasure/actions/workflows/ci.yml)
[![Crates](https://img.shields.io/crates/v/reed-solomon-erasure.svg)](https://crates.io/crates/reed-solomon-erasure)
[![Documentation](https://docs.rs/reed-solomon-erasure/badge.svg)](https://docs.rs/reed-solomon-erasure)
[![dependency status](https://deps.rs/repo/github/houseme/reed-solomon-erasure/status.svg)](https://deps.rs/repo/github/houseme/reed-solomon-erasure)

Rust implementation of Reed-Solomon erasure coding

> **Provenance:** Versions 0.9.0–6.0.0 were created by [Darren Ldl](https://github.com/darrenldl) (2017–2021) and maintained by the [rust-rse](https://github.com/rust-rse) community (2021–2022). Versions >6.0.0 are developed by [houseme](https://github.com/houseme) (2026–present), featuring a Rust 2024 edition rewrite with runtime SIMD dispatch, Leopard-GF8 codec, and NEON/x86 SIMD backends.

CI has been migrated to a unified GitHub Actions workflow (`.github/workflows/ci.yml`) that includes test, build, security, typos, and tag-gated publish stages.

WASM builds are also available, see section **WASM usage** below for details.

This is a port of [BackBlaze's Java implementation](https://github.com/Backblaze/JavaReedSolomon), [Klaus Post's Go implementation](https://github.com/klauspost/reedsolomon), and [Nicolas Trangez's Haskell implementation](https://github.com/NicolasT/reedsolomon).

Version `1.X.X` copies BackBlaze's implementation, and is less performant as there were fewer places where parallelism could be added.

Version `>= 2.0.0` copies Klaus Post's implementation. The SIMD C code is copied from Nicolas Trangez's implementation with minor modifications.

See [Notes](#notes) and [License](#license) section for details.

## WASM usage

See [here](wasm/README.md) for details.

## Rust usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
reed-solomon-erasure = "6.0"
```

Or to enable SIMD acceleration (recommended for performance-sensitive workloads):

```toml
# Enable all SIMD backends (umbrella feature — recommended for most users)
[dependencies]
reed-solomon-erasure = { version = "6.0", features = ["simd-accel"] }

# Or enable only the backend(s) matching your target architecture:
# [dependencies]
# reed-solomon-erasure = { version = "6.0", features = ["simd-neon"] }    # aarch64 NEON
# reed-solomon-erasure = { version = "6.0", features = ["simd-avx2"] }   # x86_64 AVX2
# reed-solomon-erasure = { version = "6.0", features = ["simd-ssse3"] }  # x86_64 SSSE3
# reed-solomon-erasure = { version = "6.0", features = ["simd-avx512"] } # x86_64 AVX-512
# reed-solomon-erasure = { version = "6.0", features = ["simd-gfni"] }   # x86_64 GFNI (requires AVX2/AVX-512)
```

> **Note:** The `simd-accel` feature enables all SIMD backends and is recommended for most users. For cross-compilation or minimal binary size, enable only the feature matching your target architecture. All features use runtime detection — enabling `simd-avx2` on a machine without AVX2 will safely fall back to scalar code.
>
> The bundled `simd_c` implementation is retained as a legacy fallback when `simd-accel` (or any individual SIMD feature) is enabled.
>
> Set `RSE_BACKEND_OVERRIDE` at runtime to force a specific backend (e.g., `scalar`, `rust-neon`, `rust-avx2`, `rust-gfni-avx2`), or `RUST_REED_SOLOMON_ERASURE_ARCH` at build time to configure the legacy C backend's `-march` flag.

## Example

```rust
use reed_solomon_erasure::galois_8::ReedSolomon;
use reed_solomon_erasure::VerifyWorkspace;
// or use the following for Galois 2^16 backend
// use reed_solomon_erasure::galois_16::ReedSolomon;

fn main() {
    let r = ReedSolomon::new(3, 2).unwrap(); // 3 data shards, 2 parity shards

    let mut master_copy = vec![
        vec![0, 1,  2,  3],
        vec![4, 5,  6,  7],
        vec![8, 9, 10, 11],
        vec![0, 0,  0,  0], // last 2 rows are parity shards
        vec![0, 0,  0,  0],
    ];

    // Construct the parity shards
    r.encode(&mut master_copy).unwrap();

    // Make a copy and transform it into option shards arrangement
    // for feeding into reconstruct_shards
    let mut shards: Vec<_> = master_copy.iter().cloned().map(Some).collect();

    // We can remove up to 2 shards, which may be data or parity shards
    shards[0] = None;
    shards[4] = None;

    // Try to reconstruct missing shards
    r.reconstruct(&mut shards).unwrap();

    // Convert back to normal shard arrangement
    let result: Vec<_> = shards.into_iter().filter_map(|x| x).collect();

    let mut verify_workspace = VerifyWorkspace::new(&r, master_copy[0].len());
    assert!(r.verify_with_workspace(&result, &mut verify_workspace).unwrap());
    assert_eq!(master_copy, result);
}
```

For repeated verify calls, prefer `verify_with_workspace` or `verify_with_buffer`
over plain `verify`, so the parity scratch buffer can be reused across calls.

### LeopardGF8 Example

```rust
use reed_solomon_erasure::galois_8::ReedSolomon;
use reed_solomon_erasure::{CodecOptions, CodecFamily};

fn main() {
    // LeopardGF8 uses FFT-based encoding for high shard counts.
    // Shard lengths must be multiples of 64 bytes.
    let r = ReedSolomon::with_options(
        4, 4,
        CodecOptions {
            codec_family: CodecFamily::LeopardGF8,
            ..CodecOptions::default()
        },
    ).unwrap();

    let shard_len = 256; // must be multiple of 64
    let mut shards: Vec<Vec<u8>> = (0..8)
        .map(|i| (0..shard_len).map(|j| (i * shard_len + j) as u8).collect())
        .collect();

    // Encode: data shards 0..4 produce parity shards 4..7
    let (data, parity) = shards.split_at_mut(4);
    let data_refs: Vec<&[u8]> = data.iter().map(|s| s.as_slice()).collect();
    let mut parity_refs: Vec<&mut [u8]> = parity.iter_mut().map(|s| s.as_mut_slice()).collect();
    r.encode_sep(&data_refs, &mut parity_refs).unwrap();

    // Verify
    let all_refs: Vec<&[u8]> = shards.iter().map(|s| s.as_slice()).collect();
    assert!(r.verify(&all_refs).unwrap());

    // Reconstruct with missing shards
    let mut reconstructable: Vec<Option<Vec<u8>>> =
        shards.iter().map(|s| Some(s.clone())).collect();
    reconstructable[0] = None;
    reconstructable[5] = None;
    r.reconstruct(&mut reconstructable).unwrap();

    // Verify reconstruction
    let recovered: Vec<Vec<u8>> = reconstructable.into_iter().map(|s| s.unwrap()).collect();
    assert_eq!(recovered[0], shards[0]);
    assert_eq!(recovered[5], shards[5]);
}
```

For SIMD-sensitive workloads on `galois_8`, you can allocate 64-byte aligned shards with
`reed_solomon_erasure::galois_8::alloc_aligned_shards(...)` or `galois_8::ReedSolomon::alloc_aligned(...)`.
These helpers return `AlignedShard` buffers that implement `AsRef<[u8]>` and `AsMut<[u8]>`, so they can be passed
directly to the existing encode/verify APIs without changing codec output semantics.

### Streaming API

For large datasets that don't fit in memory, use the streaming API to process data in blocks:

```rust
use reed_solomon_erasure::galois_8::ReedSolomon;
use reed_solomon_erasure::stream::StreamOptions;
use std::io::Cursor;

fn main() {
    let rs = ReedSolomon::new(3, 2).unwrap();
    let shard_size = 64 * 1024; // 64 KiB

    // Prepare data shards as readers.
    let data: Vec<Vec<u8>> = (0..3).map(|i| vec![i as u8; shard_size]).collect();
    let mut readers: Vec<&[u8]> = data.iter().map(|d| d.as_slice()).collect();
    let mut parity_writers: Vec<Vec<u8>> = vec![Vec::new(); 2];

    // Encode in blocks (default 4 MiB block size).
    let opts = StreamOptions::new().with_block_size(256 * 1024); // 256 KiB blocks
    rs.encode_stream(&mut readers, &mut parity_writers, &opts).unwrap();

    // Verify all shards in blocks.
    let mut all_readers: Vec<&[u8]> = Vec::new();
    for d in &data { all_readers.push(d.as_slice()); }
    for p in &parity_writers { all_readers.push(p.as_slice()); }
    assert!(rs.verify_stream(&mut all_readers, &opts).unwrap());

    // Reconstruct with missing shard 0.
    // Present shards: Cursor with data; missing shards: empty Cursor.
    let mut shards: Vec<Cursor<Vec<u8>>> = vec![
        Cursor::new(Vec::new()),               // missing
        Cursor::new(data[1].clone()),
        Cursor::new(data[2].clone()),
        Cursor::new(parity_writers[0].clone()),
        Cursor::new(parity_writers[1].clone()),
    ];
    rs.reconstruct_stream(&mut shards, &opts).unwrap();
    assert_eq!(shards[0].get_ref(), &data[0]);
}
```

The streaming API is available on `galois_8::ReedSolomon` (classic family only) and requires the `std` feature.

## Codec Families

`CodecOptions::codec_family` makes the algorithm family explicit:

- `CodecFamily::Classic`
  - default family
  - preserves the crate's current classic behavior and compatibility assumptions
- `CodecFamily::LeopardGF8`
  - FFT-based Leopard codec over GF(2^8) for high-shard-count scenarios
  - supports up to 256 total shards (data + parity)
  - requires shard lengths that are multiples of 64 bytes
  - fully functional: `encode`, `encode_sep`, `encode_opt`, `verify`, `reconstruct`, `reconstruct_data`, `reconstruct_some` (and their `_opt`/`_par` variants)
  - does **not** support `encode_single`, `encode_single_sep`, `update`, or `decode_idx` (these return `Error::UnsupportedCodecFamily`)
- `CodecFamily::LeopardGF16`
  - FFT-based Leopard codec over GF(2^16)
  - supports up to 65536 total shards (data + parity)
  - requires shard lengths that are multiples of 64 bytes
  - fully functional: `encode`, `encode_sep`, `encode_opt`, `verify`, `reconstruct`, `reconstruct_data`, `reconstruct_some` (and their `_opt`/`_par` variants)
  - does **not** support `encode_single`, `encode_single_sep`, `update`, or `decode_idx` (these return `Error::UnsupportedCodecFamily`)

This means classic users do not silently switch families. Any future Leopard use will stay opt-in.

## Matrix Modes

`CodecOptions::matrix_mode` now supports multiple real matrix families:

- `MatrixMode::Vandermonde`
  - default mode
  - preserves the crate's classic output behavior
- `MatrixMode::Cauchy`
  - alternative coding matrix
  - output is not compatible with the default mode
- `MatrixMode::JerasureLike`
  - alternative coding matrix inspired by Jerasure-style layout
  - output is not compatible with the default mode
- `MatrixMode::Custom`
  - requires explicit parity rows through `ReedSolomon::with_custom_matrix(...)`
  - `with_options(... MatrixMode::Custom ...)` without matrix payload returns `Error::InvalidCustomMatrix`

If you need compatibility with existing classic Reed-Solomon payloads or MinIO-oriented classic output expectations,
stay on `MatrixMode::Vandermonde`.

Minimal `with_custom_matrix(...)` example:

```rust
use reed_solomon_erasure::{CodecOptions, galois_8::ReedSolomon};

// Example custom parity rows for 3 data shards and 2 parity shards.
// These rows define the parity part of the generator matrix.
let custom_rows = vec![vec![1u8, 1, 1], vec![1u8, 2, 4]];

let custom = ReedSolomon::with_custom_matrix(3, 2, &custom_rows, CodecOptions::default()).unwrap();

let mut shards = vec![
    vec![1u8, 2, 3, 4],
    vec![5u8, 6, 7, 8],
    vec![9u8, 10, 11, 12],
    vec![0u8; 4],
    vec![0u8; 4],
];
custom.encode(&mut shards).unwrap();
assert!(custom.verify(&shards).unwrap());
```

## Progressive Decode

For multi-step recovery flows, `decode_idx(...)` lets you accumulate reconstruction contributions as input shards
arrive instead of requiring a one-shot `reconstruct(...)` call:

```rust
use reed_solomon_erasure::galois_8::ReedSolomon;

let rs = ReedSolomon::new(5, 3).unwrap();

let mut shards = vec![
    vec![1u8, 2, 3, 4],
    vec![5u8, 6, 7, 8],
    vec![9u8, 10, 11, 12],
    vec![13u8, 14, 15, 16],
    vec![17u8, 18, 19, 20],
    vec![0u8; 4],
    vec![0u8; 4],
    vec![0u8; 4],
];
rs.encode(&mut shards).unwrap();

// Reconstruct shards 1 and 4 incrementally.
let mut dst = vec![None; 8];
dst[1] = Some(vec![0u8; 4]);
dst[4] = Some(vec![0u8; 4]);

// Positions marked `true` are expected to arrive as input across calls.
let expect_input = vec![true, false, true, true, false, true, true, false];

let mut first_input = vec![None; 8];
first_input[0] = Some(shards[0].clone());
first_input[2] = Some(shards[2].clone());
rs.decode_idx(&mut dst, Some(&expect_input), &first_input).unwrap();

let mut second_input = vec![None; 8];
second_input[3] = Some(shards[3].clone());
second_input[5] = Some(shards[5].clone());
second_input[6] = Some(shards[6].clone());
rs.decode_idx(&mut dst, Some(&expect_input), &second_input).unwrap();

assert_eq!(dst[1].as_deref(), Some(shards[1].as_slice()));
assert_eq!(dst[4].as_deref(), Some(shards[4].as_slice()));
```

`decode_idx(...)` also supports a merge mode by passing `expect_input = None`, which XOR-accumulates another partial
decode result into the destination buffers.

## Benchmark it yourself

You can test performance under different configurations quickly (e.g. data parity shards ratio, parallel parameters)
with standard `cargo bench` command.

## Performance

Versions 1.X.X and 2.0.0 do not utilise SIMD. Version 2.1.0 onward uses Nicolas's C files for SIMD operations.

Versions >= 4.0.0 have been substantially re-architectured with runtime SIMD dispatch. For detailed benchmark methodology and results, see [`docs/benchmark-methodology.md`](docs/benchmark-methodology.md).

### Reference Benchmarks

| Platform | CPU | Backend | Encode 10x2x1M | Reconstruct 10x2x1M |
|---|---|---|---|---|
| x86_64 | AMD EPYC 9V45 | AVX2 | ~12 GB/s | ~10 GB/s |
| aarch64 | Apple M5 Max | NEON | ~10 GB/s | ~8 GB/s |

> These are approximate figures from smoke benchmarks. Actual performance depends on shard count, shard size, and CPU. Run `cargo bench` for precise numbers on your hardware.

## Benchmarking

```bash
# Run all benchmarks
cargo bench

# Enable SIMD acceleration during benchmarks
cargo bench --features simd-accel

# Benchmark with a specific SIMD backend only
cargo bench --features simd-avx2   # x86_64 AVX2 only
cargo bench --features simd-neon   # aarch64 NEON only

# Run benchmark smoke tests (fast profile)
VALIDATION_PROFILE=fast cargo test --test benchmark_smoke

# Run extended smoke tests
VALIDATION_PROFILE=extended cargo test --test benchmark_smoke
```

See [`docs/benchmark-methodology.md`](docs/benchmark-methodology.md) for full details on methodology, profiles, and interpreting results.

## Changelog

[Changelog](CHANGELOG.md)

## Contributions

Contributions are welcome. Note that by submitting contributions, you agree to license your work under the same license used by this project as stated in the LICENSE file.

## Credits

#### Major rewrite (2026)

Many thanks to [houseme](https://github.com/houseme) for the major rewrite of the library:

- Migration to Rust 2024 edition (rust-version 1.95)
- Runtime-dispatched GF(2^8) backend architecture with SIMD backends (SSSE3, AVX2, AVX512, GFNI, NEON)
- Leopard-GF8 codec family implementation
- Comprehensive benchmarking, CI, and release automation infrastructure
- Extensive documentation and design documents

#### Library overhaul and Galois 2^16 backend

Many thanks to the following people for overhaul of the library and introduction of Galois 2^16 backend:

  - [@drskalman](https://github.com/drskalman)

  - Jeff Burdges [@burdges](https://github.com/burdges)

  - Robert Habermeier [@rphmeier](https://github.com/rphmeier)

#### WASM builds

Many thanks to Nazar Mokrynskyi [@nazar-pc](https://github.com/nazar-pc) for submitting his package for WASM builds.

He is the original author of the files stored in `wasm` folder. The files may have been modified by me later.

#### AVX512 support

Many thanks to [@sakridge](https://github.com/sakridge) for adding support for AVX512 (see [PR #69](https://github.com/darrenldl/reed-solomon-erasure/pull/69))

#### build.rs improvements

Many thanks to [@ryoqun](https://github.com/ryoqun) for improving the usability of the library in the context of cross-compilation (see [PR #75](https://github.com/darrenldl/reed-solomon-erasure/pull/75))

#### no_std support

Many thanks to Nazar Mokrynskyi [@nazar-pc](https://github.com/nazar-pc) for adding `no_std` support (see [PR #90](https://github.com/darrenldl/reed-solomon-erasure/pull/90))

#### Testers

Many thanks to the following people for testing and benchmarking on various platforms:

  - Laurențiu Nicola [@lnicola](https://github.com/lnicola/) (platforms: Linux, Intel)

  - Roger Andersen [@hexjelly](https://github.com/hexjelly) (platforms: Windows, AMD)

## Notes

#### Code quality review

If you'd like to evaluate the quality of this library, you may find audit comments helpful.

Simply search for "AUDIT" to see the dev notes that are aimed at facilitating code reviews.

#### Implementation notes

The `1.X.X` implementation mostly copies [BackBlaze's Java implementation](https://github.com/Backblaze/JavaReedSolomon).

`2.0.0` onward mostly copies [Klaus Post's Go implementation](https://github.com/klauspost/reedsolomon), and copies C files from [Nicolas Trangez's Haskell implementation](https://github.com/NicolasT/reedsolomon).

`>= 7.0.0` introduces a runtime-dispatched GF(2^8) backend architecture with Rust SIMD implementations (SSSE3, AVX2, AVX512, GFNI on x86_64; NEON on aarch64), a Leopard-GF8 codec family, multiple matrix modes (Vandermonde, Cauchy, JerasureLike, Custom), and progressive decode via `decode_idx`.

The test suite for all versions copies [Klaus Post's Go implementation](https://github.com/klauspost/reedsolomon) as basis.

## License

#### Nicolas Trangez's Haskell Reed-Solomon implementation

The C files for SIMD operations are copied (with no/minor modifications) from [Nicolas Trangez's Haskell implementation](https://github.com/NicolasT/reedsolomon), and are under the same MIT License as used by NicolasT's project.

#### TL;DR

All files are released under the MIT License.
