# Benchmark Results — 2026-06-04

Platform: Apple M-series (aarch64), macOS
Backend: RustNEon (runtime auto-selected)
Tool: criterion
Date: 2026-06-04

---

## 1. GF(2^8) mul_slice / mul_slice_xor (galois_backend bench)

### Baseline (before optimization)

| Benchmark | Time | Throughput |
|-----------|------|------------|
| mul_slice / 64KB | 1.2050 µs | 50.650 GiB/s |
| mul_slice / 1MB | 19.187 µs | 50.896 GiB/s |
| mul_slice / 4MB | 76.723 µs | 50.914 GiB/s |
| mul_slice_xor / 64KB | 1.2252 µs | 49.815 GiB/s |
| mul_slice_xor / 1MB | 20.129 µs | 48.514 GiB/s |
| mul_slice_xor / 4MB | 82.306 µs | 47.460 GiB/s |

### Optimized (after NEON XOR unroll branch removal)

| Benchmark | Time | Throughput | Change vs Baseline |
|-----------|------|------------|---------------------|
| mul_slice / 64KB | 1.1900 µs | 51.292 GiB/s | **-1.24% time** (p=0.00) |
| mul_slice / 1MB | 19.175 µs | 50.930 GiB/s | -0.06% (ns) |
| mul_slice / 4MB | 77.066 µs | 50.687 GiB/s | +0.45% (ns) |
| mul_slice_xor / 64KB | 1.2169 µs | 50.156 GiB/s | **-0.68% time** (p=0.00) |
| mul_slice_xor / 1MB | 20.151 µs | 48.463 GiB/s | +0.11% (ns) |
| mul_slice_xor / 4MB | 82.618 µs | 47.281 GiB/s | +0.38% (ns) |

Key improvement: Removing the runtime `RS_NEON_MUL_SLICE_XOR_UNROLL` env var check eliminated a branch in the hot XOR path. Statistically significant for 64KB shards.

---

## 2. Leopard GF(2^8) Encode (throughput_matrix bench)

### Baseline (before optimization)

| Config | Time | Throughput |
|--------|------|------------|
| leopard_encode_4x2_64k | 12.471 µs | 19.577 GiB/s |
| leopard_encode_10x4_1m | 1.0390 ms | 9.3994 GiB/s |
| leopard_encode_32x16_1m | 5.1765 ms | 6.0369 GiB/s |

### Optimized (after lut_xor table rebuild elimination)

| Config | Time | Throughput | Change vs Baseline |
|--------|------|------------|---------------------|
| leopard_encode_4x2_64k | 12.737 µs | 19.168 GiB/s | +2.13% (ns, noise) |
| **leopard_encode_10x4_1m** | **932.98 µs** | **10.467 GiB/s** | **-10.21% time, +11.36% throughput** (p=0.00) |
| **leopard_encode_32x16_1m** | **4.4379 ms** | **7.0416 GiB/s** | **-14.27% time, +16.65% throughput** (p=0.00) |

Key improvement: `fft_dit2`/`ifft_dit2` now use pre-split nibble tables (`Mul8Lut.low`/`high`) via `dit2_step_prebuilt`/`dit2_step_inv_prebuilt`, eliminating the per-call 16-byte table rebuild in `lut_xor()`.

### Leopard Setup (unchanged, as expected)

| Config | Time | Throughput |
|--------|------|------------|
| leopard_setup_4x2_64k | 1.0460 ms | 239.01 MiB/s |
| leopard_setup_10x4_1m | 1.0170 ms | 9.6022 GiB/s |
| leopard_setup_32x16_1m | 1.0605 ms | 29.466 GiB/s |

Setup is unaffected — it doesn't use FFT butterflies.

---

## 3. x86_64 AVX2/SSSE3 Loop Unrolling (2x) — REVERTED

Platform: AMD EPYC 9V45 (Zen 4), Azure VM
Backend override: `RSE_BACKEND_OVERRIDE=avx2` / `RSE_BACKEND_OVERRIDE=ssse3`

**Result: Reverted.** 2x loop unrolling was attempted but showed inconsistent results due to shuffle port contention on Zen 4. `vpshufb` only executes on 2 ports (Port 0/1), so two independent chains serialize completely. See `docs/avx2-ssse3-unroll-analysis-2026-06-04.md` for full analysis with assembly evidence.

