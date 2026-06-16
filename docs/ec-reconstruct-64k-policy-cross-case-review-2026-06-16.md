# EC Reconstruct 64KiB Policy Cross-case Review (2026-06-16)

## 1. Goal

Re-run the `64 KiB` reconstruct-policy comparison with stronger measurement
controls, then decide whether the `64 KiB` threshold should become a default
policy or stay experimental.

Compared cases:

1. `4x2_64k`
2. `10x4_64k`
3. `16x8_64k`
4. `32x16_64k`

## 2. What Changed In This Pass

The earlier one-variant-at-a-time harness overstated the benefit of
`reconstruct_opt_*` because benchmark order was not controlled tightly enough.

This pass changed the harness to:

1. run all variants in `round_robin_rotating_start` order
2. record `measurement_strategy`, `measurement_order`, `measurement_iterations`,
   and `warmup_rounds`
3. export explicit `entry_path` plus runtime counters for reconstruct entry and
   stage execution

It also tested a narrow runtime refactor:

1. `reconstruct_opt(...)` and `reconstruct_data_opt(...)` now reuse the already
   computed option-vec reconstruct plan on serial fallback
2. this removes one redundant planning pass, but keeps policy behavior unchanged

## 3. Compared Variants

For every case, the same variants were compared:

1. `reconstruct_serial`
2. `reconstruct_opt_default`
3. `reconstruct_opt_minparallel64k_minjob64k`
4. `reconstruct_data_opt_two_data_missing`

The tuned variant used:

- `RS_RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES=65536`
- `RS_RECONSTRUCT_MIN_BYTES_PER_JOB=65536`

## 4. Results

### 4.1 `4x2_64k`

| variant | throughput MB/s | speedup vs serial | entry path |
| --- | ---: | ---: | --- |
| `reconstruct_serial` | `14732.6033` | `1.0000x` | `reconstruct_direct_serial` |
| `reconstruct_opt_default` | `4700.0580` | `0.3190x` | `reconstruct_opt_fallback_serial` |
| `reconstruct_opt_minparallel64k_minjob64k` | `4512.6761` | `0.3063x` | `reconstruct_opt_fallback_serial` |
| `reconstruct_data_opt_two_data_missing` | `4655.2045` | `0.3160x` | `reconstruct_data_opt_fallback_serial` |

### 4.2 `10x4_64k`

| variant | throughput MB/s | speedup vs serial | entry path |
| --- | ---: | ---: | --- |
| `reconstruct_serial` | `22092.0916` | `1.0000x` | `reconstruct_direct_serial` |
| `reconstruct_opt_default` | `9177.6574` | `0.4154x` | `reconstruct_opt_fallback_serial` |
| `reconstruct_opt_minparallel64k_minjob64k` | `9585.0565` | `0.4339x` | `reconstruct_opt_fallback_serial` |
| `reconstruct_data_opt_two_data_missing` | `9283.0054` | `0.4202x` | `reconstruct_data_opt_fallback_serial` |

### 4.3 `16x8_64k`

| variant | throughput MB/s | speedup vs serial | entry path |
| --- | ---: | ---: | --- |
| `reconstruct_serial` | `18938.5570` | `1.0000x` | `reconstruct_direct_serial` |
| `reconstruct_opt_default` | `11188.8112` | `0.5908x` | `reconstruct_opt_fallback_serial` |
| `reconstruct_opt_minparallel64k_minjob64k` | `11436.1084` | `0.6039x` | `reconstruct_opt_fallback_serial` |
| `reconstruct_data_opt_two_data_missing` | `11298.4988` | `0.5966x` | `reconstruct_data_opt_fallback_serial` |

### 4.4 `32x16_64k`

| variant | throughput MB/s | speedup vs serial | entry path |
| --- | ---: | ---: | --- |
| `reconstruct_serial` | `20152.1826` | `1.0000x` | `reconstruct_direct_serial` |
| `reconstruct_opt_default` | `14432.7392` | `0.7162x` | `reconstruct_opt_fallback_serial` |
| `reconstruct_opt_minparallel64k_minjob64k` | `14811.8890` | `0.7350x` | `reconstruct_opt_fallback_serial` |
| `reconstruct_data_opt_two_data_missing` | `14431.9061` | `0.7161x` | `reconstruct_data_opt_fallback_serial` |

## 5. Hard Findings

1. All four cases stayed on serial entry paths.
2. All four cases recorded `decision_use_parallel=false`.
3. All four cases recorded `runtime_parallel_policy_calls=0`.
4. All four cases recorded `runtime_code_some_parallel_calls=0`.
5. The `64 KiB` candidate never actually crossed the reconstruct entry threshold
   in these runs.

That means the old interpretation was wrong:

- the observed throughput differences were not evidence that a lower `64 KiB`
  threshold improved real parallel reconstruct entry

## 6. Runtime Refactor Result

The serial-fallback refactor removed one redundant `plan_option_vec_reconstruct`
pass from `reconstruct_opt(...)` and `reconstruct_data_opt(...)`.

What it changed:

1. the code path is less patch-stacked
2. serial fallback now reuses the already computed plan instead of re-entering
   generic reconstruct planning

What it did not change enough:

1. `reconstruct_opt_*` remains far slower than direct `reconstruct_serial`
2. the refactor only moved the needle by a small amount inside the slower group

So the main remaining cost is not just duplicate planning. It is broader
serial-path overhead in the `Option<Vec<u8>>` opt/fallback flow.

## 7. Final Decision

Do **not** promote the `64 KiB` threshold to the default policy.

Do **not** use the earlier one-variant-at-a-time results as policy evidence.

The current evidence says:

1. direct serial reconstruct is still the fastest path for these `64 KiB`
   option-vec reconstruct benchmarks
2. the current `64 KiB` candidate does not actually trigger parallel entry
3. the remaining problem is structural serial-path overhead, not missing proof
   for a more aggressive threshold

## 8. Recommended Next Step

The next narrow hotspot should target the serial `Option<Vec<u8>>` reconstruct
path itself, not another threshold tweak.

Most likely directions:

1. compare direct `reconstruct_internal` style execution against the
   `Option<Vec<u8>>` fallback path to isolate allocation and indirection costs
2. look for reusable buffers or reduced copying in serial option-vec reconstruct
3. keep the round-robin harness and explicit path markers as the default A/B
   method for future reconstruct-policy work
