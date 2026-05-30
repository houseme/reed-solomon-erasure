# EC Update Benchmark Results 2026-05-28

## Goal

This note records the first benchmark-backed comparison between classic full `encode` and the new classic-path
`update` API for sparse data-shard changes.

The goal is not to claim universal performance yet. The goal is to answer the first practical question:

- does `update` materially beat full `encode` when only a small number of data shards changed?

## Scope

Current measured cases:

- `4x2_64k`
- `4x2_4m`
- `10x4_1m`
- `10x4_4m`
- `32x16_1m`
- `32x16_4m`
- each with:
  - one changed data shard
  - two changed data shards
  - three changed data shards
  - four changed data shards

Artifacts written by the smoke comparison tests:

- `target/benchmark-smoke/update-vs-encode-4x2_64k.json`
- `target/benchmark-smoke/update-vs-encode-4x2_64k.csv`
- `target/benchmark-smoke/update-vs-encode-4x2_4m.json`
- `target/benchmark-smoke/update-vs-encode-4x2_4m.csv`
- `target/benchmark-smoke/update-vs-encode-10x4_1m.json`
- `target/benchmark-smoke/update-vs-encode-10x4_1m.csv`
- `target/benchmark-smoke/update-vs-encode-10x4_4m.json`
- `target/benchmark-smoke/update-vs-encode-10x4_4m.csv`
- `target/benchmark-smoke/update-vs-encode-32x16_1m.json`
- `target/benchmark-smoke/update-vs-encode-32x16_1m.csv`
- `target/benchmark-smoke/update-vs-encode-32x16_4m.json`
- `target/benchmark-smoke/update-vs-encode-32x16_4m.csv`

## How Results Were Produced

Commands:

```bash
cargo test benchmark_update_vs_encode_4x2_64k_exports_results -- --nocapture
cargo test benchmark_update_vs_encode_4x2_4m_exports_results -- --nocapture
cargo test benchmark_update_vs_encode_10x4_1m_exports_results -- --nocapture
cargo test benchmark_update_vs_encode_10x4_4m_exports_results -- --nocapture
cargo test benchmark_update_vs_encode_32x16_1m_exports_results -- --nocapture
cargo test benchmark_update_vs_encode_32x16_4m_exports_results -- --nocapture
```

Each test:

1. constructs a fixed benchmark case
2. times full `encode`
3. times `update` with one changed shard
4. times `update` with two changed shards
5. times `update` with three changed shards
6. times `update` with four changed shards
7. exports `speedup_vs_encode`

## Results

### `4x2_64k`

From `target/benchmark-smoke/update-vs-encode-4x2_64k.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs Encode |
|---|---:|---:|---:|
| `update_4x2_64k_1_change` | `143.1571` | `1746333.50` | `5.7302x` |
| `update_4x2_64k_2_changes` | `93.0759` | `2685979.50` | `2.7943x` |
| `update_4x2_64k_3_changes` | `70.5401` | `3544083.00` | `1.8261x` |
| `update_4x2_64k_4_changes` | `53.5392` | `4669479.00` | `1.3858x` |

### `10x4_1m`

From `target/benchmark-smoke/update-vs-encode-10x4_1m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs Encode |
|---|---:|---:|---:|
| `update_10x4_1m_1_change` | `349.6745` | `28598021.00` | `12.0604x` |
| `update_10x4_1m_2_changes` | `174.9114` | `57171792.00` | `6.0504x` |
| `update_10x4_1m_3_changes` | `116.8680` | `85566625.00` | `4.0506x` |
| `update_10x4_1m_4_changes` | `87.7212` | `113997583.50` | `3.0421x` |

### `4x2_4m`

From `target/benchmark-smoke/update-vs-encode-4x2_4m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs Encode |
|---|---:|---:|---:|
| `update_4x2_4m_1_change` | `212.8873` | `75157146.00` | `5.3257x` |
| `update_4x2_4m_2_changes` | `107.3175` | `149090270.50` | `2.6698x` |
| `update_4x2_4m_3_changes` | `70.9786` | `225420125.00` | `1.7669x` |
| `update_4x2_4m_4_changes` | `53.7258` | `297808396.00` | `1.3355x` |

### `32x16_1m`

From `target/benchmark-smoke/update-vs-encode-32x16_1m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs Encode |
|---|---:|---:|---:|
| `update_32x16_1m_1_change` | `372.6549` | `85870333.50` | `34.5720x` |
| `update_32x16_1m_2_changes` | `180.9976` | `176797937.50` | `17.2871x` |
| `update_32x16_1m_3_changes` | `112.7975` | `283694208.50` | `10.7307x` |
| `update_32x16_1m_4_changes` | `88.0984` | `363230250.00` | `8.6837x` |

