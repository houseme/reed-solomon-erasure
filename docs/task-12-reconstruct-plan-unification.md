# Task 12: Reconstruct Plan Unification

## 1. Goal

Introduce a shared reconstruction planning layer that separates:

- which shards are present
- which shards are missing
- which outputs are actually required
- which matrix rows are needed
- which execution strategy should be used

from the mechanics of serial or parallel coding.

The immediate objective is to reduce duplicated logic and make future work such as `decode_idx` easier to implement.

## 2. Why This Task Exists

The current code already has good targeted optimizations, especially around small-output reconstruction. However,
reconstruction planning is still spread across multiple branches and path-specific flows:

- `reconstruct_internal`
- `reconstruct_internal_option_vec_par_with_stage_policies`
- `reconstruct_some`
- data-only vs full-reconstruct branches

That makes future feature work more expensive and increases the chance of behavior drift between paths.

## 3. Current Code Anchors

Primary anchors:

- `src/core.rs`
  - `reconstruct_internal()`
  - `reconstruct_internal_option_vec_par_*()`
  - `reconstruct_some()`
  - `get_data_decode_matrix()`

## 4. Desired End State

Planning and execution should become distinct layers.

### Planning layer responsibilities

- validate shard counts and lengths
- collect valid indices
- collect invalid indices
- classify missing data vs missing parity
- classify requested outputs
- resolve decode matrix
- build matrix-row list for requested outputs
- choose stage policies

### Execution layer responsibilities

- choose serial vs parallel coding
- preserve one/two-output optimized data-only paths where profitable
- allocate output buffers
- write results back

## 5. Recommended Internal Types

Introduce focused internal structs, for example:

```rust
struct ReconstructPlan<'a, F: Field> {
    shard_len: usize,
    valid_indices: SmallVec<[usize; 32]>,
    invalid_indices: SmallVec<[usize; 32]>,
    missing_data_indices: SmallVec<[usize; 32]>,
    missing_parity_indices: SmallVec<[usize; 32]>,
    requested_data_indices: SmallVec<[usize; 32]>,
    requested_parity_indices: SmallVec<[usize; 32]>,
    data_decode_matrix: Arc<Matrix<F>>,
}
```

Names can vary, but the shape should make intent obvious.

## 6. Scope Boundaries

This task should not:

- change default matrix semantics
- introduce Leopard
- redesign the public API

This task may:

- move internal logic into helpers
- alter internal planning flow
- preserve specialized execution branches after planning is unified

## 7. Implementation Strategy

## 7.1 First step: extract shared planning

Create a helper that, given shard presence and requested recovery scope, returns a plan object.

That helper should serve:

- full reconstruct
- reconstruct_data
- reconstruct_some
- parallel `Option<Vec<_>>` path

Current applied scope in this repository:

- the first implemented planner is specialized for `Option<Vec<u8>>`
- it now serves:
  - `reconstruct_opt(...)`
  - `reconstruct_data_opt(...)`
  - `reconstruct_some_opt(..., required)` for the data-only path

## 7.2 Second step: keep execution optimized

Do not collapse everything into one generic slow path.

Instead:

- use the plan object to drive the existing optimized one/two-output data-only path
- use general execution for larger output sets
- use stage policies for data and parity when needed

Current specialized shape:

- the `Option<Vec<u8>>` planner already carries:
  - `shard_len`
  - `valid_indices`
  - `invalid_indices`
  - `number_present`
  - `data_decode_matrix`
  - `required_missing_data_indices`
- the specialized execution layer is now also factored into a helper:
  - `execute_option_vec_required_data_plan(...)`
- execution still builds `matrix_rows` and output buffers locally inside that helper, but the expensive
  presence/index/decode-matrix derivation is now shared and the data-only required path has a clear `plan + execute`
  split
- `reconstruct_data_opt(...)` now also consumes the same shared `Option<Vec<u8>>` planner for its presence/index/
  decode-matrix setup

Current code anchor for this specialized planner:

