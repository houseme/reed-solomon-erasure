# EC Reconstruct Round-robin Review (2026-06-16)

## Summary

The reconstruct policy benchmark harness was upgraded so the next tuning round
 can reason from path evidence instead of throughput guesses.

Main outcomes:

1. benchmark order bias was real
2. the earlier `64 KiB` policy interpretation was not reliable
3. current `64 KiB` cases still do not enter parallel reconstruct
4. removing duplicate planning from serial fallback is worthwhile cleanup, but
   not the main performance unlock

## Harness Changes

The reconstruct policy benchmark artifacts now record:

1. `measurement_strategy`
2. `measurement_order`
3. `measurement_iterations`
4. `warmup_rounds`
5. `entry_path`
6. reconstruct entry/runtime counters
7. reconstruct data/parity stage policy context

The measurement strategy is now:

- `round_robin_rotating_start`

This avoids the old one-variant-at-a-time bias where later variants could look
artificially better.

## What The New Artifacts Proved

Across `4x2_64k`, `10x4_64k`, `16x8_64k`, and `32x16_64k`:

1. `decision_use_parallel` stayed `false`
2. `runtime_parallel_policy_calls` stayed `0`
3. `runtime_code_some_parallel_calls` stayed `0`
4. `entry_path` stayed on `reconstruct_opt_fallback_serial` or
   `reconstruct_data_opt_fallback_serial`

So the current `64 KiB` candidate is not a real entry-level parallel policy
win. It is still a serial fallback path.

## Refactor Review

The branch had started to accumulate patch-on-patch reconstruct logic around:

1. entry policy decisions
2. benchmark-only inference
3. serial fallback behavior

This pass cleaned up one concrete patch stack:

1. `reconstruct_opt(...)` and `reconstruct_data_opt(...)` no longer compute a
   plan and then immediately re-enter full generic reconstruct planning on
   serial fallback
2. serial fallback now reuses the already computed option-vec reconstruct plan

That is a cleaner executable shape, but benchmark evidence says it only gives a
small improvement inside the still-slower fallback group.

## Practical Conclusion

The next high-value hotspot is not another threshold rule. It is the serial
`Option<Vec<u8>>` reconstruct path itself.

That is the path to compare and optimize next:

1. allocation behavior
2. extra copying
3. option-vec specific scanning/indirection costs
