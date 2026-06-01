# GFNI Backend Benchmark Results

> Date: 2026-06-01
> Hardware: AMD EPYC 9V45 (96-core)
> OS: Linux (x86_64)

---

## Important Caveat

**AMD EPYC 9V45 does not support GFNI (Galois Field New Instructions).** GFNI is available on Intel Ice Lake (10th gen Xeon) and later, or AMD Zen 5+.

The benchmarks below measure the **AVX2 backend** performance, not GFNI. GFNI-specific performance numbers require Intel Ice Lake / Sapphire Rapids hardware.

---

## What Was Measured

### Backend Auto-Selection

On AMD EPYC 9V45, the runtime auto-selects:
- **Primary**: `rust-avx2` (AVX2 available, GFNI not available)
- **Fallback chain**: GFNI+AVX-512 → GFNI+AVX2 → AVX2 → AVX-512 → SSSE3 → scalar

The GFNI backends are correctly skipped when the CPU doesn't support the instruction set.

### Benchmark Configurations

Tests covered common erasure coding configurations:

| Config | Data Shards | Parity Shards | Use Case |
|--------|-------------|---------------|----------|
| 10+4 | 10 | 4 | MinIO, HDFS |
| 12+4 | 12 | 4 | HDFS |
| 8+3 | 8 | 3 | MinIO, Ceph |
| 8+4 | 8 | 4 | Ceph |
| 6+3 | 6 | 3 | Ceph |
| 4+2 | 4 | 2 | MinIO, Ceph |

### Benchmark Files

| File | Description |
|------|-------------|
| `comprehensive-x86_64-benchmark.json` | Full matrix across configs and shard sizes |
| `2026-05-30-optimization-backtest-gfni-auto.json` | GFNI auto-selection backtest |
| `2026-05-30-small-file-gfni-auto.json` | Small file (4 KiB–64 KiB) performance |
| `2026-05-30-smoke-gfni-auto.json` | Smoke test validation |

---

## GFNI Implementation Status

### What's Implemented

- **GFNI+AVX2 backend** (`src/galois_8/x86/gfni.rs`): Full `mul_slice` and `mul_slice_xor` using `_mm256_gf2p8mul_epi8` + AVX2 shuffle fallback for remainder bytes
- **GFNI+AVX-512 backend** (`src/galois_8/x86/gfni.rs`): Full implementation using 512-bit GFNI intrinsics
- **Runtime auto-selection** (`src/galois_8/backend.rs`): GFNI backends have highest priority when available
- **SIMD codegen** (`build.rs`): Specialized encode functions generated for (10,4), (12,4), (8,3), (8,4), (6,3), (4,2) configs

### Expected GFNI Performance (Estimated)

Based on instruction-level analysis:

| Operation | AVX2 (current) | GFNI+AVX2 (expected) | Speedup |
|-----------|----------------|---------------------|---------|
| `mul_slice` (32B/iter) | 2 table lookups + 2 XORs | 1 `gf2p8mul` instruction | ~2x |
| `mul_slice_xor` (32B/iter) | 2 lookups + 3 XORs | 1 `gf2p8mul` + 1 XOR | ~1.7x |
| Codegen encode (10x4) | 40 table loads/chunk | 40 GF mul/chunk | ~1.5-2x |

**Note**: These are theoretical estimates. Actual speedup depends on memory bandwidth, cache behavior, and instruction scheduling.

---

## How to Run GFNI Benchmarks on Supported Hardware

```bash
# On Intel Ice Lake / Sapphire Rapids or AMD Zen 5+:
cargo bench --features simd-gfni

# Verify GFNI is auto-selected:
RSE_BACKEND_OVERRIDE=auto cargo bench --features simd-gfni 2>&1 | grep backend

# Compare GFNI vs AVX2:
RSE_BACKEND_OVERRIDE=rust-gfni-avx2 cargo bench --features simd-gfni
RSE_BACKEND_OVERRIDE=rust-avx2 cargo bench --features simd-avx2
```

---

## Action Items

1. **Run benchmarks on Intel Ice Lake / Sapphire Rapids** to get actual GFNI numbers
2. **Compare GFNI+AVX2 vs plain AVX2** on same hardware to measure real speedup
3. **Test GFNI+AVX-512 vs AVX-512** to isolate GFNI contribution
4. **Benchmark codegen paths** (10x4, 12x4, etc.) with and without GFNI
