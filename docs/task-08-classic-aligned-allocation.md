# Task 08: Classic Aligned Allocation

## 1. Goal

Add aligned shard allocation helpers for the classic GF(2^8) path so callers can obtain allocation layouts that are
more friendly to SIMD backends, without changing any encoding or decoding semantics.

This task is intentionally additive. It should improve ergonomics and potentially improve throughput on alignment-
sensitive CPUs, but it must not change the observable behavior of existing APIs such as `split()`, `encode()`,
`reconstruct()`, or `verify()`.

## 2. Why This Task Exists

Upstream `klauspost/reedsolomon` exposes aligned allocation because shard alignment can matter for vectorized code,
especially on x86 when shard sizes are not naturally aligned to cache- and vector-friendly boundaries.

This crate already has runtime-dispatched SIMD backends in:

- `src/galois_8/backend.rs`
- `src/galois_8/x86/*.rs`
- `src/galois_8/aarch64/*.rs`

What is missing is a public, intentional way for callers to request aligned shards up front.

## 3. Current Code Anchors

Primary code anchors:

- `src/core.rs`
  - `split()` / `join()` live here and currently allocate ordinary `Vec`s
- `src/lib.rs`
  - public exports
- `src/tests/mod.rs`
  - existing API tests
- `benches/galois_backend.rs`
  - backend-sensitive benchmark entry

## 4. Non-Goals

This task must not:

- silently change `split()` behavior
- change zero-padding rules
- change shard ordering
- introduce alternative codec families
- force all internal buffers to be aligned by default

## 5. Public API Design

## 5.1 Minimum additive API

Recommended public APIs:

```rust
impl crate::galois_8::ReedSolomon {
    pub fn alloc_aligned(&self, shard_len: usize) -> Vec<AlignedShard>;
}

pub fn alloc_aligned_shards(total_shards: usize, shard_len: usize) -> Vec<AlignedShard>;
```

The method form is ergonomic for normal callers. The free function is useful when callers need aligned buffers before
constructing a codec or for auxiliary scratch/storage layout work.

`AlignedShard` should implement:

- `AsRef<[u8]>`
- `AsMut<[u8]>`
- `Deref<Target = [u8]>`
- `DerefMut`
- `Clone`
- `FromIterator<u8>`

This preserves compatibility with the crate's existing generic slice-based APIs while avoiding unsafe attempts to
promise stronger alignment on ordinary `Vec<u8>`.

## 5.2 Return contract

Each returned shard must:

- have length exactly `shard_len`
- be zero-initialized
- be independently mutable
- remain safe to pass into existing APIs with no special casing

## 5.3 Alignment contract

Document the guarantee conservatively:

- target 64-byte alignment for each shard start on `galois_8`
- do not promise more than is actually enforced by the implementation
- if a platform-specific fallback cannot guarantee the target alignment, document the fallback clearly

## 6. Implementation Strategy

## 6.1 Preferred implementation direction

Implement a small internal helper dedicated to `u8` shard allocation.

Suggested shape:

```rust
fn alloc_aligned_u8_shards(total_shards: usize, shard_len: usize, align: usize) -> Vec<Vec<u8>>
```

Implementation options:

1. Manual aligned raw allocation with `std::alloc`
2. Over-allocation + offset alignment + owned backing buffer strategy

Preferred choice:

- use explicit aligned allocation behind `AlignedShard`

## 6.2 Safety model

If `std::alloc` is used:

- keep unsafe code tightly isolated in one helper
- allocate enough bytes for `total_shards * shard_len`
- align each shard start to 64 bytes
- free through the same layout path
- add focused tests for length, mutation, and independence

Do not spread alignment-specific unsafe code into codec hot paths.

## 6.3 Ownership model

Returning ordinary `Vec<Vec<u8>>` is not a safe way to guarantee 64-byte alignment on stable Rust.
Therefore the implementation should use an owned wrapper type such as `AlignedShard` that manages an explicitly
aligned allocation while still presenting a normal byte-slice interface to the codec APIs.

## 6.4 Integration with existing APIs

Do not change `split()` in this task.

If later desired, add a separate aligned variant:

```rust
pub fn split_aligned(&self, data: &[u8]) -> Result<Vec<Vec<u8>>, Error>
```

That follow-up should be a separate task because it is contract-sensitive.

## 7. Detailed Execution Steps

1. Add internal alignment helper in `src/core.rs` or a small dedicated module if it improves isolation.
2. Expose `alloc_aligned()` on `galois_8::ReedSolomon`.
3. Expose a free helper if desired for non-method use.
4. Add tests that verify:
   - shard count
   - shard length
   - zero initialization
   - independent ownership
   - alignment of shard starts
5. Add a small benchmark comparison for aligned vs non-aligned shard allocation use in backend-sensitive cases.
6. Update README and docs with alignment guidance and compatibility neutrality.

## 8. Test Plan

## 8.1 Unit tests

Add tests for:

- exact shard count returned
- exact shard length returned
- all bytes initially zero
- mutating one shard does not affect another
- shard start address satisfies documented alignment target

## 8.2 Integration tests

Construct aligned shards and run:

- `encode`
- `verify`
- `reconstruct`

against the same logical data as non-aligned shards and assert identical results.

## 8.3 Benchmark checks

At minimum:

```bash
cargo bench --bench galois_backend --features simd-accel
```

If bench coverage is expanded, add:

- aligned encode case
- unaligned encode case
- aligned reconstruct hotspot case

## 9. Acceptance Criteria

This task is complete when:

1. aligned allocation helpers are public and documented
2. existing APIs accept the aligned shards without adapters
3. output bytes are identical to non-aligned usage
4. tests prove the alignment guarantee actually holds
5. no compatibility-sensitive API silently changes behavior

## 10. Risks

### R1. Unsafe allocation bug

Mitigation:

- keep unsafe code tiny
- centralize allocation/deallocation
- add alignment and mutation tests

### R2. Over-promising alignment across platforms

Mitigation:

- document the exact supported contract
- gate or degrade gracefully if a platform-specific implementation is needed later

### R3. Premature coupling to `split()`

Mitigation:

- keep `split()` unchanged in this task

## 11. Rollout Guidance

Suggested PR title:

- `task08: add classic aligned allocation helpers`

Suggested PR scope:

- public API
- tests
- docs
- optional targeted benchmark only

Do not combine this task with broader reconstruct or matrix work.
