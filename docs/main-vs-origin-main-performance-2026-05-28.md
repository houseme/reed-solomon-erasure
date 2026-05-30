# Main Vs Origin Main Performance 2026-05-28

## Goal

This note compares the current working tree against the remote `origin/main` commit for shared performance paths and
records the current absolute performance of newly added APIs that do not exist on the baseline commit.

## Compared Revisions

- baseline `origin/main`: `fa49e2a4211b14992e1ebd7f737445d6337d1771`
- current working tree: local modifications on top of the same commit

Important context:

- `HEAD` and `origin/main` point to the same commit
- the comparison is therefore:
  - baseline worktree at the clean commit
  - current dirty working tree with local changes

## Method

Baseline worktree:

```bash
git worktree add /private/tmp/reed-solomon-erasure-main-baseline fa49e2a4211b14992e1ebd7f737445d6337d1771
```

Commands run on the baseline worktree:

```bash
cargo test benchmark_smoke_matrix_runs_and_exports_results -- --nocapture
```

Commands run on the current working tree:

```bash
cargo test benchmark_smoke_matrix_runs_and_exports_results -- --nocapture
cargo test benchmark_update_vs_encode_4x2_64k_exports_results -- --nocapture
cargo test benchmark_update_vs_encode_10x4_1m_exports_results -- --nocapture
cargo test benchmark_update_vs_encode_32x16_1m_exports_results -- --nocapture
```

## Scope Boundary

Only operations present on both trees can be directly compared:

- `encode`
- `verify`
- `reconstruct`
- `reconstruct_data`

New APIs added locally cannot be directly compared against `origin/main` because they do not exist there:

- `update`
- `decode_idx`
- newer specialized reconstruct planner changes exposed only internally

For those, this note records current absolute measurements only.

## Shared-Operation Smoke Comparison

Source files:

- baseline:
  - `/private/tmp/reed-solomon-erasure-main-baseline/target/benchmark-smoke/smoke-results.csv`
- current:
  - `target/benchmark-smoke/smoke-results.csv`

## `4x2_64k`

| Operation | Baseline MB/s | Current MB/s | Relative |
|---|---:|---:|---:|
| `encode` | `21.5080` | `40.3483` | `1.88x` |
| `verify` | `15.7775` | `27.0434` | `1.71x` |
| `reconstruct` | `18.8565` | `26.9948` | `1.43x` |
| `reconstruct_data` | `21.3563` | `26.6134` | `1.25x` |

## `10x4_1m`

| Operation | Baseline MB/s | Current MB/s | Relative |
|---|---:|---:|---:|
| `encode` | `29.4929` | `28.5220` | `0.97x` |
| `verify` | `18.1708` | `17.4024` | `0.96x` |
| `reconstruct` | `22.3462` | `21.5924` | `0.97x` |
| `reconstruct_data` | `22.5885` | `21.6331` | `0.96x` |

## Interpretation For Shared Operations

### Small-shard case

The current working tree is clearly better on the `4x2_64k` smoke case across all shared operations.

### Mid-size case

On `10x4_1m`, the current working tree is roughly flat to slightly below the clean baseline on the shared operation
smoke numbers.

### What this means

The current local changes do not show a broad regression disaster on shared operations, but they also do not produce a
clean across-the-board win on the mid-size shared smoke case. The worktree currently looks like:

- strong gains on small shared cases
- near-flat shared behavior on `10x4_1m`

This is consistent with the recent direction of adding targeted APIs and specialized internal fast paths rather than
trying to globally speed up every shared operation at once.

## New API: `update`

There is no baseline comparison because `origin/main` at `fa49e2a4211b14992e1ebd7f737445d6337d1771` does not provide
the `update` API.

Current worktree paired-comparison artifacts:

- `target/benchmark-smoke/update-vs-encode-4x2_64k.csv`
- `target/benchmark-smoke/update-vs-encode-10x4_1m.csv`
- `target/benchmark-smoke/update-vs-encode-32x16_1m.csv`
- plus their `4m` variants

See:

- `docs/ec-update-benchmark-results-2026-05-28.md`

Key takeaway:

- `update` is a high-value additive API in the current worktree
- it should be judged against full `encode` on the same tree, not against `origin/main`

## New API: `decode_idx`

There is no baseline comparison because `origin/main` does not provide `decode_idx`.

Current status:

- implementation exists in `src/galois_8/policy.rs`
- functional tests cover progressive mode, merge mode, and key error paths

Key takeaway:

- `decode_idx` is currently a correctness-validated capability addition
- performance benchmarking for it should be treated as a local API benchmark, not a direct baseline comparison

## Recommendation

Use this note as the decision reference:

1. for shared operations, compare against the baseline smoke tables above
2. for newly added APIs, compare within the current worktree against their nearest legacy equivalent
3. do not interpret missing baseline rows for `update` or `decode_idx` as measurement gaps; they are capability gaps
   in the baseline revision
