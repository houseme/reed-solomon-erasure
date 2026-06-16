# EC ReconstructSome Container Cost 10x4 64KiB (2026-06-16)

## Goal

Check whether the strong `ShardSlot` win seen on full `reconstruct(...)` also
applies to the `reconstruct_some(required_data_only)` path.

Case:

1. `10x4_64k`
2. missing pattern `d0|d2`
3. required mask requests only the two missing data shards

## Compared Variants

1. `option_vec_missing_none`
   Uses `Vec<Option<Vec<u8>>>`
2. `shard_slot_preallocated_missing`
   Uses `Vec<ShardSlot<Vec<u8>>>`

## Results

| variant | throughput MB/s | ns/iter | speedup vs option vec |
| --- | ---: | ---: | ---: |
| `option_vec_missing_none` | `429.4800` | `291049.67` | `1.0000x` |
| `shard_slot_preallocated_missing` | `438.4027` | `285126.00` | `1.0208x` |

## Interpretation

This result changed materially after two focused refactors in
[src/core/reconstruct.rs](/data/rustfs/reed-solomon-erasure/src/core/reconstruct.rs):

1. borrow present input shards directly instead of cloning them into
   `sub_shards_snapshot`
2. compute required outputs directly into caller-owned shard buffers instead of
   using a separate `recovered_data` buffer plus copy-back

For `reconstruct_some(required_data_only)`:

1. `Option<Vec<u8>>` throughput is now a bit more than `2x` the original
   snapshot-heavy baseline
2. removing output copy-back lifted the path again, from about `405 MB/s` to
   about `429 MB/s`
3. `ShardSlot` is now only marginally ahead, about `1.02x`
4. the large `2.5x+` container win from full reconstruct still does **not** carry over

That means the copy-elision work removed the actual dominant cost for this path.

## What This Proves

The old bottleneck was not missing-output container shape. It was the full-copy
snapshot of valid input shards in the required-data-only branch.

After borrowing present inputs directly and then removing output copy-back:

1. `reconstruct_some(required_data_only)` improves substantially on the default
   `Option<Vec<u8>>` path
2. reusable missing-output containers no longer provide a large advantage
3. the next hotspot, if any, is now likely in the matrix/code path itself or in
   remaining output initialization cost, not in whole-input cloning

## Conclusion

`ShardSlot` is a strong recommendation for full `reconstruct(...)`, but it is
not the main lever for `reconstruct_some(required_data_only)`.

The required-data-only copy/snapshot hotspot has now been addressed well enough
that it should stop being the next priority target.
