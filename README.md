# rustfs-erasure-codec

[![CI](https://github.com/houseme/reed-solomon-erasure/actions/workflows/ci.yml/badge.svg)](https://github.com/houseme/reed-solomon-erasure/actions/workflows/ci.yml)
[![Crates](https://img.shields.io/crates/v/rustfs-erasure-codec.svg)](https://crates.io/crates/rustfs-erasure-codec)
[![Documentation](https://docs.rs/rustfs-erasure-codec/badge.svg)](https://docs.rs/rustfs-erasure-codec)
[![dependency status](https://deps.rs/repo/github/houseme/reed-solomon-erasure/status.svg)](https://deps.rs/repo/github/houseme/reed-solomon-erasure)
[![Crates.io Total Downloads](https://img.shields.io/crates/d/rustfs-erasure-codec)](https://crates.io/crates/rustfs-erasure-codec)
[![Crates.io License](https://img.shields.io/crates/l/rustfs-erasure-codec)](https://crates.io/crates/rustfs-erasure-codec)
[![Crates.io Version](https://img.shields.io/crates/v/rustfs-erasure-codec)](https://crates.io/crates/rustfs-erasure-codec)

English | [Chinese](README_CN.md)

Rust implementation of Reed-Solomon erasure coding.

This repository currently contains the modernized Rust 2024 codebase with:

- classic Reed-Solomon over `GF(2^8)` and `GF(2^16)`
- runtime-dispatched SIMD backends for `galois_8`
- multiple classic matrix modes
- Leopard codec families for high shard-count workflows
- low-allocation verification and reconstruction helpers
- streaming APIs, benchmarks, and release-validation scripts

WASM bindings are available under [wasm/README.md](wasm/README.md).

## Highlights

- `galois_8::ReedSolomon` is the main optimized path and supports runtime backend selection.
- `galois_16::ReedSolomon` remains available for classic `GF(2^16)` workflows.
- `CodecOptions` lets you choose codec family, matrix mode, cache behavior, and parallel policy.
- `VerifyWorkspace` and `ShardSlot<T>` help reduce repeated allocation in hot paths.
- `decode_idx(...)` and `reconstruct_some(...)` support incremental or targeted recovery for classic codecs.
- `stream::StreamOptions` enables block-based encode, verify, and reconstruct for large files.

## Install

Add the crate:

```toml
[dependencies]
rustfs-erasure-codec = "7.0.0"
```

Enable SIMD acceleration when you care about throughput:

```toml
[dependencies]
rustfs-erasure-codec = { version = "7.0.0", features = ["simd-accel"] }
```

You can also enable only the backend family you need:

```toml
[dependencies]
rustfs-erasure-codec = { version = "7.0.0", features = ["simd-neon"] }   # aarch64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-ssse3"] } # x86_64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-avx2"] }  # x86_64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-avx512"] }# x86_64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-gfni"] }  # x86_64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-vsx"] }   # powerpc64
```

Notes:

- `simd-accel` is an umbrella feature that enables all Rust/C SIMD backends.
- Runtime dispatch is safe: unsupported ISAs fall back to scalar execution.
- `std` is enabled by default. Disable default features for `no_std` usage.

## Quick Start

```rust
use rustfs_erasure_codec::galois_8::ReedSolomon;
use rustfs_erasure_codec::VerifyWorkspace;

fn main() {
    let rs = ReedSolomon::new(3, 2).unwrap();

    let mut shards = vec![
        vec![0, 1, 2, 3],
        vec![4, 5, 6, 7],
        vec![8, 9, 10, 11],
        vec![0, 0, 0, 0],
        vec![0, 0, 0, 0],
    ];

    rs.encode(&mut shards).unwrap();

    let original = shards.clone();
    let mut missing: Vec<Option<Vec<u8>>> = shards.into_iter().map(Some).collect();
    missing[0] = None;
    missing[4] = None;

    rs.reconstruct(&mut missing).unwrap();

    let rebuilt: Vec<Vec<u8>> = missing.into_iter().map(|shard| shard.unwrap()).collect();
    let mut verify_workspace = VerifyWorkspace::new(&rs, rebuilt[0].len());

    assert!(rs.verify_with_workspace(&rebuilt, &mut verify_workspace).unwrap());
    assert_eq!(rebuilt, original);
}
```

For repeated `verify(...)` calls, prefer `verify_with_workspace(...)` or
`verify_with_buffer(...)` to reuse parity scratch space.

## Low-Allocation Helpers

For repeated reconstruct flows, `ShardSlot<T>` avoids reallocating missing shard buffers:

```rust
use rustfs_erasure_codec::galois_8::{ReedSolomon, mark_missing_slots, shards_to_slots};

fn main() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let mut shards = vec![
        vec![0, 1, 2, 3],
        vec![4, 5, 6, 7],
        vec![8, 9, 10, 11],
        vec![12, 13, 14, 15],
        vec![0, 0, 0, 0],
        vec![0, 0, 0, 0],
    ];

    rs.encode(&mut shards).unwrap();

    let mut slots = shards_to_slots(&shards);
    mark_missing_slots(&mut slots, &[1, 5]);
    rs.reconstruct(&mut slots).unwrap();

    assert!(slots[1].is_present());
    assert!(slots[5].is_present());
}
```

For SIMD-sensitive workloads on `galois_8`, you can allocate 64-byte aligned shards with:

- `rustfs_erasure_codec::galois_8::alloc_aligned_shards(...)`
- `galois_8::ReedSolomon::alloc_aligned(...)`

## Codec Families

`CodecOptions::codec_family` selects the algorithm family:

| Family        | Status                                   | Notes                                                                                                                                                                                                                |
|---------------|------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Classic`     | fully supported                          | Default family. Supports `update`, `encode_single`, `decode_idx`, `reconstruct_some`, and matrix-mode selection.                                                                                                     |
| `LeopardGF8`  | supported for `galois_8` workloads       | FFT-based codec over `GF(2^8)`. Requires shard lengths that are multiples of 64 bytes and supports up to 256 total shards. Classic-only APIs such as `update`, `encode_single*`, and `decode_idx` are not supported. |
| `LeopardGF16` | supported for high shard-count workflows | FFT-based codec family for large total shard counts. Classic-only APIs such as `update`, `encode_single*`, and `decode_idx` are not supported.                                                                       |

Example:

```rust
use rustfs_erasure_codec::galois_8::ReedSolomon;
use rustfs_erasure_codec::{CodecFamily, CodecOptions};

let rs = ReedSolomon::with_options(
32,
16,
CodecOptions {
codec_family: CodecFamily::LeopardGF8,
..CodecOptions::default ()
},
).unwrap();
```

## Matrix Modes

`CodecOptions::matrix_mode` applies to `CodecFamily::Classic`:

- `Vandermonde`: default classic behavior
- `Cauchy`: alternative coding matrix
- `JerasureLike`: Jerasure-style matrix layout
- `Custom`: explicit matrix rows via `ReedSolomon::with_custom_matrix(...)`

If you need compatibility with existing classic payload expectations, stay on
`MatrixMode::Vandermonde`.

Minimal custom-matrix example:

```rust
use rustfs_erasure_codec::galois_8::ReedSolomon;
use rustfs_erasure_codec::CodecOptions;

let custom_rows = vec![vec![1u8, 1, 1], vec![1u8, 2, 4]];
let rs = ReedSolomon::with_custom_matrix(3, 2, & custom_rows, CodecOptions::default ()).unwrap();
```

## Advanced APIs

### Incremental Recovery

`decode_idx(...)` is available on classic `galois_8::ReedSolomon` and is useful
when inputs arrive over multiple steps instead of in one full reconstruct call.

### Targeted Recovery

`reconstruct_some(...)` reconstructs only the shards you mark as required, which
can reduce unnecessary work when you do not need the full stripe restored.

### Streaming

The streaming API lives under `rustfs_erasure_codec::stream` and is available
with the default `std` feature:

- `encode_stream(...)`
- `verify_stream(...)`
- `reconstruct_stream(...)`

Use it when your data is too large to hold in memory as a full shard matrix.
`StreamOptions` defaults to 4 MiB blocks and lets you tune block size.

## Runtime Backend Control

The main `galois_8` path supports runtime backend inspection and overrides.

Environment variables:

- `RSE_BACKEND_OVERRIDE`: force a backend such as `scalar`, `rust-neon`, `rust-avx2`, `rust-avx512`, `rust-gfni-avx2`,
  `rust-gfni-avx512`, `rust-ssse3`, `rust-vsx`, or `simd-c`
- `RSE_STRICT_BACKEND_OVERRIDE=1`: fail validation if the requested backend cannot be honored
- `RUST_REED_SOLOMON_ERASURE_ARCH`: tune the legacy C backend build target

Public helpers:

- `galois_8::active_backend_name()`
- `galois_8::active_backend_kind()`
- `galois_8::active_backend_id()`

## Tuning Options

`CodecOptions` exposes several useful knobs:

- `fast_one_parity`: enables XOR-only fast path when `parity_shards == 1`
- `inversion_cache`: enables/disables cached decode matrices
- `inversion_cache_capacity`: controls cache sizing explicitly
- `max_parallel_jobs`: caps parallel work for a codec instance

You can also tune parallel policy through environment variables:

- `RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES`
- `RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB`
- `RS_PARALLEL_POLICY_MAX_JOBS`

## Benchmarks And Validation

Common workflows:

```bash
# Run tests
cargo test

# Run benches
cargo bench --features simd-accel

# Run the release validation profile
bash scripts/release-check.sh

# Run the extended validation profile
VALIDATION_PROFILE=extended bash scripts/release-check.sh

# Collect x86_64 SIMD benchmark artifacts
bash scripts/collect_x86_simd_benchmarks.sh
```

Useful references:

- [docs/benchmark-methodology.md](docs/benchmark-methodology.md)
- [docs/README-performance-index.md](docs/README-performance-index.md)
- [scripts/README.md](scripts/README.md)
- [docs/README.md](docs/README.md)

## Provenance

Versions `0.9.0` through `6.0.0` were originally created by
[Darren Ldl](https://github.com/darrenldl) and later maintained by the
[rust-rse](https://github.com/rust-rse) community. The current codebase in this
repository includes the Rust 2024 rewrite and the newer SIMD/Leopard work
maintained by [houseme](https://github.com/houseme).

## Contributing

Contributions are welcome. Please include focused validation for the area you
touch, especially for backend-specific or benchmark-sensitive changes.

## License

This project is released under the MIT License. See [LICENSE](LICENSE).

The bundled `simd_c` sources are derived from
[Nicolas Trangez's Haskell implementation](https://github.com/NicolasT/reedsolomon)
and remain under the MIT License as well.
