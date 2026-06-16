# EC DecodeIdx Benchmark Results 2026-05-28

## Goal

This note records the first benchmark-backed comparison between progressive `decode_idx(...)` and one-shot
`reconstruct_some(...)` on the current worktree.

The goal is not to prove `decode_idx` is universally faster. The goal is to establish a realistic first baseline for
the new API so future optimization work has a concrete point of comparison.

## Scope

Current measured cases:

- `4x2_64k`
- `4x2_4m`
- `10x4_1m`
- `32x16_1m`
- `32x16_4m`
- reconstructing `2` required data shards
- `decode_idx(...)` driven by two progressive input batches

Artifacts:

- `target/benchmark-smoke/decode-idx-vs-reconstruct-some-4x2_64k.csv`
- `target/benchmark-smoke/decode-idx-vs-reconstruct-some-4x2_4m.csv`
- `target/benchmark-smoke/decode-idx-vs-reconstruct-some-10x4_1m.csv`
- `target/benchmark-smoke/decode-idx-vs-reconstruct-some-32x16_1m.csv`
- `target/benchmark-smoke/decode-idx-vs-reconstruct-some-32x16_4m.csv`

Artifact retention note:

- these benchmark-smoke artifacts now accumulate run history by appending new
  records instead of overwriting the previous contents

## How Results Were Produced

Command:

```bash
cargo test benchmark_decode_idx_vs_reconstruct_some_4x2_64k_exports_results -- --nocapture
cargo test benchmark_decode_idx_vs_reconstruct_some_4x2_4m_exports_results -- --nocapture
cargo test benchmark_decode_idx_vs_reconstruct_some_10x4_1m_exports_results -- --nocapture
cargo test benchmark_decode_idx_vs_reconstruct_some_32x16_1m_exports_results -- --nocapture
cargo test benchmark_decode_idx_vs_reconstruct_some_32x16_4m_exports_results -- --nocapture
```

The benchmark:

1. constructs a `10 data + 4 parity` case with 1 MiB shards
2. measures one-shot `reconstruct_some(...)` for the same required data targets
3. measures `decode_idx(...)` with two progressive input batches
4. exports `speedup_vs_reconstruct_some`

## Results

### `10x4_1m`

From `target/benchmark-smoke/decode-idx-vs-reconstruct-some-10x4_1m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs reconstruct_some |
|---|---:|---:|---:|
| `decode_idx_10x4_1m_two_step` | `16.9140` | `118244937.50` | `0.9091x` |

### `4x2_64k`

From `target/benchmark-smoke/decode-idx-vs-reconstruct-some-4x2_64k.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs reconstruct_some |
|---|---:|---:|---:|
| `decode_idx_4x2_64k_two_step` | `22.9769` | `5440250.00` | `1.0336x` |

### `4x2_4m`

From `target/benchmark-smoke/decode-idx-vs-reconstruct-some-4x2_4m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs reconstruct_some |
|---|---:|---:|---:|
| `decode_idx_4x2_4m_two_step` | `30.6650` | `260883375.00` | `0.8614x` |

### `32x16_1m`

From `target/benchmark-smoke/decode-idx-vs-reconstruct-some-32x16_1m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs reconstruct_some |
|---|---:|---:|---:|
| `decode_idx_32x16_1m_two_step` | `6.0942` | `328180729.50` | `1.0142x` |

### `32x16_4m`