### 3.1 Final Verification: Reverted (no unrolling) vs Baseline

After reverting to single-chain loops, verification benchmarks confirmed no regression:

#### AVX2 (32 bytes/iteration, reverted)

| Operation | 64KB | 1MB | 4MB |
|-----------|------|-----|-----|
| mul_slice | 666 ns | 18.1 µs | 73.7 µs |
| mul_slice_xor | 724 ns | 18.6 µs | 66.6 µs |

#### SSSE3 (16 bytes/iteration, reverted)

| Operation | 64KB | 1MB | 4MB |
|-----------|------|-----|-----|
| mul_slice | 662 ns | 17.6 µs | 68.4 µs |
| mul_slice_xor | 774 ns | 18.2 µs | 69.2 µs |

### 3.2 Historical: Unrolled vs Non-unrolled Comparison (for reference)

#### AVX2 unrolled (32→64 bytes/iteration)

| Operation | Size | Time Change | Throughput Change |
|-----------|------|-------------|-------------------|
| mul_slice | 64KB | +1.0% (p=0.00) | -1.0% |
| mul_slice | 1MB | **-3.7%** (p=0.00) | **+3.8%** |
| mul_slice | 4MB | **-3.9%** (p=0.00) | **+4.1%** |
| mul_slice_xor | 64KB | +14.9% (p=0.00) | -13.0% |
| mul_slice_xor | 1MB | **-4.0%** (p=0.00) | **+4.2%** |
| mul_slice_xor | 4MB | **-5.3%** (p=0.00) | **+5.6%** |

#### SSSE3 unrolled (16→32 bytes/iteration)

| Operation | Size | Time Change | Throughput Change |
|-----------|------|-------------|-------------------|
| mul_slice | 64KB | +4.7% (p=0.00) | -4.5% |
| mul_slice | 1MB | +1.5% (p=0.00) | -1.5% |
| mul_slice | 4MB | **-5.9%** (p=0.00) | **+6.2%** |
| mul_slice_xor | 64KB | -0.7% (p=0.01) | +0.7% |
| mul_slice_xor | 1MB | **-2.2%** (p=0.00) | **+2.2%** |
| mul_slice_xor | 4MB | -0.1% (p=0.65, ns) | ~0% |

#### Analysis

- **Large shards (1MB+) showed benefit**: +4-6% throughput for AVX2, +2-6% for SSSE3.
- **Small shards (64KB) regressed for AVX2 mul_slice_xor**: +15% latency from pipeline pressure.
- **Root cause**: Shuffle port contention (Port 0/1 only) prevents latency hiding even with 2x unrolling. See analysis doc for assembly evidence.

---

## 4. Summary

| Optimization | Target | Measured Improvement |
|--------------|--------|---------------------|
| NEON XOR unroll branch removal | `mul_slice_xor` hot path | -0.68% ~ -1.24% latency (64KB) |
| lut_xor table rebuild elimination | Leopard FFT butterflies | **-10% ~ -14% latency, +11% ~ +17% throughput** (1MB) |
| AVX2/SSSE3 loop unrolling (2x) | x86_64 mul_slice | **Reverted** — shuffle port contention prevents benefit, see analysis |
| Reconstruct copy elision | Leopard reconstruct | Not benchmarked (memory savings) |

### Final Verification (Post-Revert)

| Backend | Operation | 64KB | 1MB | 4MB |
|---------|-----------|------|-----|-----|
| AVX2 | mul_slice | 666 ns | 18.1 µs | 73.7 µs |
| AVX2 | mul_slice_xor | 724 ns | 18.6 µs | 66.6 µs |
| SSSE3 | mul_slice | 662 ns | 17.6 µs | 68.4 µs |
| SSSE3 | mul_slice_xor | 774 ns | 18.2 µs | 69.2 µs |

---

## 5. Test Results

```
cargo test --lib:          272 passed, 0 failed
cargo test leopard:         50 passed, 0 failed
cargo test --test selftest:  2 passed, 0 failed
cargo test --test golden:    7 passed, 0 failed
```
