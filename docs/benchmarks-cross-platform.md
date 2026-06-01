# Cross-Platform Benchmark Report

> Date: 2026-06-01
> Library: reed-solomon-erasure v6.0.0

---

## Benchmark Infrastructure

### Benchmark Targets

| Target | File | Description |
|--------|------|-------------|
| `bandwidth` | `benches/bandwidth.rs` | Encode/reconstruct throughput (GiB/s) |
| `throughput_matrix` | `benches/throughput_matrix.rs` | Encode throughput across config matrix |
| `smoke` | `tests/benchmark_smoke.rs` | CI-friendly smoke tests (fast profile) |

### Configuration Matrix (22 configs)

Defined in `benches/common/mod.rs`:

| Shard Size | Data×Parity Configs |
|------------|---------------------|
| 1 KiB | 10×4, 12×4, 8×3, 8×4, 6×3, 4×2 |
| 4 KiB | 10×4, 12×4, 8×3, 8×4, 6×3, 4×2 |
| 64 KiB | 10×4, 12×4, 8×3, 8×4, 6×3, 4×2 |
| 1 MiB | 10×4, 12×4, 8×3, 8×4, 6×3, 4×2 |

### How to Run

```bash
# Full benchmark suite
cargo bench

# Specific SIMD backend
cargo bench --features simd-avx2    # x86_64 AVX2
cargo bench --features simd-neon    # aarch64 NEON
cargo bench --features simd-gfni    # x86_64 GFNI (requires Intel Ice Lake+)

# Smoke tests (CI)
VALIDATION_PROFILE=fast cargo test --test benchmark_smoke

# Export results as JSON
RSE_WRITE_PROFILE_REPORT=1 cargo bench
```

---

## Platform Results

### aarch64 (Apple Silicon)

**Hardware**: Apple M-series (aarch64)
**Backend**: `rust-neon` (auto-selected)
**Feature**: `simd-neon`

| Config | Shard Size | Encode (GiB/s) | Reconstruct (GiB/s) |
|--------|------------|----------------|---------------------|
| 10×4 | 4 KiB | — | — |
| 10×4 | 64 KiB | — | — |
| 10×4 | 1 MiB | — | — |

> **Status**: Benchmark data pending. Run `cargo bench --features simd-neon` on Apple Silicon to populate.

### x86_64 (AMD EPYC)

**Hardware**: AMD EPYC 9V45 (96-core)
**Backend**: `rust-avx2` (auto-selected; GFNI not available on AMD)
**Feature**: `simd-avx2`

Benchmark data available in `benchmarks/x86_64-simd/`:
- `comprehensive-x86_64-benchmark.json` — full matrix
- `2026-05-30-small-file-gfni-auto.json` — small file perf

> **Note**: AMD EPYC does not support GFNI. See [GFNI results doc](benchmarks-gfni-results.md) for details.

### x86_64 (Intel with GFNI)

**Status**: No data yet. Requires Intel Ice Lake (10th gen Xeon) or later.

Expected backends:
- `rust-gfni-avx512` (if AVX-512 + GFNI available)
- `rust-gfni-avx2` (if AVX2 + GFNI available)

---

## Methodology

### Metrics

- **Encode throughput**: `data_shards × shard_size / encode_time` (GiB/s)
- **Reconstruct throughput**: `data_shards × shard_size / reconstruct_time` (GiB/s)
- **Backend**: Auto-detected at runtime, reported in benchmark output

### Warm-up

- Criterion benchmarks: 3-second warm-up, 10 measurement iterations
- Smoke tests: No warm-up (CI validation only)

### Environment Variables

| Variable | Purpose |
|----------|---------|
| `RSE_BACKEND_OVERRIDE` | Force specific backend (`auto`, `scalar`, `rust-avx2`, etc.) |
| `RSE_WRITE_PROFILE_REPORT` | Export profiling data as JSON |
| `VALIDATION_PROFILE` | Smoke test profile (`fast`, `extended`) |
| `RS_PARALLEL_POLICY_MAX_JOBS` | Limit parallelism for reproducible results |

---

## Comparison with klauspost/reedsolomon (Go)

### Methodology Alignment

To compare Rust vs Go implementations:

1. Same hardware, same OS
2. Same (data, parity) configurations
3. Same shard sizes
4. Single-threaded comparison (set `RS_PARALLEL_POLICY_MAX_JOBS=1`)
5. Multi-threaded comparison (default parallelism)

### Go Benchmark Command

```bash
# In klauspost/reedsolomon repo:
go test -bench=BenchmarkEncode -benchtime=5s -cpu=1
go test -bench=BenchmarkReconstruct -benchtime=5s -cpu=1
```

### Key Differences

| Aspect | reed-solomon-erasure (Rust) | klauspost/reedsolomon (Go) |
|--------|---------------------------|---------------------------|
| SIMD dispatch | Runtime (CPUID) | Runtime (CPUID) |
| GF backends | Scalar, SSSE3, AVX2, AVX-512, GFNI, NEON | Scalar, SSSE3, AVX2, AVX-512, GFNI |
| Leopard codec | GF8 (FFT-based, high shard count) | GF8 + GF16 |
| Parallelism | Rayon + configurable policy | Goroutines |
| Streaming API | Not yet implemented | Supported |

---

## Action Items

1. **Run benchmarks on Apple Silicon** to get NEON numbers
2. **Run benchmarks on Intel Ice Lake** to get GFNI numbers
3. **Run Go benchmarks on same hardware** for direct comparison
4. **Create comparison table** with normalized throughput (GiB/s per core)
