# Collection Guide

This folder keeps machine-specific benchmark captures used to validate x86_64 runtime-dispatch ordering.

## Expected inputs

For each machine, collect:

1. `cargo bench --bench galois_backend --features 'std simd-accel'`
2. `cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
3. `lscpu`

## Required backend overrides

Capture at least:

1. `rust-avx2`
2. `rust-avx512`
3. `rust-gfni-avx2`
4. `rust-gfni-avx512` when supported
5. `simd-c`
6. `scalar`
7. `auto`

## Output convention

Store one JSON file per machine:

`YYYY-MM-DD-<cpu-slug>.json`

The JSON should include:

1. machine identity
2. CPU feature list
3. criterion benchmark snapshots
4. release smoke benchmark snapshots
5. enough metadata to reproduce the run
