# Plan: SIMD Codegen for Common Encode Configurations

## Context

The current encode path (`encode_sep` -> `code_some_slices` -> `code_single_slice_range`) iterates over data shards and parity rows with per-shard function pointer calls to `mul_slice`/`mul_slice_xor`. For common configurations like 10+4, this generates 40 function pointer calls per chunk, each with its own table load and loop setup overhead.

SIMD codegen generates specialized functions that:
- Unroll the data shard loop at compile time
- Load GF multiplication table pointers once per coefficient (outside the inner loop)
- Eliminate per-shard function pointer dispatch
- Enable better register allocation by the compiler

## Target Configurations

Initial set (sorted by deployment frequency):
- `(10, 4)` — MinIO, HDFS
- `(12, 4)` — HDFS
- `(8, 3)` — MinIO, Ceph
- `(8, 4)` — Ceph
- `(6, 3)` — Ceph
- `(4, 2)` — MinIO, Ceph

## Architecture

### Phase 1: build.rs Code Generator

**File: `build.rs`**

Add `generate_encode_codegen(out_dir, target_arch)` function:
- Only generates for `x86_64` (AVX2) and `aarch64` (NEON)
- Outputs `OUT_DIR/codegen_encode.rs`
- For each config `(D, P)`, generates a function `encode_{D}x{P}_{arch}`

Generated function signature (x86_64 example):
```rust
#[cfg(all(feature = "simd-avx2", target_arch = "x86_64", ...))]
#[target_feature(enable = "avx2")]
pub(crate) unsafe fn encode_10x4_avx2(
    parity_rows: &[&[u8]; 4],   // 4 parity rows, each with 10 coefficients
    data: &[&[u8]; 10],          // 10 data shards
    parity: &mut [&mut [u8]; 4], // 4 parity shards (output)
    shard_len: usize,
)
```

Generated function body:
1. For each parity row `p`, load the 10 coefficient bytes into registers
2. Outer loop: iterate over 32-byte chunks (AVX2 width)
3. For each chunk, load all 10 data shards via `_mm256_loadu_si256`
4. For each parity shard, compute `p[i] = XOR(gf_mul_avx2(d[j], coef[j]))` for j in 0..10
5. Store parity chunks via `_mm256_storeu_si256`
6. Scalar tail for remaining bytes

The `gf_mul_avx2` uses the same nibble-lookup approach as `avx2.rs`:
```rust
// Load table halves for coefficient c
let (low_tbl, high_tbl) = load_tables(c);
// Split input into nibbles
let low = _mm256_and_si256(input, nibble_mask);
let high = _mm256_srli_epi64::<4>(input, nibble_mask);
// Lookup + XOR
_mm256_xor_si256(_mm256_shuffle_epi8(low_tbl, low), _mm256_shuffle_epi8(high_tbl, high))
```

### Phase 2: Dispatch Module

**New file: `src/galois_8/x86/codegen.rs`**

```rust
include!(concat!(env!("OUT_DIR"), "/codegen_encode.rs"));

pub(crate) fn try_encode_codegen_avx2(
    data_shard_count: usize,
    parity_shard_count: usize,
    parity_rows: &[&[u8]],
    data: &[&[u8]],
    parity: &mut [&mut [u8]],
    shard_len: usize,
) -> bool {
    match (data_shard_count, parity_shard_count) {
        (10, 4) => { unsafe { encode_10x4_avx2(...); } true }
        (12, 4) => { unsafe { encode_12x4_avx2(...); } true }
        // ...
        _ => false,
    }
}
```

**New file: `src/galois_8/aarch64/codegen.rs`** (NEON variant, same pattern)

### Phase 3: Integration

**File: `src/core/encode.rs`**

In `encode_sep`, after the LeopardGF8 and fast_one_parity checks, add codegen dispatch:

```rust
// After fast_one_parity check, before generic path:
#[cfg(all(feature = "simd-avx2", target_arch = "x86_64", ...))]
{
    let data_refs: Vec<&[u8]> = data.iter().map(|d| d.as_ref()).collect();
    let mut parity_refs: Vec<&mut [u8]> = parity.iter_mut().map(|p| p.as_mut()).collect();
    if crate::galois_8::x86::codegen::try_encode_codegen_avx2(
        self.data_shard_count, self.parity_shard_count,
        &parity_rows, &data_refs, &mut parity_refs, shard_len,
    ) {
        return Ok(());
    }
}

// Fallback to generic path
let parity_rows = self.get_parity_rows();
self.code_some_slices(&parity_rows, data, parity);
```

### Phase 4: Module Wiring

**File: `src/galois_8/x86/mod.rs`** — add `pub(crate) mod codegen;` gated by `simd-avx2`
**File: `src/galois_8/aarch64/mod.rs`** — add `pub(crate) mod codegen;` gated by `simd-neon`

### Phase 5: Tests

**File: `src/tests/mod.rs`** — add test that verifies codegen output matches generic path:
```rust
#[test]
fn test_codegen_encode_10x4_matches_generic() {
    // Use RSE_BACKEND_OVERRIDE=scalar to force generic path
    // Compare results with codegen path
}
```

## Files Modified

| File | Change |
|------|--------|
| `build.rs` | Add `generate_encode_codegen()` |
| `src/galois_8/x86/codegen.rs` | **New** — include generated code, dispatch fn |
| `src/galois_8/x86/mod.rs` | Add `pub(crate) mod codegen` |
| `src/galois_8/aarch64/codegen.rs` | **New** — NEON variant |
| `src/galois_8/aarch64/mod.rs` | Add `pub(crate) mod codegen` |
| `src/core/encode.rs` | Add codegen dispatch in `encode_sep` |
| `src/tests/mod.rs` | Add codegen correctness test |

## Verification

1. `cargo check --features simd-avx2` — generated code compiles on x86_64
2. `cargo check --features simd-neon` — generated code compiles on aarch64
3. `cargo test --lib` — all 214+ tests pass
4. New test verifies codegen output matches generic path for (10,4) config
5. `cargo bench --bench bandwidth --features simd-avx2` — benchmark comparison
