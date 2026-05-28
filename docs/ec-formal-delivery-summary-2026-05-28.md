# EC Formal Delivery Summary 2026-05-28

## 1. Summary

This document is the formal delivery summary for the current local working tree on top of:

- base commit: `fa49e2a`

It captures the implemented capability set, the verification performed, the benchmark-backed conclusions, the
compatibility boundaries, and the recommended next steps.

This summary is intended to be the primary handoff document for future contributors.

## 2. Delivered Capability Set

## 2.1 Core structure and maintainability

The previous monolithic `src/core.rs` layout has been split into focused modules:

- `src/core/mod.rs`
- `src/core/codec.rs`
- `src/core/encode.rs`
- `src/core/verify.rs`
- `src/core/reconstruct.rs`
- `src/core/options.rs`
- `src/core/metrics.rs`
- `src/core/parallel.rs`
- `src/core/shard_by_shard.rs`
- `src/core/workspace.rs`

Outcome:

- lower review complexity
- better task isolation
- easier hot-path reasoning

## 2.2 Aligned allocation for SIMD-sensitive callers

Delivered:

- `src/galois_8/aligned.rs`
- `galois_8::AlignedShard`
- `galois_8::alloc_aligned_shards(...)`
- `galois_8::ReedSolomon::alloc_aligned(...)`

Outcome:

- explicit aligned allocation API without changing codec output semantics

## 2.3 Real matrix-mode support

`MatrixMode` is now meaningful instead of placeholder-only:

- `Vandermonde`
- `Cauchy`
- `JerasureLike`
- `Custom` through `with_custom_matrix(...)`

Outcome:

- default classic output remains unchanged
- non-default matrix modes are explicit and tested
- custom parity-row construction is supported through a dedicated constructor

## 2.4 Sparse parity update API

Delivered:

- `ReedSolomon::update(...)`

Outcome:

- parity maintenance for sparse data-shard changes without full re-encode
- byte-equivalence validated against full `encode(...)`

## 2.5 Specialized reconstruct planning consolidation

The current mature planner work is on the `Option<Vec<u8>>` path.

Delivered in `src/galois_8/policy.rs`:

- `OptionVecReconstructPlan`
- `plan_option_vec_reconstruct(...)`
- `execute_option_vec_required_data_plan(...)`

Shared specialized paths:

- `reconstruct_opt(...)`
- `reconstruct_data_opt(...)`
- `reconstruct_some_opt(..., required)` data-only path

Outcome:

- shared planning state across the highest-value concrete reconstruct path
- clearer `plan + execute` structure on specialized recovery flows

## 2.6 Progressive decode API

Delivered first-pass `decode_idx(...)` support in `src/galois_8/policy.rs`:

- normal progressive mode
- merge mode
- `Option<Vec<u8>>` destination path

Outcome:

- classic GF(2^8) can now support incremental decode and partial-result merge workflows

## 3. Validation Performed

Representative validation commands used successfully in this phase:

```bash
cargo check --tests
```

```bash
cargo test test_alloc_aligned_roundtrip_encode_verify_and_reconstruct
```

```bash
cargo test test_cauchy_matrix_mode_roundtrips_and_differs_from_vandermonde
cargo test test_jerasure_like_matrix_mode_roundtrips_and_differs_from_vandermonde
cargo test test_with_custom_matrix_roundtrips_and_uses_supplied_rows
```

```bash
cargo test test_update_matches_full_encode_for_single_changed_data_shard
cargo test test_update_matches_full_encode_for_multiple_changed_data_shards
cargo test test_update_fast_one_parity_matches_full_encode
```

```bash
cargo test test_galois_8_reconstruct_data_opt_matches_reconstruct_data
cargo test test_galois_8_reconstruct_data_opt_matches_reconstruct_data_for_small_shards
cargo test test_galois_8_reconstruct_opt_matches_reconstruct
cargo test test_galois_8_reconstruct_some_opt_matches_reconstruct_some_for_data_only
```

```bash
cargo test test_galois_8_decode_idx_progressive_matches_reconstruct_some
cargo test test_galois_8_decode_idx_merge_mode_accumulates_partial_results
cargo test test_galois_8_decode_idx_rejects_invalid_expect_input_length
cargo test test_galois_8_decode_idx_rejects_incorrect_dst_len
cargo test test_galois_8_decode_idx_rejects_incorrect_input_len
cargo test test_galois_8_decode_idx_rejects_shard_size_mismatch
cargo test test_galois_8_decode_idx_merge_mode_rejects_missing_dst_target
cargo test test_galois_8_decode_idx_rejects_too_few_expected_inputs
```

