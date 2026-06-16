# EC Reconstruct Fanout-aware Policy A/B (2026-06-16)

## 1. Goal

This pass tested whether a fanout-aware reconstruct policy could do better than
the earlier “global `64 KiB` threshold” experiment.

The intended design was:

- keep small fanout conservative
- let medium fanout (`10x4` to `16x8`) enter parallel reconstruct earlier

## 2. Candidate

The candidate was enabled only through the experiment flag:

- `RS_RECONSTRUCT_FANOUT_AWARE_EXPERIMENT=1`

It was compared against:

1. `reconstruct_opt_default`
2. `reconstruct_opt_minparallel64k_minjob64k`

## 3. Correct Measurement Note

An earlier attempt ran several policy benchmarks in parallel, which polluted the
results. The final decision in this note is based only on **serially rerun**
case-by-case measurements.

## 4. Final Serial Rerun Results

### `4x2_64k`

| variant | throughput MB/s |
| --- | ---: |
| `reconstruct_opt_default` | `3999.6800` |
| `reconstruct_opt_minparallel64k_minjob64k` | `4745.9043` |
| `reconstruct_opt_fanout_aware` | `4734.6393` |

### `10x4_64k`

| variant | throughput MB/s |
| --- | ---: |
| `reconstruct_opt_default` | `8257.4019` |
| `reconstruct_opt_minparallel64k_minjob64k` | `9245.4254` |
| `reconstruct_opt_fanout_aware` | `9304.9338` |

### `16x8_64k`

| variant | throughput MB/s |
| --- | ---: |
| `reconstruct_opt_default` | `10824.1118` |
| `reconstruct_opt_minparallel64k_minjob64k` | `12033.0910` |
| `reconstruct_opt_fanout_aware` | `6438.2380` |

### `32x16_64k`

| variant | throughput MB/s |
| --- | ---: |
| `reconstruct_opt_default` | `13917.1097` |
| `reconstruct_opt_minparallel64k_minjob64k` | `15183.8960` |
| `reconstruct_opt_fanout_aware` | `14997.6566` |

## 5. Judgment

The fanout-aware candidate should **not** be kept.

Reason:

1. it is only marginally better than the global `64 KiB` threshold on `10x4_64k`
2. it is slightly worse than the global `64 KiB` threshold on `4x2_64k`
3. it is slightly worse than the global `64 KiB` threshold on `32x16_64k`
4. most importantly, it regresses badly on `16x8_64k`

That means this candidate is not a safe refinement. It is strictly weaker than
the simpler global-threshold experiment.

## 6. Final Decision

1. Do not promote this fanout-aware candidate.
2. Do not keep the candidate code path enabled by default.
3. If further policy work continues, start from a new hypothesis instead of
   iterating on this exact rule.

## 7. Recommended Next Step

The most promising direction is now:

1. revert this failed fanout-aware experiment code
2. keep the benchmark harness additions
3. if policy work continues, explore a case-family rule that is explicitly
   validated against `4x2_64k` and `16x8_64k` first, because those are the
   fastest failure detectors
