# Task 10: Classic Parity Update API

## 1. Goal

Add a classic-path `update` API that refreshes parity shards after sparse data-shard changes without requiring a full
re-encode of all data shards.

The updated parity output must be byte-identical to running a full `encode` on the new full shard set.

## 2. Why This Task Exists

Current APIs support:

- full encode
- single-shard progressive encode
- shard-by-shard encode

But none directly support the common operational case:

- parity exists already
- only a few data shards change
- caller wants to update parity cheaply

This is a meaningful gap for storage engines and write-heavy systems.

## 3. Current Code Anchors

Primary anchors:

- `src/core/encode.rs`
  - encode, delta-apply helper, and `update`
- `src/lib.rs`
  - public exports
- `src/tests/mod.rs`
  - API and correctness tests
- `benches/throughput_matrix.rs`
  - place to add sparse-update benchmarking

## 4. Compatibility Rule

This task must preserve classic shard payload compatibility.

That means:

- parity after `update` must equal parity after full `encode`
- no matrix semantics change
- no shard order change
- no padding rule change

## 5. Proposed Public API

Recommended first-pass API:

```rust
pub fn update<T, U>(
    &self,
    old_data: &[T],
    new_data: &[Option<T>],
    parity: &mut [U],
) -> Result<(), Error>
where
    T: AsRef<[F::Elem]>,
    U: AsRef<[F::Elem]> + AsMut<[F::Elem]>;
```

Alternative container-oriented API:

```rust
pub fn update_shards<T>(
    &self,
    old_shards: &[T],
    new_data: &[Option<T>],
    parity: &mut [T],
) -> Result<(), Error>;
```

The first-pass goal is a small, explicit API rather than maximum polymorphism.

Implemented first-pass shape:

```rust
pub fn update<T, U>(
    &self,
    old_data: &[T],
    new_data: &[Option<T>],
    parity: &mut [U],
) -> Result<(), Error>
where
    T: AsRef<[F::Elem]>,
    U: AsRef<[F::Elem]> + AsMut<[F::Elem]>;
```

## 6. Recommended Semantics

- `old_data[i]` is the previous data shard at index `i`
- `new_data[i] == None` means shard `i` is unchanged
- `new_data[i] == Some(..)` means shard `i` changed to the supplied contents
- `parity` contains the old parity on input and the new parity on output

Equivalent formula:

- `new_parity = old_parity XOR encode(delta_rows)`

where each `delta_row = old_data[i] XOR new_data[i]` under GF(2^8) addition semantics.

## 7. Implementation Strategy

## 7.1 Core algebra

For each changed data shard:

1. compute delta = old XOR new
2. apply the same parity coefficients that would be used by `encode_single`
3. XOR the resulting contribution into existing parity

This should reuse the codec's existing parity-row logic instead of inventing a separate path.

## 7.2 Internal helper structure

Recommended helper layering:

```rust
fn update_single_delta(...)
fn update_delta_set(...)
pub fn update(...)
```

This keeps the public API small and the hot path easier to benchmark.

## 7.3 Fast path opportunities

- if no shards changed, return early
- if one data shard changed, use a direct single-delta path
- if one-parity fast mode is active, update via XOR-only fast path

## 7.4 Error handling

The API should reject:

- wrong data count
- wrong parity count
- length mismatch between shards
- changed shard with wrong length
- empty shards

Current first-pass behavior:

- `new_data.len() != data_shard_count` is rejected
- parity length mismatch is rejected through the existing parity count checks
- changed shard length mismatch is rejected before any parity mutation

## 8. Detailed Execution Steps

1. Define the exact API signature and document caller expectations.
2. Add input validation mirroring existing encode checks.
3. Implement per-changed-shard delta application.
4. Add fast path for one-parity mode.
5. Add tests comparing `update` against full `encode`.
6. Add benchmark cases for sparse updates.
7. Document compatibility guarantee.

## 9. Test Plan

## 9.1 Correctness tests

At minimum cover:

- zero changed shards
- one changed data shard
- two changed data shards
- all data shards changed
- one-parity mode
- multi-parity mode
- error on length mismatch
- error on wrong changed-shard count
- error on wrong parity count
- error on empty old data shards

