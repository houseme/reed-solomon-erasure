# Benchmark Methodology

## 1. Scope

This document standardizes how to run, collect, and compare benchmark results for this crate.

Targets:

- smoke regression (`tests/benchmark_smoke.rs`)
- throughput matrix (`benches/throughput_matrix.rs`)
- backend kernel benchmarks (`benches/galois_backend.rs`)

## 2. Baseline Commands

Smoke (fast regression):

```bash
cargo test --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture
```

Throughput matrix:

```bash
cargo bench --bench throughput_matrix --features std
```

Backend kernels:

```bash
cargo bench --bench galois_backend --features std
```

SIMD smoke (when available):

```bash
cargo test --features "std simd-accel" --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture
```

## 3. Determinism Rules

- Use fixed seeds from `benches/common/mod.rs` (`BASE_SEED` + `derived_seed`).
- Do not mix old and new build artifacts when comparing results.
- For A/B comparisons, run both sides from clean builds.

## 4. Output Artifacts

Smoke outputs:

- `target/benchmark-smoke/smoke-results.json`
- `target/benchmark-smoke/smoke-results.csv`

Cache analysis outputs (tests):

- `target/benchmark-smoke/reconstruction-cache-stats.json`
- `target/benchmark-smoke/reconstruction-cache-patterns.csv`
- `target/benchmark-smoke/reconstruction-hotspot-results.json`
- `target/benchmark-smoke/reconstruction-hotspot-results.csv`

Regression gate inputs / outputs:

- baseline artifact: user-provided `smoke-results.json` via `RSE_SMOKE_BASELINE`
- current artifact: `target/benchmark-smoke/smoke-results.json`
- consistency sweep: `scripts/check_backend_consistency.sh`

Throughput profiling output (optional):

- `target/benchmark-smoke/throughput-profile-report.json` (when `RSE_WRITE_PROFILE_REPORT=1`)

## 4.1 Unified Artifact Schema

Every benchmark or profiling artifact should now expose a stable core envelope so
results from different phases can be compared without guessing field meaning.

Required core fields when applicable:

- `schema_version`
- `artifact_kind`
- `benchmark_metrics_enabled`
- `git_revision`
- `target_triple`
- `features`
- `backend`
- `backend_id`
- `backend_kind`
- `backend_override`
- `operation`
- `data_shards`
- `parity_shards`
- `shard_size`

Artifact-specific fields remain allowed, but the core envelope should stay stable.

Current artifact mapping:

- `smoke-results`: regression-oriented throughput snapshot
- `parallel-helper-results`: serial vs optimized helper comparison
- `reconstruction-hotspot-results`: reconstruct workload comparison
- `reconstruction-cache-stats`: cache observability snapshot
- `reconstruction-cache-patterns`: cache workload comparison
- `throughput-profile-report`: runtime/profile counter export

Current schema version:

- `schema_version = 1`

Notes:

- JSON artifacts should always include `schema_version` and `artifact_kind`.
- CSV artifacts should keep a fixed leading column order that starts with
  `schema_version,artifact_kind`.
- Scripts may ignore extra fields, but should not assume field order beyond the
  documented CSV header.

## 5. Comparison Protocol

1. Record commit hash and feature set.
2. Keep backend selection explicit:
   - default runtime dispatch, or
   - force backend via `RSE_BACKEND_OVERRIDE`.
3. For each operation (`encode`, `verify`, `reconstruct`, `reconstruct_data`), compare:
   - throughput (`throughput_mb_s`)
   - latency (`ns_per_iter`) when available
4. Use median of repeated runs for decisions; avoid single-run conclusions.

## 6. Cache Metrics Interpretation

For reconstruction cache stats:

- `requests`: total decode-matrix lookup attempts
- `hits`: cache hits
- `misses`: cache misses
- `inserts`: inserted decode-matrix entries
- `evictions`: entries evicted by LRU capacity pressure

Derived analysis fields:

- `hit_rate = hits / requests`
- `reuse_ratio = hits / inserts`
- `miss_cost_per_request = misses / requests`

## 7. Recommended Release Checklist

Use the fixed script:

```bash
./scripts/release-check.sh
```

This script runs test gates for:

- unit/integration correctness
- self-test entry
- benchmark smoke
- smoke regression gate (when `RSE_SMOKE_BASELINE` is set)
- backend/ISA consistency sweep (when `RUN_BACKEND_CONSISTENCY=1`)
- `no_std` build path
- `std` and optional `simd-accel` paths