## 4. Benchmark-Backed Conclusions

## 4.1 `update` vs full `encode`

Reference:

- `docs/ec-update-benchmark-results-2026-05-28.md`

High-confidence conclusion:

- `update` is strongly worthwhile for sparse writes
- speedup scales roughly with inverse changed-shard count
- higher fanout increases the value of `update`

Representative result:

- `10x4_1m`, `1` changed shard: about `12.1x` faster than full `encode`

## 4.2 `decode_idx` vs one-shot `reconstruct_some`

Reference:

- `docs/ec-decode-idx-benchmark-results-2026-05-28.md`

Current benchmark picture:

- `4x2_64k`: about `1.03x`
- `4x2_4m`: about `0.86x`
- `10x4_1m`: about `0.91x`
- `32x16_1m`: about `1.01x`
- `32x16_4m`: about `1.00x`

Interpretation:

- `decode_idx(...)` is already performance-viable on higher-fanout shapes
- smaller-fanout shapes still show more overhead sensitivity
- shard size alone does not remove that smaller-fanout sensitivity

## 4.3 Current worktree vs clean baseline commit

Reference:

- `docs/main-vs-origin-main-performance-2026-05-28.md`

Important distinction:

- shared legacy operations can be directly compared against the clean baseline
- new APIs such as `update` and `decode_idx` must be judged using current-tree paired comparisons because they do not
  exist on the baseline revision

## 5. Compatibility Boundaries

Reference:

- `docs/ec-minio-compatibility-checklist.md`

Current compatibility position:

- classic MinIO-oriented compatibility remains tied to the default classic GF(2^8) path
- safe optimizations are mainly in:
  - execution strategy
  - scheduling
  - caching
  - allocation behavior
- output-changing items remain compatibility-sensitive:
  - non-classic matrix modes
  - custom matrices
  - future Leopard-family work

## 6. What Was Tried And Intentionally Not Kept

## 6.1 Generic required-only copy elimination

The generic `ReconstructShard<F>` route was not fully unified or copy-elided because the borrow model becomes
substantially more complex there.

Decision:

- keep generic path conservative
- prioritize the `Option<Vec<u8>>` fast path first

## 6.2 Aggressive small-fanout `decode_idx` micro-optimization

A more aggressive reduced-column / small-output optimization was attempted for `decode_idx(...)`.

Benchmark result:

- it did not produce a stable improvement on `4x2_64k` / `10x4_1m`

Decision:

- revert the unstable optimization
- retain the more stable prior implementation
- record the failed direction so it is not retried blindly

## 6.3 Repeated-call planning-cache attempt for `decode_idx`

A repeated-call planning-cache direction was also evaluated.

Benchmark result:

- no stable improvement for the targeted smaller/moderate-fanout cases

Decision:

- not retained in the current implementation

## 7. Current Technical Debt / Deferred Work

## 7.1 Generic reconstruct planner unification

Status:

- intentionally deferred

Reason:

- borrow/lifetime complexity is materially higher than the specialized `Option<Vec<u8>>` case

## 7.2 `decode_idx(...)` as a final optimized path

Status:

- not yet fully optimized

Reason:

- correctness and operational usefulness were prioritized first
- a realistic benchmark baseline now exists for future tuning

## 7.3 Leopard family / alternative codec-family expansion

Status:

- still boundary-defined, not implemented as production path

Reason:

- compatibility and algorithm-family separation are more important than rushing implementation

## 8. Recommended Next Steps

Priority-ordered suggestions:

1. continue `decode_idx(...)` tuning only if a clearly better direction than the rejected micro-optimizations is
   identified
2. if not, shift effort to another high-value API or algorithmic task instead of forcing diminishing-return tuning
3. revisit generic reconstruct-planner unification only after specialized-path value is exhausted

## 9. Reading Guide

Use these documents depending on what you need:

- overall phase snapshot:
  - `docs/ec-phase-summary-2026-05-28.md`
- `update`:
  - `docs/task-10-classic-parity-update-api.md`
  - `docs/ec-update-benchmark-results-2026-05-28.md`
- reconstruct planner:
  - `docs/task-11-required-only-reconstruct-copy-elision.md`
  - `docs/task-12-reconstruct-plan-unification.md`
- progressive decode:
  - `docs/task-13-progressive-decode-idx.md`
  - `docs/ec-decode-idx-benchmark-results-2026-05-28.md`
- baseline comparison:
  - `docs/main-vs-origin-main-performance-2026-05-28.md`
- quick reference index:
  - `docs/README-performance-index.md`
