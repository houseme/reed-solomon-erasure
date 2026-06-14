# EC AArch64 Validation Review (2026-06-14)

## 1. Scope

This pass re-validated the current `main` branch on Apple Silicon / `aarch64-apple-darwin`
with two goals:

1. confirm whether the current EC benchmark and validation flow still works end-to-end
2. review whether the recent benchmark / verify changes had drifted into patch-on-patch fixes

Referenced materials:

- `docs/ec-small-file-benchmark-playbook.md`
- `docs/benchmark-methodology.md`
- `benchmarks/small-file/2026-05-27-aarch64-apple-silicon-extended.csv`

## 2. What Was Validated

Environment:

- host arch: `arm64`
- Rust host triple: `aarch64-apple-darwin`
- rustc: `1.96.0`
- current HEAD: `d1edce6`

Commands executed:

```bash
cargo clippy --all-targets --all-features -- -D warnings

bash scripts/run_aarch64_backend_smoke_matrix.sh

RSE_SMALL_FILE_PROFILE=fast \
bash scripts/run_small_file_benchmark_matrix.sh

RSE_SMALL_FILE_PROFILE=extended \
cargo test --release --features "std simd-accel" \
  --test benchmark_small_files \
  benchmark_small_file_matrix_runs_and_exports_results -- --ignored --nocapture

RSE_SMALL_FILE_PROFILE=extended \
RSE_SMALL_FILE_CASE_FILTER=10x4_1k \
RSE_SMALL_FILE_ITERATIONS=40 \
cargo test --release --features "std simd-accel" \
  --test benchmark_small_files \
  benchmark_small_file_matrix_runs_and_exports_results -- --ignored --nocapture

RSE_SMALL_FILE_PROFILE=extended \
RSE_SMALL_FILE_CASE_FILTER=4x2_1k,10x4_512k \
RSE_SMALL_FILE_ITERATIONS=40 \
cargo test --release --features "std simd-accel" \
  --test benchmark_small_files \
  benchmark_small_file_matrix_runs_and_exports_results -- --ignored --nocapture
```

## 3. Review Findings

### 3.1 Small-file coverage is required, not optional

After re-reading the playbook and methodology, the answer is yes:
small-file EC performance must be evaluated explicitly for
`1 KiB / 4 KiB / 16 KiB / 64 KiB / 128 KiB / 256 KiB / 512 KiB`.

Those sizes are already part of the supported `small-file` matrix, so the real task
was not to add coverage, but to make sure the validation entrypoints still executed it correctly.

### 3.2 The validation scripts had drifted out of sync with ignored benchmark tests

Recent test governance changes marked the artifact-producing smoke and small-file tests as `#[ignore]`.
However, several scripts and docs still invoked them without `--ignored`.

Impact:

- `scripts/run_aarch64_backend_smoke_matrix.sh` looked like it was validating backend smoke, but it actually skipped the benchmark test and then failed when copying missing artifacts
- `scripts/run_x86_backend_smoke_matrix.sh` had the same issue
- `scripts/collect_x86_simd_benchmarks.sh` inherited the same broken smoke call
- `scripts/run_small_file_benchmark_matrix.sh` and `scripts/release-check.sh` also needed the ignored benchmark entrypoint

This was the clearest patch-on-patch symptom in the current branch: the test governance change landed,
but the benchmark runners and docs were not fully reconciled afterward.

### 3.3 Leopard verify reuse was only half-landed

The public API had already added:

- `verify_with_buffer`
- `verify_with_workspace`
- `verify_with_workspace_opt`

But Leopard-family verify still fell back to an internal helper that allocated fresh `Vec<Vec<_>>`
buffers instead of reusing caller-provided parity scratch.

That meant the interface advertised a reusable fast path, while the Leopard branch quietly bypassed it.

The current fix closes that gap:

- `verify_with_buffer` now reuses caller buffers on Leopard
- `verify_with_workspace` now reuses `VerifyWorkspace` on Leopard
- `verify_with_workspace_opt` now dispatches to workspace-backed Leopard verify instead of plain `verify()`

This is the structural refactor from “patch-on-patch” to one consistent execution path.

## 4. Benchmark Interpretation

### 4.1 Archived baseline limitations

The archived AArch64 extended baseline from `2026-05-27` does **not** include `verify_with_buffer`.
So it is still useful for `encode / verify / reconstruct / reconstruct_data`,
but incomplete for judging the newer verify-buffer reuse path.

### 4.2 Single-run extended comparisons are still noisy

A fresh one-shot `extended` comparison against the archived baseline produced several apparent regressions,
including `4x2_1k` and `10x4_512k`.

Those points were not trustworthy enough to justify kernel changes because:

1. the regressions were not shape-consistent across neighboring sizes
2. the same session already showed that ultra-small cases were especially sensitive to iteration count
3. targeted high-iteration reruns contradicted the one-shot regression signal

### 4.3 High-iteration drill-down says “do not optimize the wrong thing”

Focused reruns with `RSE_SMALL_FILE_ITERATIONS=40` showed:

- `10x4_1k encode`: `3383.20 ns` vs archived baseline `3375.00 ns`
  - effectively flat, not a real regression
- `4x2_1k verify`: `432.27 ns` in the filtered rerun artifact
  - this confirms the point is extremely sensitive to run shape and must not be judged from a low-iteration one-shot run
- `10x4_512k verify`: `566816.65 ns` in the filtered rerun artifact
  - again inconsistent with the earlier one-shot “regression” signal

Conclusion:

- current code does **not** show a stable, reproducible small-file regression that justifies a new EC hot-path optimization pass today
- the bigger risk was benchmark procedure drift, not a newly introduced kernel slowdown

## 5. Final Judgment

### 5.1 Do we need to consider small-file speed?

Yes. That is part of the standard validation surface and should remain so.

### 5.2 Do we need an immediate new kernel optimization based on the current data?

No, not from the evidence gathered in this pass.

The current branch needed:

1. validation entrypoint repairs
2. Leopard verify buffer/workspace path consolidation
3. clearer methodology for rerunning suspicious `1 KiB` / `4 KiB` outliers

Those were higher-confidence improvements than trying to tune EC internals from noisy one-shot results.

### 5.3 Is the branch still patch-on-patch?

Before this pass: partially yes.

The strongest examples were:

- ignored benchmark tests without matching script/doc updates
- Leopard verify APIs exposing reuse semantics that Leopard dispatch did not actually honor

After the changes in this pass, those mismatches are materially reduced.

## 6. Follow-up Recommendation

1. Keep the current archived `2026-05-27` AArch64 baseline for historical comparison, but do not refresh it from a noisy mixed-load run.
2. If a future `extended` run flags only `1 KiB` / `4 KiB` outliers, rerun the case with `RSE_SMALL_FILE_CASE_FILTER` and a higher iteration count before changing code.
3. When the team wants a new baseline refresh, run a quiet dedicated session and archive both JSON and CSV together so `verify_with_buffer` is included in the next AArch64 baseline generation.
