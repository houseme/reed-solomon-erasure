# EC Reconstruct Size-aware Policy A/B (2026-06-16)

## 1. Goal

This pass tested a narrower follow-up to the failed fanout-aware candidate.

The size-aware hypothesis was:

1. keep `4x2_64k` conservative
2. only let the `64 KiB` band with multi-output full reconstruct enter parallel earlier
3. avoid the broader fanout rule that had already regressed `16x8_64k`

## 2. Candidate

The candidate was enabled only through:

- `RS_RECONSTRUCT_SIZE_AWARE_EXPERIMENT=1`

Compared variants:

1. `reconstruct_opt_default`
2. `reconstruct_opt_minparallel64k_minjob64k`
3. `reconstruct_opt_size_aware`

## 3. Final Serial Rerun Results

### `4x2_64k`

| variant | throughput MB/s |
| --- | ---: |
| `reconstruct_opt_default` | `4013.2920` |
| `reconstruct_opt_minparallel64k_minjob64k` | `5059.1414` |
| `reconstruct_opt_size_aware` | `5058.1178` |

### `10x4_64k`

| variant | throughput MB/s |
| --- | ---: |
| `reconstruct_opt_default` | `8525.3042` |
| `reconstruct_opt_minparallel64k_minjob64k` | `9391.9995` |
| `reconstruct_opt_size_aware` | `9336.7195` |

### `16x8_64k`

| variant | throughput MB/s |
| --- | ---: |
| `reconstruct_opt_default` | `10854.5026` |
| `reconstruct_opt_minparallel64k_minjob64k` | `12134.2534` |
| `reconstruct_opt_size_aware` | `6422.6351` |

### `32x16_64k`

| variant | throughput MB/s |
| --- | ---: |
| `reconstruct_opt_default` | `13829.3937` |
| `reconstruct_opt_minparallel64k_minjob64k` | `14834.7960` |
| `reconstruct_opt_size_aware` | `9541.9923` |

## 4. Judgment

The size-aware candidate should **not** be kept.

Reason:

1. on `4x2_64k`, it is only flat with the simpler global `64 KiB` threshold
2. on `10x4_64k`, it is slightly worse than the simpler global `64 KiB` threshold
3. on `16x8_64k`, it regresses badly
4. on `32x16_64k`, it also regresses badly

That makes it clearly worse than the already-tested `reconstruct_opt_minparallel64k_minjob64k`
experiment.

## 5. Final Decision

1. revert the size-aware experiment from runtime policy code
2. keep the reconstruct policy benchmark harness in `tests/benchmark_smoke.rs`
3. treat both the fanout-aware and size-aware candidates as failed hypotheses

## 6. What This Means

Choosing different handling by size can absolutely matter, but these results show
that the rule shape has to be extremely well-targeted.

A naive or semi-naive size-aware rule can easily improve one case while causing
large regressions elsewhere.
