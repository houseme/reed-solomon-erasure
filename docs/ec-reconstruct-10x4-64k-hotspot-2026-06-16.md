# EC Reconstruct 10x4 64KiB Hotspot A/B (2026-06-16)

## 1. Goal

This pass narrowed the investigation to the `10x4 / 64 KiB / reconstruct`
hotspot that remained suspicious after the broader `x86_64` validation review.

The main question was whether the current full-reconstruct parallel policy is
still too conservative for this case.

## 2. Benchmark Entry

Added a dedicated benchmark export entry:

- `benchmark_reconstruct_policy_10x4_64k_exports_results`

Artifact outputs:

- `target/benchmark-smoke/reconstruct-policy-10x4_64k.json`
- `target/benchmark-smoke/reconstruct-policy-10x4_64k.csv`

## 3. Command

```bash
cargo test --release --features "std simd-accel" \
  --test benchmark_smoke \
  benchmark_reconstruct_policy_10x4_64k_exports_results -- --ignored --nocapture
```

## 4. Compared Variants

1. `reconstruct_serial`
2. `reconstruct_opt_default`
3. `reconstruct_opt_minparallel64k_minjob64k`
4. `reconstruct_data_opt_two_data_missing`

The tuned reconstruct variant used:

- `RS_RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES=65536`
- `RS_RECONSTRUCT_MIN_BYTES_PER_JOB=65536`

## 5. Results

| variant | throughput MB/s | ns/iter | speedup vs serial |
| --- | ---: | ---: | ---: |
| `reconstruct_serial` | `7085.0967` | `88213.33` | `1.0000x` |
| `reconstruct_opt_default` | `8221.3224` | `76021.83` | `1.1604x` |
| `reconstruct_opt_minparallel64k_minjob64k` | `9151.4306` | `68295.33` | `1.2916x` |
| `reconstruct_data_opt_two_data_missing` | `9178.5560` | `68093.50` | `1.2955x` |

## 6. Interpretation

1. `reconstruct_opt_default` is already meaningfully better than pure serial for
   this workload.
2. Lowering the full reconstruct parallel threshold to `64 KiB` for this case
   improves throughput again, from `8221.3224 MB/s` to `9151.4306 MB/s`.
3. The tuned full reconstruct path lands very close to the specialized
   `reconstruct_data_opt` two-missing-data path.

This is the strongest signal from the narrowed pass:

- the remaining `10x4 / 64 KiB / reconstruct` gap is likely policy-bound more
  than kernel-bound

## 7. What Not To Conclude Yet

This is still a single-case result.

It is **not** enough by itself to globally change the default full-reconstruct
parallel policy, because earlier work already showed that more aggressive
parallel thresholds can hurt other small-shard cases.

## 8. Recommended Next Step

Before changing the default policy, run the same A/B shape on at least:

1. `4x2_64k reconstruct`
2. `16x8_64k reconstruct`
3. `32x16_64k reconstruct`

If the `64 KiB` threshold stays positive across that set without reopening old
small-shard regressions, then it becomes a real candidate for a policy update
rather than a case-local experiment.
