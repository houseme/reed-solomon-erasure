# aarch64 SIMD Backend Design

## 1. Overview

This document describes the aarch64 SIMD backend architecture for GF(2^8) multiplication in `rustfs-erasure-codec`.

## 2. Backend Hierarchy

Runtime dispatch selects the first available backend:

| Priority | Backend | ISA Requirement | Status |
|----------|---------|-----------------|--------|
| 1 | `rust-neon` | NEON | Active |
| 2 | `simd-c` | NEON | Legacy fallback |
| 3 | `scalar-rust` | None | Baseline |

## 3. Module Layout

```
src/galois_8/aarch64/
├── mod.rs     — Feature detection (detect_neon_features, detect_sve_features)
├── neon.rs    — Rust NEON backend (4x unroll + 2x unroll + scalar tail)
└── sve.rs     — SVE stub (available: false, reserved for future)
```

## 4. NEON Algorithm

The NEON backend uses `vqtbl1q_u8` table lookup (equivalent to x86 `pshufb`):

1. **Table Loading**: Load `MUL_TABLE_LOW[c]` and `MUL_TABLE_HIGH[c]` into NEON registers
2. **4x Unrolled Loop** (64-byte chunks via `vld1q_u8_x4`):
   - Split each 16-byte vector into low/high nibbles (`vandq_u8` + `vshrq_n_u8::<4>`)
   - Table lookup low nibbles: `vqtbl1q_u8(low_tbl, low)`
   - Table lookup high nibbles: `vqtbl1q_u8(high_tbl, high)`
   - XOR results: `veorq_u8(low_result, high_result)`
3. **2x Unrolled Loop** (16-byte chunks for remainder)
4. **Scalar Tail**: Bytes not aligned to 16 bytes handled by `scalar::mul_slice_pure_rust`

## 5. Feature Detection

```rust
Aarch64FeatureSet {
    neon: std::arch::is_aarch64_feature_detected!("neon"),
    sve: false, // Reserved; detect_sve_features() returns available: false
}
```

Apple Silicon (M1/M2/M3/M4) always has NEON. The `cfg` gates exclude MSVC, Android, and iOS.

## 6. Runtime Override

```bash
RSE_BACKEND_OVERRIDE=rust-neon   # Force NEON backend
RSE_BACKEND_OVERRIDE=scalar      # Force scalar fallback
RSE_BACKEND_OVERRIDE=simd-c      # Force legacy C backend
RSE_STRICT_BACKEND_OVERRIDE=1    # Fail if override cannot be honored
```

## 7. Parallel Policy (aarch64-specific)

aarch64 has dedicated environment variable overrides for reconstruction parallelism:

| Variable | Default | Purpose |
|----------|---------|---------|
| `RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES` | Inherited from base | Minimum shard size for parallel reconstruct |
| `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB` | Inherited from base | Minimum bytes per parallel job |
| `RS_AARCH64_RECONSTRUCT_MAX_JOBS` | Inherited from base | Maximum parallel jobs |
| `RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB` | Inherited from base | Data-only reconstruct min bytes per job |
| `RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB` | Inherited from base | Parity reconstruct min bytes per job |

## 8. SVE Extension Point

The `sve.rs` module is a placeholder. When implemented:

1. Use scalable vector types (`svuint8_t`) instead of fixed-width (`uint8x16_t`)
2. Use predicate registers for VL-agnostic tail handling
3. Backend priority will be: `rust-sve` > `rust-neon` > `simd-c` > `scalar`
4. Must pass: scalar correctness, override verification, metadata consistency

## 9. Profile Metrics

The NEON backend collects runtime statistics (gated by `benchmark-metrics` feature):

- `mul_calls`: Total invocation count
- `vector_64b_chunks`: 64-byte aligned chunks processed (4x unrolled)
- `vector_16b_chunks`: 16-byte chunks in remainder
- `tail_bytes`: Scalar fallback bytes

The `mul_slice_xor` variant additionally supports configurable unroll factor via `RS_NEON_MUL_SLICE_XOR_UNROLL` (4 or 2) and schedule splitting via `RS_NEON_MUL_SLICE_XOR_SCHEDULE`.
