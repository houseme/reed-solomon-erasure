# EC Klauspost Alignment Design

## Goal

This document narrows the earlier upstream comparison into an implementation plan for this crate.
The goal is to absorb high-value ideas from `klauspost/reedsolomon` while preserving compatibility for
the classic MinIO-oriented Reed-Solomon path.

The design separates work into three tracks:

1. Safe performance and ergonomics improvements on the classic GF(2^8) path.
2. New APIs that improve incremental and operational workflows without changing encoded output.
3. Optional alternative codec paths that should never be enabled by default for MinIO-compatible usage.

## Current Baseline

The current crate already has several strengths that should be retained:

- Runtime-dispatched SIMD backend selection in `src/galois_8/backend.rs`.
- Explicit parallel decision and profiling surfaces in `src/core.rs` and `src/galois_8/policy.rs`.
- Reconstruction cache heuristics and observability in `src/core.rs`.
- Reconstruct hot-path specialization for one/two-output cases in `src/core.rs`.
- Reusable verify workspace support and benchmark/reporting infrastructure.

This means the next phase should not be "copy upstream literally". The right move is to reuse upstream ideas
where they fill real product gaps.

## Non-Goals

The following are not goals for the MinIO-compatible mainline:

- Changing the default coding matrix away from the classic Vandermonde-compatible path.
- Switching the default encode/reconstruct path to Leopard mode.
- Optimizing for shard counts that MinIO erasure sets do not use as a primary design center.
- Replacing current Rust runtime-dispatch and policy controls with a simpler option model.

## Compatibility Boundary

There are two compatibility layers:

### Layer 1: shard payload compatibility

This means the generated data/parity shard bytes match another implementation for the same input.

This compatibility is preserved as long as all of the following remain unchanged:

- data shard count
- parity shard count
- shard ordering
- padding and split/join rules
- coding matrix
- classic GF(2^8) encoding path

### Layer 2: MinIO object-format compatibility

This is stricter than shard payload compatibility. It additionally requires matching MinIO metadata and layout:

- erasure set placement
- part sizing rules
- checksum and bitrot rules
- metadata fields and ordering
- `xl.meta` expectations

This crate only directly controls Layer 1 today. Layer 2 requires additional storage-format work outside the
core erasure codec.

## What To Absorb From Klauspost

## 1. Add aligned allocation as a first-class API

Upstream provides aligned shard allocation because alignment can noticeably improve SIMD throughput on some CPUs.
This is worth adding here.

### Proposed API

- `pub fn alloc_aligned(&self, shard_len: usize) -> Vec<Vec<F::Elem>>`
- `pub fn alloc_aligned_shards(total_shards: usize, shard_len: usize) -> Vec<Vec<u8>>` for `galois_8`

### Constraints

- The API must be additive.
- It must not change the behavior of existing `split()`.
- `split()` may later gain an aligned variant rather than silently changing allocation behavior.

### Expected value

- Better x86 SIMD locality/alignment.
- Cleaner caller ergonomics for high-throughput users.

## 2. Add parity update API

Upstream's `Update` API is highly valuable for workloads where only a few data shards change.

### Proposed API

- `pub fn update<T, U>(&self, old_shards: &[T], new_data: &[Option<U>], parity: &mut [U]) -> Result<(), Error>`
- `pub fn update_opt(...)` for the `std` fast path when useful

### Semantics

- Caller supplies unchanged parity and only the changed data shards.
- Implementation computes delta parity instead of full re-encode.
- Output parity must be byte-for-byte identical to full `encode`.

### Why this matters

This is a real operational gap today. `encode_single` and `ShardByShard` help progressive encode, but they do not
solve sparse-update parity maintenance.

## 3. Add progressive decode API

Upstream's `DecodeIdx` supports incremental reconstruction from partial arrivals or merged partial work.

### Proposed API

- `pub fn decode_idx(&self, dst: &mut [Option<Vec<u8>>], expect_input: Option<&[bool]>, input: &[Option<Vec<u8>>]) -> Result<(), Error>`

### Initial scope

- Support classic GF(2^8) only.
- Support additive accumulation into caller-provided zeroed buffers.
- Skip parity reconstruction in the first iteration if that keeps the API smaller and safer.

### Why this matters

This fits distributed reconstruction and networked recovery much better than forcing everything through a one-shot
`reconstruct` call.

## 4. Make `MatrixMode` real

