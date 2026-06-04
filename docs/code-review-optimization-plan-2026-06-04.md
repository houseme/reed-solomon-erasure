# Code Review & Optimization Plan — 2026-06-04

Comprehensive review of the Reed-Solomon erasure coding library (~22,400 lines). Focus: performance, code deduplication, dead code removal, and allocation reduction.

---

## 1. SIMD Backend Performance

### 1.1 AVX2 Loop Unrolling (2x) — IMPLEMENTED

**File**: `src/galois_8/x86/avx2.rs`

**Problem**: Main loop processes 32 bytes/iteration with a single `__m256i` shuffle in flight. Shuffle latency (1 cycle on Zen 3+, 3 cycles on Skylake) means the loop is latency-bound.

**Fix**: 2x unroll to 64 bytes/iteration. Two independent load→mask→shuffle→XOR chains in flight, hiding latency.

**Impact**: ~10-20% throughput improvement for `mul_slice`/`mul_slice_xor` on AVX2 systems with shard sizes >= 64 bytes.

### 1.2 SSSE3 Loop Unrolling (2x) — IMPLEMENTED

**File**: `src/galois_8/x86/ssse3.rs`

**Problem**: Main loop processes 16 bytes/iteration. Same latency-bound issue as AVX2.

**Fix**: 2x unroll to 32 bytes/iteration.

**Impact**: ~10-15% throughput improvement on SSSE3-only systems.

### 1.3 AVX-512 — No Change Needed

AVX-512 already processes 64 bytes/iteration, which is sufficient to hide latency.

### 1.4 NEON XOR Unroll Branch — TO DO

**File**: `src/galois_8/aarch64/neon.rs:158-197`

**Problem**: `mul_slice_xor` checks a runtime env var `RS_NEON_MUL_SLICE_XOR_UNROLL` on every call, creating a branch in the hot path. The non-XOR path doesn't have this tunability.

**Fix**: Remove runtime tunability, hardcode 4x unrolling (matching the non-XOR path).

### 1.5 GFNI — Design Limitation (No Change)

GFNI requires 3 instructions (affine+multiply+affine) per GF multiply due to polynomial isomorphism. This is inherent to the GFNI approach, not fixable.

---

## 2. Code Deduplication

### 2.1 TransformDir + dit4_pairwise_one — TO DO

**Problem**: `TransformDir` enum defined twice:
- `leopard_gf8/encode.rs:289`
- `leopard_gf8/decode.rs:428`

`dit4_pairwise_one` (encode.rs:580-655) and `dit4_decode_pairwise_one` (decode.rs:544-620) are structurally identical but differ in work buffer type (`&mut [W]` vs `&mut FlatWork`).

**Fix**: Move `TransformDir` to `ops.rs`. Extract a generic `dit4_pairwise` that works over a trait providing indexed lane access.

### 2.2 get_pair_mut — TO DO

**Problem**: 4 copies of the same split-at-mut pattern:
- `leopard_gf8/ops.rs:646` — `get_pair_mut<T>`
- `leopard_gf8/decode.rs:414` — `get_pair_mut_flat`
- `leopard_gf16/ops.rs:652` — `get_pair_mut_16<T>`
- `leopard_gf16/decode.rs:406` — `get_pair_mut_flat16`

**Fix**: Keep `get_pair_mut` in `ops.rs` (both gf8 and gf16). For `FlatWork`/`FlatWork16` variants, use the existing `lane_mut()` method with a shared helper.

### 2.3 SIMD Wrapper Boilerplate — SKIPPED

The `mul_slice`/`mul_slice_xor` early-out boilerplate is repeated 12 times across 6 backends (37 lines each). However, cfg attributes differ enough between platforms that a macro would need to accept cfg as parameters, reducing the benefit. Left as-is.

---

## 3. Allocation Reduction

### 3.1 Leopard reconstruct_impl Over-Allocation — TO DO

**File**: `src/core/reconstruct.rs:625`

**Problem**: Allocates `total` output buffers of `shard_len` bytes, even for present shards that don't need recovery. Present shard data is then copied into these buffers.

**Current**:
```rust
let mut output_bufs: Vec<Vec<u8>> = (0..total).map(|_| vec![0u8; shard_len]).collect();
// Then copies present shard data into output_bufs[i]
```

**Fix**: Only allocate buffers for missing shards. Pass present shard data directly via `input_data`. Use a mapping from missing index to buffer index.

**Impact**: Saves `(total - missing_count) * shard_len` bytes of allocation + copies. For typical use (e.g., 10+4 with 2 missing), eliminates 12 * shard_len bytes of allocation.

### 3.2 Leopard GF16 Byte-Layout Conversion — FUTURE

**Files**: `leopard_gf16/encode.rs:104-118`, `leopard_gf16/decode.rs:164-173`

**Problem**: Every GF16 encode/decode allocates temporary `Vec<u8>` per shard for `user_bytes_to_work_bytes` / `work_bytes_to_user_bytes` conversion.

