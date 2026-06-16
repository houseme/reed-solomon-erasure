# EPYC Small-file Rerun Guidance (2026-06-16)

## 1. Why this note exists

On the `AMD EPYC 9V45` `x86_64` host, a full one-shot `extended` small-file run
produced several apparent latency regressions versus the archived
`2026-05-27-x86_64-linux-extended` baseline.

However, rerunning the suspicious cases with:

- `RSE_SMALL_FILE_CASE_FILTER=...`
- `RSE_SMALL_FILE_ITERATIONS=40`

showed that most of those regressions were run-shape noise rather than stable
kernel slowdowns.

## 2. Practical rule

When an `x86_64` small-file `extended` run on this host flags regressions:

1. Do not treat the first full-matrix result as sufficient evidence.
2. Rerun the suspicious cases in isolation.
3. Use higher iteration counts before deciding to change EC internals.

## 3. Recommended command

```bash
RSE_SMALL_FILE_PROFILE=extended \
RSE_SMALL_FILE_CASE_FILTER=4x2_1k,4x2_4k,4x2_16k,4x2_64k,10x4_64k,10x4_256k \
RSE_SMALL_FILE_ITERATIONS=40 \
cargo test --release --features "std simd-accel" \
  --test benchmark_small_files \
  benchmark_small_file_matrix_runs_and_exports_results -- --ignored --nocapture
```

## 4. What this changes

This guidance does **not** reduce small-file coverage.

It only changes how suspicious points should be validated before code changes are
approved on this host.
