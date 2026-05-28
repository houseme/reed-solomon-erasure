# reed-solomon-erasure
[![CI](https://github.com/darrenldl/reed-solomon-erasure/actions/workflows/ci.yml/badge.svg)](https://github.com/darrenldl/reed-solomon-erasure/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/darrenldl/reed-solomon-erasure/branch/master/graph/badge.svg)](https://codecov.io/gh/darrenldl/reed-solomon-erasure)
[![Coverage Status](https://coveralls.io/repos/github/darrenldl/reed-solomon-erasure/badge.svg?branch=master)](https://coveralls.io/github/darrenldl/reed-solomon-erasure?branch=master)
[![Crates](https://img.shields.io/crates/v/reed-solomon-erasure.svg)](https://crates.io/crates/reed-solomon-erasure)
[![Documentation](https://docs.rs/reed-solomon-erasure/badge.svg)](https://docs.rs/reed-solomon-erasure)
[![dependency status](https://deps.rs/repo/github/darrenldl/reed-solomon-erasure/status.svg)](https://deps.rs/repo/github/darrenldl/reed-solomon-erasure)

Rust implementation of Reed-Solomon erasure coding

CI has been migrated to a unified GitHub Actions workflow (`.github/workflows/ci.yml`) that includes test, build, security, typos, and tag-gated publish stages.

WASM builds are also available, see section **WASM usage** below for details

This is a port of [BackBlaze's Java implementation](https://github.com/Backblaze/JavaReedSolomon), [Klaus Post's Go implementation](https://github.com/klauspost/reedsolomon), and [Nicolas Trangez's Haskell implementation](https://github.com/NicolasT/reedsolomon).

Version `1.X.X` copies BackBlaze's implementation, and is less performant as there were fewer places where parallelism could be added.

Version `>= 2.0.0` copies Klaus Post's implementation. The SIMD C code is copied from Nicolas Trangez's implementation with minor modifications.

See [Notes](#notes) and [License](#license) section for details.

## WASM usage

See [here](wasm/README.md) for details

## Rust usage
Add the following to your `Cargo.toml` for the normal version (pure Rust version)
```toml
[dependencies]
reed-solomon-erasure = "4.0"
```
or the following for the version which tries to utilise SIMD
```toml
[dependencies]
reed-solomon-erasure = { version = "4.0", features = [ "simd-accel" ] }
```
and the following to your crate root
```rust
extern crate reed_solomon_erasure;
```

NOTE: `simd-accel` now prefers Rust runtime-dispatched SIMD backends on supported CPUs. The bundled `simd_c`
implementation is retained as a legacy fallback. Set environment variable `RUST_REED_SOLOMON_ERASURE_ARCH` during
build if you explicitly want to compile the legacy C backend for a specific architecture (`-march` flag in GCC/Clang).
For example, setting it to `native` may improve the legacy C path on the local machine, but it will stop running on
older CPUs, YMMV.

## Example
```rust
#[macro_use(shards)]
extern crate reed_solomon_erasure;

use reed_solomon_erasure::galois_8::ReedSolomon;
use reed_solomon_erasure::VerifyWorkspace;
// or use the following for Galois 2^16 backend
// use reed_solomon_erasure::galois_16::ReedSolomon;

fn main () {
    let r = ReedSolomon::new(3, 2).unwrap(); // 3 data shards, 2 parity shards

    let mut master_copy = shards!(
        [0, 1,  2,  3],
        [4, 5,  6,  7],
        [8, 9, 10, 11],
        [0, 0,  0,  0], // last 2 rows are parity shards
        [0, 0,  0,  0]
    );

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

For SIMD-sensitive workloads on `galois_8`, you can allocate 64-byte aligned shards with
`reed_solomon_erasure::galois_8::alloc_aligned_shards(...)` or `galois_8::ReedSolomon::alloc_aligned(...)`.
These helpers return `AlignedShard` buffers that implement `AsRef<[u8]>` and `AsMut<[u8]>`, so they can be passed
directly to the existing encode/verify APIs without changing codec output semantics.

## Codec Families

`CodecOptions::codec_family` makes the algorithm family explicit:

- `CodecFamily::Classic`
  - default family
  - preserves the crate's current classic behavior and compatibility assumptions
- `CodecFamily::LeopardGF8`
  - reserved for an explicit high-shard-count Leopard GF(2^8) family
  - can now be constructed as an explicit internal-family prototype
  - exposes setup metadata such as `leopard_setup_matrix_shape()`
  - `encode(...)`, `encode_sep(...)`, and `encode_opt(...)` are now wired to an explicit family-specific prototype path
  - `verify`, `reconstruct`, `update`, and `decode_idx` still return `Error::UnsupportedLeopardPrototype`
- `CodecFamily::LeopardGF16`
  - reserved for a future Leopard GF(2^16) family
  - currently exposed only as a prototype skeleton and returns `Error::UnsupportedLeopardPrototype`

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
Version `1.X.X`, `2.0.0` do not utilise SIMD.

Version `2.1.0` onward uses Nicolas's C files for SIMD operations.

Machine: laptop with `Intel(R) Core(TM) i5-3337U CPU @ 1.80GHz (max 2.70GHz) 2 Cores 4 Threads`

Below shows the result of one of the test configurations, other configurations show similar results in terms of ratio.

|Configuration| Klaus Post's | >= 2.1.0 && < 4.0.0 | 2.0.X | 1.X.X |
|---|---|---|---|---|
| 10x2x1M | ~7800MB/s |~4500MB/s | ~1000MB/s | ~240MB/s |

Versions `>= 4.0.0` have not been benchmarked thoroughly yet

## Benchmarking
You can run benchmarks via `cargo bench`. To enable simd acceleration during benchmarks use `cargo bench --features simd-accel`.

## Changelog
[Changelog](CHANGELOG.md)

## Contributions
Contributions are welcome. Note that by submitting contributions, you agree to license your work under the same license used by this project as stated in the LICENSE file.

## Credits
#### Library overhaul and Galois 2^16 backend
Many thanks to the following people for overhaul of the library and introduction of Galois 2^16 backend

  - [@drskalman](https://github.com/drskalman)

  - Jeff Burdges [@burdges](https://github.com/burdges)

  - Robert Habermeier [@rphmeier](https://github.com/rphmeier)

#### WASM builds
Many thanks to Nazar Mokrynskyi [@nazar-pc](https://github.com/nazar-pc) for submitting his package for WASM builds

He is the original author of the files stored in `wasm` folder. The files may have been modified by me later.

#### AVX512 support
Many thanks to [@sakridge](https://github.com/sakridge) for adding support for AVX512 (see [PR #69](https://github.com/darrenldl/reed-solomon-erasure/pull/69))

#### build.rs improvements
Many thanks to [@ryoqun](https://github.com/ryoqun) for improving the usability of the library in the context of cross-compilation (see [PR #75](https://github.com/darrenldl/reed-solomon-erasure/pull/75))

#### no_std support
Many thanks to Nazar Mokrynskyi [@nazar-pc](https://github.com/nazar-pc) for adding `no_std` support (see [PR #90](https://github.com/darrenldl/reed-solomon-erasure/pull/90))

#### Testers
Many thanks to the following people for testing and benchmarking on various platforms

  - Laurențiu Nicola [@lnicola](https://github.com/lnicola/) (platforms: Linux, Intel)

  - Roger Andersen [@hexjelly](https://github.com/hexjelly) (platforms: Windows, AMD)

## Notes
#### Code quality review
If you'd like to evaluate the quality of this library, you may find audit comments helpful.

Simply search for "AUDIT" to see the dev notes that are aimed at facilitating code reviews.

#### Implementation notes
The `1.X.X` implementation mostly copies [BackBlaze's Java implementation](https://github.com/Backblaze/JavaReedSolomon).

`2.0.0` onward mostly copies [Klaus Post's Go implementation](https://github.com/klauspost/reedsolomon), and copies C files from [Nicolas Trangez's Haskell implementation](https://github.com/NicolasT/reedsolomon).

The test suite for all versions copies [Klaus Post's Go implementation](https://github.com/klauspost/reedsolomon) as basis.

## License
#### Nicolas Trangez's Haskell Reed-Solomon implementation
The C files for SIMD operations are copied (with no/minor modifications) from [Nicolas Trangez's Haskell implementation](https://github.com/NicolasT/reedsolomon), and are under the same MIT License as used by NicolasT's project

#### TL;DR
All files are released under the MIT License