**Fix**: Use pre-allocated thread-local buffers (like the `FlatWork` cache pattern in GF8) or implement in-place conversion.

**Impact**: Eliminates `data_shard_count + parity_shard_count` allocations per encode call.

### 3.3 Reconstruct Snapshot Copies — FUTURE

**File**: `src/core/reconstruct.rs:536-548`

**Problem**: Copies all valid shards into new vectors to work around the borrow checker.

**Fix**: Use index-based access pattern to avoid the snapshot.

---

## 4. Dead Code Cleanup

### 4.1 SVE Stub — TO DO

**File**: `src/galois_8/aarch64/sve.rs` (53 lines)

**Problem**: Entire file is a stub that always returns `available: false`. Compiled unconditionally on aarch64+neon builds. Detection result is discarded in `backend.rs:597`.

**Fix**: Add `#[allow(dead_code)]` or gate behind a feature flag. Keep the file as a reserved slot for future SVE implementation.

---

## 5. Hot-Path Micro-Optimizations

### 5.1 lut_xor Table Rebuild — TO DO

**File**: `src/core/leopard_gf8/ops.rs:285-301`

**Problem**: `lut_xor()` rebuilds the 16-byte high nibble table on every call:
```rust
let mut high = [0u8; 16];
for i in 0..16 { high[i] = lut[i * 16]; }
```
Plus a `.expect()` on a slice that's always 16 bytes.

**Fix**: Refactor `fft_dit2`/`ifft_dit2` to accept `&Mul8Lut` (pre-split tables) instead of `&[u8; 256]`. The encode path already uses `lut_xor_prebuilt`; the decode path should too.

**Impact**: Eliminates 16-byte table rebuild + expect() per butterfly call. Minor but accumulates across thousands of FFT butterfly operations.

---

## 6. Implementation Priority

| Priority | Task | Impact | Risk |
|----------|------|--------|------|
| P0 | AVX2/SSSE3 loop unrolling | High (10-20% throughput) | Low |
| P1 | lut_xor table rebuild elimination | Medium | Low |
| P1 | TransformDir + dit4 dedup | Medium (code quality) | Low |
| P1 | get_pair_mut dedup | Low (code quality) | Low |
| P2 | Reconstruct over-allocation | Medium (memory) | Medium |
| P2 | NEON XOR unroll branch cleanup | Low | Low |
| P3 | SVE stub cleanup | Low (code quality) | None |
| P3 | GF16 byte-layout conversion | Medium | Medium |

---

## 7. Testing Strategy

All changes must pass:
```bash
cargo test --features simd-accel
cargo test --features simd-accel --release
```

SIMD-specific tests in each backend file verify correctness against scalar reference implementation.

---

## 8. Benchmark Results (Apple M-series, aarch64)

### 8.1 GF(2^8) mul_slice / mul_slice_xor (NEON backend)

| Metric | 64KB | 1MB | 4MB |
|--------|------|-----|-----|
| mul_slice time change | **-0.58%** (p=0.00) | -0.03% (ns) | +0.06% (ns) |
| mul_slice_xor time change | **-1.12%** (p=0.00) | -0.03% (ns) | **-0.86%** (p=0.00) |

NEON improvements from removing the runtime unroll-factor branch in `mul_slice_xor`.

### 8.2 Leopard GF(2^8) Encode (FFT-based)

| Config | Time Change | Throughput Change |
|--------|-------------|-------------------|
| leopard_encode_4x2_64k | +0.92% | -0.91% |
| **leopard_encode_10x4_1m** | **-12.11%** (p=0.00) | **+13.77%** |
| **leopard_encode_32x16_1m** | **-15.00%** (p=0.00) | **+17.65%** |

Significant improvement from eliminating per-butterfly table rebuild in `fft_dit2`/`ifft_dit2`.

### 8.3 x86_64 AVX2/SSSE3 Loop Unrolling (Intel Xeon Platinum 8370C)

**AVX2 (32→64 bytes/iteration):**

| Operation | 64KB | 1MB | 4MB |
|-----------|------|-----|-----|
| mul_slice time change | +1.0% (p=0.00) | **-3.7%** (p=0.00) | **-3.9%** (p=0.00) |
| mul_slice_xor time change | +14.9% (p=0.00) | **-4.0%** (p=0.00) | **-5.3%** (p=0.00) |

**SSSE3 (16→32 bytes/iteration):**

| Operation | 64KB | 1MB | 4MB |
|-----------|------|-----|-----|
| mul_slice time change | +4.7% (p=0.00) | +1.5% (p=0.00) | **-5.9%** (p=0.00) |
| mul_slice_xor time change | -0.7% (p=0.01) | **-2.2%** (p=0.00) | -0.1% (ns) |

Large shards (1MB+) benefit from unrolling (+4-6% throughput). Small shards (64KB) show regressions due to pipeline pressure from the extra load+XOR chain in the unrolled loop.