`MatrixMode` is exposed publicly, but `build_matrix_with_options()` currently routes all modes through the same
classic matrix builder.

### Required follow-up

- Implement true `Cauchy`.
- Implement true `JerasureLike`.
- Define whether `Custom` means "full generator matrix" or "parity rows only", and document it precisely.
- Add explicit compatibility notes to each mode.

### Important rule

The default must remain the classic MinIO-safe matrix mode.

## 5. Unify reconstruction output planning

Upstream uses a unified reconstruction model that builds one output plan for all requested missing shards.
Our current Rust path is already specialized and benchmark-aware, but still splits data/parity reconstruction
into separate stages on the parallel `Option<Vec<_>>` path.

### Proposed refactor

- Introduce an internal `ReconstructPlan`:
  - valid indices
  - invalid indices
  - missing data indices
  - missing parity indices
  - matrix rows for requested outputs
  - stage policy

- Keep the ability to specialize one/two-output data-only recovery.
- Allow the general path to reconstruct all requested outputs through one internal planner.

### Why this is worth doing

- Less duplicated planning logic.
- Easier future support for `DecodeIdx`.
- Better separation between "which outputs are needed" and "how we execute them".

## 6. Remove avoidable copying in required-only reconstruct paths

`reconstruct_some(required_data_only)` still snapshots valid input shards into owned `Vec`s before coding.
That is safe, but it adds extra memory traffic.

### Proposed follow-up

- Refactor the required-only path to use borrowed input shards where possible.
- Preserve the guarantee that output shards are not mutated on error.
- Only allocate owned scratch when aliasing or initialization rules require it.

### Expected value

- Better small/medium shard recovery throughput.
- Lower memory bandwidth pressure during hot recovery loops.

## 7. Keep current policy model, do not downgrade to upstream option simplicity

Upstream's `WithAutoGoroutines` is good for Go, but this crate already has:

- `ParallelPolicy`
- `ParallelDecision`
- stage-specific reconstruct policy
- runtime profile metrics
- backend-specific tuning hooks

That is a stronger base for long-term tuning. The right action is to improve documentation and external ergonomics,
not replace it.

## Alternative Codec Track

## Leopard GF8/GF16

This is the single largest algorithmic gap versus upstream, but it must be isolated from the classic path.

### Recommendation

Add Leopard as an explicit alternative backend family:

- `CodecFamily::Classic`
- `CodecFamily::LeopardGF8`
- `CodecFamily::LeopardGF16`
- `CodecFamily::Auto`

### Guardrails

- Never make Leopard the default for MinIO-compatible use.
- Make all output-compatibility changes explicit in docs and types.
- Require dedicated benchmarks before any auto-selection rule is introduced.

### Why it still matters

Even if MinIO compatibility stays on the classic path, Leopard is the right long-term answer for high shard counts
where the classic `O(N^2)` strategy is no longer attractive.

## Execution Plan

## Phase A: additive APIs and contract cleanup

Deliver:

- aligned allocation API
- update API
- real `MatrixMode` behavior and docs
- compatibility notes in public docs

Validation:

- unit coverage for byte-identical parity output
- no regressions in existing smoke benchmarks

## Phase B: reconstruction planner cleanup

Deliver:

- internal `ReconstructPlan`
- required-only path copy reduction
- unified output planning

Validation:

- existing reconstruct smoke and hotspot benchmarks
- backend consistency checks
- no regressions against recent benchmark gates

## Phase C: progressive decode

Deliver:

- classic-path `decode_idx`
- merge/additive reconstruction semantics

Validation:

- deterministic multi-step reconstruction tests
- equivalence to one-shot `reconstruct` for supported cases

## Phase D: optional alternative family

Deliver:

- Leopard GF8
- optional Leopard GF16 evaluation

Validation:

- separate benchmark ledger from classic path
- explicit compatibility break labeling

## Acceptance Criteria

Classic MinIO-oriented mode is successful only if all of the following remain true:

- Encoded shards remain byte-compatible with the existing classic matrix path.
- Parallel/SIMD optimizations do not alter output.
- Benchmark gates remain green on current smoke cases.
- New additive APIs document whether they preserve classic compatibility.
- Alternative codec families are opt-in and clearly separated.

## Recommended Immediate Order

If we want the best ratio of value to risk, the next implementation order should be:

1. `alloc_aligned`
2. real `MatrixMode`
3. `update`
4. required-only reconstruct copy removal
5. unified reconstruct planner
6. `decode_idx`
7. Leopard track