- `src/galois_8/policy.rs`
  - `OptionVecReconstructPlan`
  - `plan_option_vec_reconstruct(...)`
  - `execute_option_vec_required_data_plan(...)`

## 7.3 Third step: normalize output planning

Where practical, construct requested output rows in one place and then choose whether to execute in one pass or two
passes based on policy and hotspot evidence.

## 8. Detailed Execution Steps

1. Extract validation and index-collection logic into a planning helper.
2. Make `reconstruct_some` build requested-output information through the same planner.
3. Make full reconstruct and data-only reconstruct consume the same plan shape.
4. Preserve specialized execution for one/two-output data-only recovery.
5. Re-run hotspot and smoke benchmarks.

Current applied progress:

1. done for `Option<Vec<u8>>` specialized planning
2. done for `reconstruct_some_opt(..., required)` data-only path
3. done for `reconstruct_opt(...)` and `reconstruct_data_opt(...)` on the same specialized planner
4. preserved, and the required-only data execution path now flows through a dedicated specialized execute helper
5. correctness regression tests re-run successfully

## 9. Testing Strategy

## 9.1 Correctness regression tests

Re-run and retain existing tests for:

- `reconstruct`
- `reconstruct_data`
- `reconstruct_some`
- golden vectors if reconstruction flows are covered there

## 9.2 Planner-specific tests

Add focused internal or unit tests for plan derivation:

- all shards present
- too few shards present
- missing data only
- missing parity only
- mixed missing data/parity
- required-only subset recovery

Current regression coverage used for the specialized planner:

- `test_galois_8_reconstruct_data_opt_matches_reconstruct_data`
- `test_galois_8_reconstruct_data_opt_matches_reconstruct_data_for_small_shards`
- `test_galois_8_reconstruct_opt_matches_reconstruct`
- `test_galois_8_reconstruct_some_opt_matches_reconstruct_some_for_data_only`

Current compile/validation status:

- `cargo check --tests`
- `cargo test test_galois_8_reconstruct_data_opt_matches_reconstruct_data`
- `cargo test test_galois_8_reconstruct_data_opt_matches_reconstruct_data_for_small_shards`
- `cargo test test_galois_8_reconstruct_opt_matches_reconstruct`
- `cargo test test_galois_8_reconstruct_some_opt_matches_reconstruct_some_for_data_only`

## 9.3 Benchmark plan

Mandatory commands:

```bash
cargo test --features std benchmark_reconstruction_hotspots
cargo test --test benchmark_smoke --features "simd-accel benchmark-metrics" -- --nocapture
cargo bench --bench throughput_matrix --features "simd-accel benchmark-metrics"
```

## 10. Acceptance Criteria

This task is complete when:

1. reconstruction planning is centralized
2. public behavior remains unchanged
3. specialized hotspot paths remain available where they are proven useful
4. benchmark gates remain neutral or positive

Current status:

- achieved for the specialized `Option<Vec<u8>>` reconstruct path
- `reconstruct_opt`, `reconstruct_data_opt`, and `reconstruct_some_opt(data_only)` now share a common planner
- `reconstruct_some_opt(data_only)` now consumes that planner through a dedicated execute helper
- the specialized planner now covers both common decode-matrix setup and requested-output planning for the data-only
  required path
- not yet generalized to all `ReconstructShard<F>` callers

## 11. Risks

### R1. Over-abstraction hides hot-path behavior

Mitigation:

- keep planner separate from executor
- do not erase specialized execution branches

### R2. Planning helper becomes too generic

Mitigation:

- design for current reconstruct variants first
- expand later only if needed

Current mitigation in practice:

- the planner was intentionally scoped to `Option<Vec<u8>>` first
- this kept the borrow model tractable while still covering the most important high-value fast path

### R3. Benchmark regressions from accidental path flattening

Mitigation:

- benchmark before/after every structural step
- keep small-output special cases explicit

## 12. Rollout Guidance

Suggested PR title:

- `task12: unify reconstruction planning`

If the patch grows too large, split it into:

1. planner extraction
2. executor adoption
3. benchmark/result cleanup
