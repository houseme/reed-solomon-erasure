# EC Reconstruct Container Cost 10x4 64KiB (2026-06-16)

## Goal

After the round-robin policy review showed that `64 KiB` reconstruct cases were
still staying on serial fallback paths, this pass isolated one narrower
question:

- how much of the remaining serial cost comes from the shard container shape
  itself?

The benchmark keeps the reconstruct algorithm the same and only changes the
container used for missing shards.

## Compared Variants

Case:

1. `10x4_64k`
2. missing pattern `d0|p0`

Variants:

1. `option_vec_missing_none`
   Uses `Vec<Option<Vec<u8>>>` with missing shards set to `None`
2. `shard_slot_preallocated_missing`
   Uses `Vec<ShardSlot<Vec<u8>>>` with missing shards represented by a
   preallocated buffer plus `present=false`

## Results

| variant | throughput MB/s | ns/iter | speedup vs option vec |
| --- | ---: | ---: | ---: |
| `option_vec_missing_none` | `7167.6915` | `87196.83` | `1.0000x` |
| `shard_slot_preallocated_missing` | `17649.7997` | `35411.17` | `2.4624x` |

## Interpretation

This is a much larger gap than the earlier policy deltas.

The main takeaway is:

1. the serial reconstruct algorithm is not the dominant problem by itself
2. `Option<Vec<u8>>` plus missing-shard initialization is a major cost center
3. a preallocated reusable shard container can be about `2.5x` faster on
   this workload

That makes the next optimization target much clearer than before.

## Recommendation

Do not spend the next iteration on another reconstruct threshold rule.

The next high-value experiment should be one of:

1. add a benchmarked reusable reconstruct API shape that accepts preallocated
   output buffers
2. add a narrow optimized path for option-vec reconstruct that reuses caller
   storage instead of allocating on missing shards
3. compare `reconstruct_some` and `decode_idx` against the same preallocated
   container style to see whether the advantage generalizes

## Follow-Up: Option Vec Fallback Direct Write

A narrower `reconstruct_opt(...)` serial fallback candidate was tested after
the container comparison:

1. initialize missing `Option<Vec<u8>>` slots directly
2. compute recovered data/parity directly into those slots
3. avoid the temporary `recovered_data` / `recovered_parity` buffers plus
   copy-back inside the fallback implementation

Correctness checks passed:

- `cargo test --features "std simd-accel" reconstruct_opt -- --nocapture`
- `cargo test --features "std simd-accel" reconstruct_data_opt -- --nocapture`
- `cargo test --features "std simd-accel" reconstruct_some_opt -- --nocapture`

However, the relevant policy A/B did not support keeping the candidate:

| variant | previous best MB/s | candidate MB/s | verdict |
| --- | ---: | ---: | --- |
| `reconstruct_opt_default` | `9647.7417` | `9322.7692` | discard |
| `reconstruct_data_opt_two_data_missing` | `9716.8887` | `9719.8607` | noise-level |

The direct-write fallback was therefore reverted. The full reconstruct container
gap is not solved by simply removing the fallback copy-back; the stronger signal
still points toward reusable/preallocated output storage or a dedicated API
shape rather than another hidden `Option<Vec<u8>>` fallback rewrite.