## 8. Smoke Regression Gate

Compare a fresh smoke run against a checked-in or archived baseline:

```bash
RSE_SMOKE_BASELINE=/abs/path/to/smoke-results.json \
python3 scripts/check_benchmark_regression.py \
  --baseline "$RSE_SMOKE_BASELINE" \
  --current target/benchmark-smoke/smoke-results.json
```

Default allowed regressions:

- `encode`: 10%
- `verify`: 12%
- `reconstruct`: 15%
- `reconstruct_data`: 15%

Thresholds can be overridden per operation:

```bash
python3 scripts/check_benchmark_regression.py \
  --baseline old.json \
  --current new.json \
  --threshold reconstruct=0.18 \
  --threshold reconstruct_data=0.18
```

## 8.1 Baseline Update Governance

Updating a benchmark baseline is allowed only when the new baseline reflects an
intentional and verified steady-state change.

Allowed cases:

1. An intentional algorithm or scheduling change has landed and the previous
   baseline no longer represents the intended default behavior.
2. Backend priority or dispatch behavior changed intentionally and the new
   default path is the one being validated.
3. A benchmark schema migration occurred, and the old artifact can no longer be
   compared mechanically without lossy translation.

Disallowed cases:

1. A single noisy run looks slower or faster than expected.
2. Different machines, CPUs, or feature sets are being compared as if they were
   the same baseline lineage.
3. The current run used a different backend override, different workload shape,
   or different build cleanliness than the archived baseline without explicit
   justification.

Minimum evidence before refreshing a baseline:

1. Same-machine comparison
2. Matching feature set
3. Matching or explicitly documented backend selection
4. Clean-build comparison when performance-sensitive conclusions are involved
5. Repeated runs with median-oriented interpretation
6. Short rationale describing why the old baseline is no longer authoritative

Recommended baseline refresh note template:

- reason:
- old baseline:
- new baseline:
- machine / cpu:
- feature set:
- backend mode:
- repeated-run summary:
- expected long-term effect:

## 9. Backend Consistency Sweep

When SIMD is available, run the reusable backend override sweep:

```bash
RUN_BACKEND_CONSISTENCY=1 ./scripts/release-check.sh
```

Or invoke it directly:

```bash
bash scripts/check_backend_consistency.sh
```

## 10. ISA Integration Template

Use this template whenever adding a new SIMD or ISA-specific backend.

Checklist:

1. Add a named backend entry with stable `name`, `id`, and `kind`.
2. Connect runtime dispatch without breaking scalar fallback.
3. Add or update `RSE_BACKEND_OVERRIDE` support for the new backend name.
4. Add scalar correctness comparison tests.
5. Include the backend in reusable consistency sweep scripts when the platform
   can expose it.
6. Validate smoke benchmark output with the new backend selected explicitly.
7. Validate kernel benchmark output where applicable.
8. Document whether the backend is experimental, candidate-default, or
   fallback-only.

Minimum deliverables:

- correctness tests
- backend override coverage
- consistency sweep coverage
- smoke or workload benchmark evidence
- dispatch and fallback notes

## 11. Matrix Mode Integration Template

Use this template whenever extending `MatrixMode` or adding a new matrix
construction strategy.

Checklist:

1. State the target workload or compatibility need.
2. Preserve current default behavior unless the change is explicitly intended to
   alter defaults.
3. Add encode correctness coverage.
4. Add reconstruction correctness coverage.
5. Check interaction with `reconstruct_data` and `reconstruct_some`.
6. Evaluate benchmark impact on at least one representative workload.
7. Update API and governance docs together with the implementation.

Minimum deliverables:

- constructor / option-path coverage
- correctness tests for encode and reconstruct flows
- documented benchmark impact
- rollback story if the mode is not yet ready for general default use

## 12. Metrics Feature Gate

Heavy benchmark and reconstruction metrics are now governed by the
`benchmark-metrics` feature.

Current behavior:

- default features keep `benchmark-metrics` enabled
- disabling default features or explicitly removing `benchmark-metrics` should
  preserve correctness behavior while allowing metric fields to degrade safely
  to zero-valued snapshots

Guidelines:

1. Do not make correctness depend on metrics collection.
2. Benchmark artifacts should record whether metrics were enabled through
   `benchmark_metrics_enabled`.
3. When metrics are disabled, scripts should treat zero-valued counters as
   “metrics unavailable in this build” unless the workload naturally produces
   zeros.