### `10x4_4m`

From `target/benchmark-smoke/update-vs-encode-10x4_4m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs Encode |
|---|---:|---:|---:|
| `update_10x4_4m_1_change` | `342.6335` | `116742833.00` | `12.0733x` |
| `update_10x4_4m_2_changes` | `175.2334` | `228267020.50` | `6.1187x` |
| `update_10x4_4m_3_changes` | `116.2330` | `344136375.00` | `4.0463x` |
| `update_10x4_4m_4_changes` | `87.4341` | `457487541.50` | `3.0245x` |

### `32x16_4m`

From `target/benchmark-smoke/update-vs-encode-32x16_4m.csv`:

| Case | Throughput MB/s | ns/iter | Speedup vs Encode |
|---|---:|---:|---:|
| `update_32x16_4m_1_change` | `362.1216` | `353472416.50` | `34.0494x` |
| `update_32x16_4m_2_changes` | `181.2237` | `706309208.00` | `16.9713x` |
| `update_32x16_4m_3_changes` | `120.9579` | `1058219812.50` | `11.3275x` |
| `update_32x16_4m_4_changes` | `90.5157` | `1414119812.50` | `8.5215x` |

## Interpretation

### Main conclusion

Across the measured cases, `update` is clearly worthwhile when the number of changed shards is small.

### What the numbers mean

- `4x2_64k`
  - `1` changed shard: about `5.7x`
  - `2` changed shards: about `2.8x`
  - `3` changed shards: about `1.8x`
  - `4` changed shards: about `1.4x`
- `4x2_4m`
  - `1` changed shard: about `5.3x`
  - `2` changed shards: about `2.7x`
  - `3` changed shards: about `1.8x`
  - `4` changed shards: about `1.3x`
- `10x4_1m`
  - `1` changed shard: about `12.1x`
  - `2` changed shards: about `6.1x`
  - `3` changed shards: about `4.1x`
  - `4` changed shards: about `3.0x`
- `10x4_4m`
  - `1` changed shard: about `12.1x`
  - `2` changed shards: about `6.1x`
  - `3` changed shards: about `4.0x`
  - `4` changed shards: about `3.0x`
- `32x16_1m`
  - `1` changed shard: about `34.6x`
  - `2` changed shards: about `17.3x`
  - `3` changed shards: about `10.7x`
  - `4` changed shards: about `8.7x`
- `32x16_4m`
  - `1` changed shard: about `34.0x`
  - `2` changed shards: about `17.0x`
  - `3` changed shards: about `11.3x`
  - `4` changed shards: about `8.5x`

### Observed scaling behavior

The results are consistent with a near-linear sparse-update cost model:

- changing twice as many data shards roughly halves the speedup
- as the number of changed shards grows, `update` smoothly approaches full `encode` cost

Large-shard observation:

- moving from `1m` to `4m` does not materially change the relative shape of the curve for the same shard topology
- this suggests the speedup is driven much more by changed-shard count and data fanout than by shard size alone

That is exactly the behavior we want from an incremental parity-update path.

## Limits of This Result

These results still do **not** prove that `update` is equally valuable across all possible shapes.

We still need direct paired results for:

- medium and high data/parity fanout variants beyond the current set
- changed-shard counts beyond `4`
- potentially different backend overrides where SIMD/runtime dispatch may shift constants

## Recommendation

Based on the current benchmark evidence:

1. keep the `update` API
2. treat it as a high-value path for sparse writes
3. expand paired comparisons to changed-shard counts beyond `4` and more topology variants
4. only after that decide whether to add stronger performance claims to the public README

## Current Verdict

For the measured cases, the answer to "is `update` worth it?" is:

- yes, emphatically, when only a small number of data shards changed
- the relative benefit grows as total data fanout grows and changed-shard count stays small
- the current curve already shows the expected tradeoff: each additional changed shard reduces the advantage, but the
  API remains attractive well past one or two changes in larger fanout configurations
