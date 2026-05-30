# Task 11: Required-Only Reconstruct Copy Elision

## 1. Goal

Remove avoidable copying in the `reconstruct_some(required_data_only)` hot path while preserving correctness,
compatibility, and the crate's error-safety guarantees.

The target path today is the required-only data recovery branch that currently snapshots valid input shards into owned
`Vec`s before coding.

## 2. Why This Task Exists

This repository already benchmarked and improved `reconstruct_some`, but the required-only data path still contains a
copy-heavy step that can inflate memory bandwidth cost in recovery-sensitive workloads.

That matters most when:

- shard sizes are small or medium
- recovery is frequent
- required output count is low
- CPU time is no longer the only bottleneck

## 3. Current Code Anchors

Primary code anchor:

- `src/core.rs`
  - `reconstruct_some()`
  - especially the `required_data_only` branch

Relevant existing behavior:

- validation occurs first
- required missing data indices are collected
- valid shards are currently copied into `sub_shards_snapshot`
- recovered outputs are written back only after successful computation

## 4. Hard Constraints

This task must preserve:

- classic shard payload compatibility
- no partial output mutation on validation error
- existing `reconstruct_some` semantics
- correctness across serial and optimized paths

## 5. Current Cost to Remove

The existing copy pattern looks conceptually like:

1. gather valid input shard references
2. clone each valid input shard into owned buffers
3. convert those owned buffers back into borrowed slices for coding

This adds extra allocations and extra full-buffer memory traffic.

## 6. Recommended Refactor Direction

Keep output recovery buffers owned, but avoid cloning input shards unless aliasing rules require it.

Target end state:

- borrow present input shards directly
- allocate only recovered outputs
- write recovered outputs back after successful compute

## 7. Safety and Borrowing Strategy

The main reason copies appear here is usually borrow-management and mutation ordering.

Recommended pattern:

1. validate all shard lengths first
2. collect required index plan
3. borrow immutable input views for present shards
4. allocate recovered output buffers separately
5. compute into recovered output buffers
6. only after success, initialize/write requested missing shards

This keeps the borrow graph manageable without copying the present shard contents.

## 7.1 Current implementation constraint

For the generic `reconstruct_some<T: ReconstructShard<F>>` path, the current `ReconstructShard` trait shape makes
full input-copy elimination difficult because:

- `get()` requires mutable access
- multiple borrowed shard slices must coexist while reconstruction is computed
- later writeback still needs mutable access to the same shard container

In practice this means:

- the fully generic path cannot trivially borrow all inputs and then safely write back later without either copying or
  changing the trait/access model

## 7.2 Practical implementation strategy adopted here

Instead of forcing a risky generic rewrite, the repository now prioritizes:

- preserving the generic path as-is
- specializing `reconstruct_some_opt(&mut [Option<Vec<u8>>], ...)` in `src/galois_8/policy.rs`
- removing unnecessary `Vec<Vec<u8>>` cloning from that specialized path by borrowing immutable input slices inside a
  tightly scoped compute phase

This gives us the highest-value win first, because `Option<Vec<u8>>` is the most common concrete container in the
benchmark and storage-facing paths.

## 8. Detailed Execution Steps

1. Isolate the required-only branch into a small internal helper if that improves readability.
2. Replace `sub_shards_snapshot` ownership with borrowed input slice collection.
   Current applied form:
   done for the specialized `reconstruct_some_opt` path over `Option<Vec<u8>>`
3. Ensure no mutable borrow of missing outputs is taken until after compute succeeds.
4. Preserve the current early-return behavior when no required missing data shards exist.
5. Add targeted benchmarks before and after the refactor.

## 9. Suggested Internal Shape

Possible helper structure:

```rust
fn reconstruct_some_required_data_only<T: ReconstructShard<F>>(...)
```

Inside that helper:

- build `valid_indices`
- build `required_missing_data_indices`
- borrow `sub_shards: SmallVec<[&[F::Elem]; 32]>`
- allocate `recovered_data`
- compute
- commit recovered outputs

## 10. Test Plan

## 10.1 Correctness tests

Retain and extend tests for:

- one missing required data shard
- multiple missing data shards but subset required
- data + parity missing while only data required
- no required missing shards

## 10.2 Mutation-safety tests

Add tests that verify:

- on input validation failure, existing shards are not modified
- on length mismatch, no output shard is initialized

## 10.3 Benchmark plan

Focus on hotspot cases:

- `reconstruct_some_required_2_of_3_missing_data`
- `reconstruct_some_32x16_required_2_of_4_missing_data`
- representative small-file case if available

Commands should include at least:

```bash
cargo test --release --features "std simd-accel" benchmark_reconstruction_hotspots -- --nocapture
scripts/run_small_file_benchmark_matrix.sh
```

## 11. Acceptance Criteria

This task is complete when:

1. required-only reconstruct no longer clones input shards unnecessarily
2. correctness is preserved across existing tests
3. benchmark evidence shows neutral-to-positive impact on the targeted hotspot cases
4. mutation-safety guarantees remain intact

Current status:

- fully generic `ReconstructShard<F>` copy-elision remains constrained by the trait borrow model
- specialized `reconstruct_some_opt(&mut [Option<Vec<u8>>], ...)` has been updated to avoid `Vec<Vec<u8>>` cloning of
  valid input shards
- correctness checks pass for:
  - `test_galois_8_reconstruct_some_opt_matches_reconstruct_some_for_data_only`
  - `test_reconstruct_some_recovers_only_required_data_shard`
- hotspot benchmark still runs and emits stable results

## 12. Risks

### R1. Borrow checker complexity increases readability cost

Mitigation:

- move the branch into a focused helper
- prefer simple explicit staging over clever borrowing tricks

Observed reality:

- this risk is real for the generic path and is the reason the first implementation scope was narrowed to the
  specialized `Option<Vec<u8>>` fast path

### R2. Hidden aliasing bug when committing outputs

Mitigation:

- compute into separate owned recovery buffers
- commit only after success

### R3. Performance win is architecture-sensitive

Mitigation:

- keep benchmark evidence per architecture
- accept neutral results if copy elimination materially simplifies future planner work

## 13. Rollout Guidance

Suggested PR title:

- `task11: remove input copies from required-only reconstruct`

Keep the patch narrow. Do not combine it with large reconstruction planner changes.