From `target/benchmark-smoke/decode-idx-vs-reconstruct-some-32x16_4m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs reconstruct_some |
|---|---:|---:|---:|
| `decode_idx_32x16_4m_two_step` | `5.7867` | `1382471354.00` | `1.0000x` |

## Interpretation

### Main conclusion

The first-pass `decode_idx(...)` implementation is functionally correct, and the current performance picture depends
strongly on fanout:

- on `4x2_64k`, it is still slower than one-shot `reconstruct_some(...)`, but already in the same ballpark
- on `4x2_4m`, it is still slower than one-shot `reconstruct_some(...)`
- on `10x4_1m`, it is still slower than one-shot `reconstruct_some(...)`, but the gap has narrowed
- on `32x16_1m`, it is now slightly ahead of one-shot `reconstruct_some(...)`
- on `32x16_4m`, it is effectively at parity with one-shot `reconstruct_some(...)`

### What the number means

- `4x2_64k`
  - `speedup_vs_reconstruct_some = 1.0336x`
  - current `decode_idx(...)` throughput is slightly above the one-shot baseline in the latest run
- `4x2_4m`
  - `speedup_vs_reconstruct_some = 0.8614x`
  - current `decode_idx(...)` throughput is about `86%` of the one-shot baseline
- `10x4_1m`
  - `speedup_vs_reconstruct_some = 0.9091x`
  - current `decode_idx(...)` throughput is about `90%` of the one-shot baseline
- `32x16_1m`
  - `speedup_vs_reconstruct_some = 1.0142x`
  - current `decode_idx(...)` throughput is now slightly above the one-shot baseline
- `32x16_4m`
  - `speedup_vs_reconstruct_some = 1.0000x`
  - current `decode_idx(...)` throughput is effectively at parity with the one-shot baseline

### Why this is still a useful result

This is an expected and acceptable first benchmark result because:

1. `decode_idx(...)` targets incremental / distributed / partial-arrival workflows, not just raw one-shot speed
2. it currently uses the safe generic chunked path after matrix-column reduction, not the most specialized hot path
3. it now gives us a concrete optimization baseline instead of hand-wavy expectations

Additional observation after the reduced-column small-output optimization:

- the smaller `4x2_64k` case is now effectively at parity in the latest run
- the relative penalty still shrinks substantially as fanout grows from `10x4` to `32x16`
- removing one layer of reduced-row execution overhead was enough to move the larger-fanout case from near-parity to
  a slight win
- this suggests the fixed overheads in the progressive path become less dominant on larger reconstruction sets, and
  that smaller-fanout cases still have more room for optimization
- the `4x2_4m` result suggests larger shard size alone does not erase the small-fanout penalty
- the `32x16_4m` result reinforces that high fanout remains the stronger predictor of decode-idx viability than shard
  size alone
- a more aggressive follow-up micro-optimization for small-fanout data-only outputs was tested and reverted because it
  did not produce a stable win on the measured shapes
- a repeated-call full-row planning-cache attempt was also tested and reverted because it did not produce a stable
  benefit on the small and moderate fanout cases

Runtime follow-up on 2026-06-16:

- the reduced-row progressive path now writes directly into caller-owned `dst`
  buffers and treats each call as an XOR-accumulate contribution
- the row setup now builds only the coefficients needed by the present input
  batch, using stack-backed `SmallVec` rows instead of full-width `Vec<u8>`
  rows plus copy-back
- on `4x2_64k`, the appended history moved from `1257.8806 MB/s`
  (`1.3558x` vs `reconstruct_some`) to `1515.6384 MB/s` (`1.5721x`)
- after reverting the evidence-weak lazy parity-row lookup candidate, the
  same `4x2_64k` artifact still lands in the stable post-optimization band at
  `1450.8987 MB/s` (`1.5140x`)
- broader spot checks after the direct-accumulate change kept the larger
  fanout cases well ahead of `reconstruct_some`: `10x4_1m` reached
  `1533.2379 MB/s` (`4.1403x`) and `32x16_1m` reached `478.8273 MB/s`
  (`4.7330x`)
- the lazy parity-row lookup was not retained: it was plausible for data-only
  output batches, but the reruns did not show a stable improvement over the
  already-simpler eager parity-row setup
- a follow-up fused `dst` scan candidate, combining output discovery with
  pointer capture, was also not retained: the `4x2_64k` rerun dropped to
  `1425.4517 MB/s` (`1.5978x`), below the current `1447` to `1454 MB/s`
  stable band
- this is a runtime-path win, not a policy-threshold change; it keeps
  progressive correctness by accumulating into existing partial outputs

## Recommended Next Optimization Direction

If `decode_idx(...)` needs better speed on smaller fanout cases, the next most promising areas are:

1. reduce row-adjustment allocation cost for repeated progressive calls even further on smaller-fanout cases
2. extend paired comparisons to more topologies to map where the relative penalty meaningfully disappears
3. consider whether a cached reduced-row planning structure is worthwhile for repeated `expect_input` patterns

## Current Verdict

For the measured cases:

- `decode_idx(...)` is already useful as a capability
- on smaller and moderate fanout it is already operationally viable, but `10x4_1m` still leaves visible optimization headroom
- on higher fanout (`32x16_1m`, `32x16_4m`) it is already at or above one-shot performance in the measured shapes