## 9.2 Equivalence tests

For each case:

1. encode original data
2. clone old parity
3. mutate selected data shards
4. run `update`
5. separately run full `encode` on the new full data
6. assert parity bytes are identical

## 9.3 Property-style coverage

If practical, add randomized equivalence tests for representative sizes.

## 10. Benchmark Plan

Add sparse-update cases to `benches/throughput_matrix.rs` or a dedicated bench:

- 10x4 1MiB one changed shard
- 10x4 1MiB two changed shards
- 32x16 1MiB one changed shard
- compare against full `encode`

Current first-pass benchmark:

- `throughput_matrix_update` has been added for the smoke cases
- it measures a single changed shard update against the same shard-size/data-shard throughput framing used elsewhere
- a paired comparison export is available via:
  - `cargo test benchmark_update_vs_encode_4x2_64k_exports_results -- --nocapture`
  - `cargo test benchmark_update_vs_encode_10x4_1m_exports_results -- --nocapture`
  - `cargo test benchmark_update_vs_encode_32x16_1m_exports_results -- --nocapture`
  - results note: `docs/ec-update-benchmark-results-2026-05-28.md`

Primary question:

- does `update` materially beat full `encode` when change set is small?

Current answers:

- `4x2_64k`
  - `1` changed shard: about `5.7x` faster than full `encode`
  - `2` changed shards: about `2.8x` faster than full `encode`
  - `3` changed shards: about `1.8x` faster than full `encode`
  - `4` changed shards: about `1.4x` faster than full `encode`
- `4x2_4m`
  - `1` changed shard: about `5.3x` faster than full `encode`
  - `2` changed shards: about `2.7x` faster than full `encode`
  - `3` changed shards: about `1.8x` faster than full `encode`
  - `4` changed shards: about `1.3x` faster than full `encode`
- `10x4_1m`
  - `1` changed shard: about `12.1x` faster than full `encode`
  - `2` changed shards: about `6.1x` faster than full `encode`
  - `3` changed shards: about `4.1x` faster than full `encode`
  - `4` changed shards: about `3.0x` faster than full `encode`
- `10x4_4m`
  - `1` changed shard: about `12.1x` faster than full `encode`
  - `2` changed shards: about `6.1x` faster than full `encode`
  - `3` changed shards: about `4.0x` faster than full `encode`
  - `4` changed shards: about `3.0x` faster than full `encode`
- `32x16_1m`
  - `1` changed shard: about `34.6x` faster than full `encode`
  - `2` changed shards: about `17.3x` faster than full `encode`
  - `3` changed shards: about `10.7x` faster than full `encode`
  - `4` changed shards: about `8.7x` faster than full `encode`
- `32x16_4m`
  - `1` changed shard: about `34.0x` faster than full `encode`
  - `2` changed shards: about `17.0x` faster than full `encode`
  - `3` changed shards: about `11.3x` faster than full `encode`
  - `4` changed shards: about `8.5x` faster than full `encode`

## 11. Acceptance Criteria

This task is complete when:

1. `update` is public and documented
2. parity output is byte-identical to full re-encode
3. one-parity mode remains correct
4. sparse-update benchmarks show expected value

Current status:

- satisfied for the first three paired comparison cases (`4x2_64k`, `10x4_1m`, `32x16_1m`)
- initial `1..=4` changed-shard scaling curve is now available for both `1m` and `4m` variants of the measured topologies
- still needs expansion to additional shard sizes and changed-shard counts beyond `4`

## 12. Risks

### R1. Delta algebra bug

Mitigation:

- equivalence-to-full-encode tests are mandatory

### R2. Over-generalized API increases complexity

Mitigation:

- ship a narrow first-pass API
- expand only after real use cases appear

### R3. Hidden compatibility regression

Mitigation:

- make output equivalence the primary acceptance gate

## 13. Rollout Guidance

Suggested PR title:

- `task10: add classic parity update api`

Suggested PR split:

1. API + correctness tests
2. benchmark additions
3. documentation polish
