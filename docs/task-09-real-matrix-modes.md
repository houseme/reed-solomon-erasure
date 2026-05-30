# Task 09: Real Matrix Modes

## 1. Goal

Turn `MatrixMode` from an API placeholder into real behavior with explicit compatibility semantics.

Today `MatrixMode` is publicly exposed, but `build_matrix_with_options()` still routes all modes through the same
classic matrix builder. That creates a contract gap between the public API surface and the actual implementation.

This task closes that gap.

## 2. Why This Task Exists

Current code exposes:

- `CodecOptions`
- `MatrixMode::{Vandermonde, Cauchy, JerasureLike, Custom}`

but current behavior in `src/core.rs` effectively treats all modes as classic.

That causes two problems:

1. The API suggests choice that does not exist.
2. Future compatibility decisions become harder because callers may already assume these modes are meaningful.

## 3. Current Code Anchors

Primary anchors:

- `src/core.rs`
  - `build_matrix()`
  - `build_matrix_with_options()`
- `src/matrix.rs`
  - basic matrix operations
- `src/tests/mod.rs`
  - options and matrix-related tests

## 4. Compatibility Rule

The default classic mode must remain the MinIO-compatible baseline.

That means:

- `MatrixMode::Vandermonde` remains default
- any non-classic mode must be clearly documented as output-altering
- README and docs must state that `Cauchy`, `JerasureLike`, and `Custom` are not classic-output compatible

## 5. Scope

## 5.1 Required in this task

- real `Vandermonde`
- real `Cauchy`
- real `JerasureLike`
- explicit `Custom` semantics

## 5.2 Explicitly not required

- PAR1 mode as a separate public enum variant
- Leopard family integration
- auto mode switching

## 6. API Contract Decisions

## 6.1 `Custom` semantics

Decide and document exactly one meaning.

Recommended meaning:

- `Custom` supplies parity rows only
- identity rows for data shards are still constructed internally

Reason:

- this matches the most common interoperability use case
- it avoids forcing callers to build the full generator matrix manually

Implemented shape in this repository:

- `with_options(... matrix_mode = MatrixMode::Custom ...)` without payload is rejected with `Error::InvalidCustomMatrix`
- custom parity rows are supplied through a dedicated constructor:

```rust
ReedSolomon::with_custom_matrix(data_shards, parity_shards, &parity_rows, options)
```

This keeps the public `CodecOptions` small while still making `Custom` real and explicit.

## 6.2 `JerasureLike` semantics

Implement a true Jerasure-style matrix construction compatible with the chosen interpretation and document:

- why it exists
- that it changes parity output relative to classic mode
- that it is for explicit interoperability or experimentation only

## 6.3 `Cauchy` semantics

Implement true Cauchy row generation and document:

- startup/build advantage
- output incompatibility with classic mode
- expected use cases

## 7. Implementation Strategy

## 7.1 Refactor matrix construction entry points

Recommended internal decomposition:

```rust
fn build_classic_matrix(...)
fn build_cauchy_matrix(...)
fn build_jerasure_like_matrix(...)
fn build_custom_matrix(...)
fn build_matrix_with_options(...)
```

This makes mode behavior reviewable and benchmarkable.

## 7.2 Keep classic path untouched

Classic mode must keep exact existing behavior.

That means:

- do not "clean up" classic generation logic in the same patch unless byte identity is proven
- benchmark and golden-vector outputs for classic mode must remain unchanged

## 7.3 Validation-first default

Before landing any mode changes:

- add tests that prove classic mode is unchanged
- add tests that prove non-classic modes differ where expected

## 8. Detailed Execution Steps

1. Refactor `build_matrix_with_options()` into per-mode helpers.
2. Preserve the current implementation as the classic builder.
3. Implement Cauchy builder.
4. Implement Jerasure-like builder.
5. Define and implement custom parity-row semantics.
   Current implementation detail:
   use `with_custom_matrix(...)` as the payload-bearing constructor and reject payload-free `MatrixMode::Custom`.
6. Add tests for mode-specific construction.
7. Add compatibility warnings to docs and README.

## 9. Test Plan

## 9.1 Correctness tests

Add tests for:

- classic matrix rows remain equal to old baseline
- Cauchy matrix builds successfully for representative sizes
- Jerasure-like matrix builds successfully for representative sizes
- custom parity rows are embedded correctly
- each mode still reconstructs correctly within its own encoding domain
- invalid custom matrix shapes are rejected

## 9.2 Compatibility tests

Add tests showing:

- classic output equals the prior classic output
- Cauchy output differs from classic for the same input when parity count > 1
- Jerasure-like output differs from classic where expected

## 9.3 Golden-vector handling

Golden vectors should remain tied to classic mode unless intentionally expanded.

Do not silently widen existing golden-vector assertions to multiple modes.

## 10. Benchmarks

At minimum ensure:

```bash
cargo test
cargo test --test golden_vectors
```

If matrix-build cost is interesting, optionally add construction-only microbenchmarks for:

- classic
- Cauchy
- Jerasure-like

## 11. Acceptance Criteria

This task is complete when:

1. every public `MatrixMode` variant has real behavior
   Current interpretation:
   `Custom` is realized through `with_custom_matrix(...)`, while payload-free `MatrixMode::Custom` is an explicit error
2. default classic behavior stays unchanged
3. docs clearly label compatibility impact
4. tests prove classic stability and non-classic distinctness

## 12. Risks

### R1. Accidental classic-output regression

Mitigation:

- keep classic builder isolated
- use explicit regression tests

### R2. `Custom` semantics remain ambiguous

Mitigation:

- pick one definition now
- document it in code and docs

### R3. Users accidentally choose non-classic modes

Mitigation:

- add warnings in README and docs
- consider rustdoc notes on each non-classic mode

## 13. Rollout Guidance

Suggested PR title:

- `task09: implement real matrix mode behavior`

Keep this task separate from parity-update and decode work because matrix semantics are compatibility-sensitive.
