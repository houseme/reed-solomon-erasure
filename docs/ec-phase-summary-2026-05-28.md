# EC Phase Summary 2026-05-28

## Scope

This summary captures the current phase closure for the local working tree on top of commit:

- `fa49e2a`

It focuses on the work completed in this phase rather than the full historical roadmap.

## What Landed In This Phase

## 1. Core directory restructuring

The old monolithic `src/core.rs` layout has been split into:

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

- responsibilities are clearer
- future task isolation is easier
- hot paths are easier to review independently

## 2. Aligned shard allocation

Added:

- `src/galois_8/aligned.rs`
- `galois_8::AlignedShard`
- `galois_8::alloc_aligned_shards(...)`
- `galois_8::ReedSolomon::alloc_aligned(...)`

Outcome:

- explicit 64-byte aligned allocation path for SIMD-sensitive workloads
- works with existing encode/verify/reconstruct APIs through standard slice traits

## 3. Real matrix modes

`MatrixMode` is no longer a placeholder:

- `Vandermonde`
- `Cauchy`
- `JerasureLike`
- `Custom` via `with_custom_matrix(...)`

Outcome:

- default classic mode remains intact
- alternative matrix families are explicit and tested
- custom parity-row construction is available through a dedicated constructor

## 4. Classic parity update API

Added:

- `ReedSolomon::update(...)`

Outcome:

- sparse parity maintenance is now supported without full re-encode
- one- and multi-shard changed-data paths are validated against full `encode`

## 5. Specialized reconstruct planner consolidation

The most mature planning work in this phase is on the `Option<Vec<u8>>` path.

Current specialized planner shape in `src/galois_8/policy.rs`:

- `OptionVecReconstructPlan`
- `plan_option_vec_reconstruct(...)`
- `execute_option_vec_required_data_plan(...)`

Shared paths:

- `reconstruct_opt(...)`
- `reconstruct_data_opt(...)`
- `reconstruct_some_opt(..., required)` for the data-only case

Outcome:

- these three specialized paths now share one planning basis
- `reconstruct_some_opt(data_only)` now has a clearer `plan + execute` structure

## 6. Progressive decode API

Added first-pass `decode_idx(...)` support in `src/galois_8/policy.rs`:

- normal progressive mode
- merge mode
- `Option<Vec<u8>>` destination path

Outcome:

- classic GF(2^8) now has an incremental decode capability
- correctness and core error paths are covered

## Verification Performed

Representative commands used successfully during this phase:

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

## Benchmark Conclusions

## 1. `update` vs full `encode`

See:

- `docs/ec-update-benchmark-results-2026-05-28.md`

Key conclusions already established:

- `update` is strongly worthwhile for sparse writes
- speedup scales roughly with the inverse of changed-shard count
- higher fanout magnifies the value of `update`

Representative result:

- `10x4_1m`, `1` changed shard: about `12.1x` faster than full `encode`

## 2. `decode_idx` vs one-shot `reconstruct_some`

See:

- `docs/ec-decode-idx-benchmark-results-2026-05-28.md`

Key conclusions:

- `4x2_64k`: `decode_idx` is effectively at parity in the latest run at about `1.03x`
- `4x2_4m`: `decode_idx` is slower at about `0.86x`
- `10x4_1m`: `decode_idx` is slower than one-shot `reconstruct_some` at about `0.90x`
- `32x16_1m`: `decode_idx` is nearly at parity at about `0.99x`
- `32x16_4m`: `decode_idx` is effectively at parity at about `1.00x`

Interpretation:

- the incremental API is already viable
- after the reduced-column + small-output optimization, even `4x2_64k` is now in the same broad performance band as
  one-shot recovery
- the remaining performance concern is now mostly about polishing smaller-fanout cases rather than proving viability
- shard size alone does not remove the small-fanout penalty, but larger fanout continues to amortize the fixed
  progressive overhead well
- a repeated-call planning-cache experiment for `decode_idx` was evaluated and then rolled back because it did not show
  a stable win on `4x2_64k` or `10x4_1m`

## 3. Current worktree vs clean baseline commit

See:

- `docs/main-vs-origin-main-performance-2026-05-28.md`

Important framing:

- shared operations can be directly compared against the clean baseline
- new APIs like `update` and `decode_idx` cannot be directly compared to the baseline because they do not exist there

## Current Risks / Non-Goals

## 1. Generic reconstruct-planner unification is still intentionally deferred

The `ReconstructShard<F>` generic path has not been fully unified because:

- borrow/lifetime constraints are materially harder there
- the best value so far has come from the specialized `Option<Vec<u8>>` path

## 2. `decode_idx` is a first-pass capability, not a final optimized path

Current design choice:

- correctness and capability first
- performance baseline established
- optimization still open

## 3. Matrix-mode expansion is complete enough for current goals, but not fully generalized

Current status:

- practical and explicit enough for this phase
- still not pushing into Leopard or broader codec-family redesign

## Recommended Next Steps

Priority-ordered suggestions:

1. optimize `decode_idx(...)` for reduced-column, small-output cases now that a benchmark baseline exists
2. decide whether to add more `decode_idx` benchmark shapes before optimizing
3. only after that revisit whether generic reconstruct-planner unification is worth the complexity

## Reading Guide

If someone needs to continue from this phase:

- for `update`:
  - `docs/task-10-classic-parity-update-api.md`
  - `docs/ec-update-benchmark-results-2026-05-28.md`
- for reconstruct planner:
  - `docs/task-11-required-only-reconstruct-copy-elision.md`
  - `docs/task-12-reconstruct-plan-unification.md`
- for progressive decode:
  - `docs/task-13-progressive-decode-idx.md`
  - `docs/ec-decode-idx-benchmark-results-2026-05-28.md`
- for baseline comparison:
  - `docs/main-vs-origin-main-performance-2026-05-28.md`
