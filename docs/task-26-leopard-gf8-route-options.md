# Task 26: Leopard GF8 Route Options

## 1. Goal

Reset the Leopard GF8 optimization strategy after the retained Task 17 baseline and the rejected first cuts from
Tasks 18, 19, 20, 22, 23, 24, and 25.

This task is intentionally a route-selection task, not an implementation task.

## 2. Why This Task Exists

The current Leopard GF8 retained baseline is already strong enough that continuing to patch nearby encode-path
variables without a larger routing decision has become low-confidence work.

Retained baseline:

- `128x64_1m`: `11.4957 MB/s`
- `64x32_1m`: `32.2955 MB/s`
- `128x64_4m`: `8.5797 MB/s`
- `64x32_4m`: `20.8331 MB/s`
- `128x64_1m` profile artifact: `11.4999 MB/s`

Retained profile evidence includes:

- `encode_chunks = 16`
- `first_group_ifft_calls = 16`
- `later_group_ifft_calls = 16`
- `remainder_group_ifft_calls = 0`
- `first_group_input_copy_bytes = 134217728`
- `later_group_input_copy_bytes = 134217728`
- `later_group_xor_bytes = 134217728`
- `output_writeback_bytes = 134217728`

At the same time, the first cuts of the following follow-up tasks were all rejected:

- Task 18: group traversal partitioning
- Task 19: group schedule metadata tightening
- Task 20: later-group bookkeeping tightening
- Task 22: later-group accumulation first cut
- Task 23: chunk-size jump
- Task 24: threshold-only change
- Task 25: work-slices-only change

That is enough evidence that the next step should not be another immediate LeopardGF8 encode patch in the same local
area.

## 3. What Is Settled

### 3.1 Retained

- execution-time `build_ifft_dit8_plan(...)` / `build_fft_dit8_plan(...)` reuse
- retained fused scalar `4x` butterfly kernel
- direct zero-fill for missing lanes
- retained batched `slice_xor(...)` path
- richer Leopard profile exporter for `128x64_1m`

### 3.2 Rejected

- helper-level butterfly and SIMD follow-ups around the retained kernel
- parity-output aliasing
- direct stage-1 materialization rewrite
- full `available == 4` materialization copy-shape rewrite
- batched parity writeback helper
- `u64` word-wise `slice_xor(...)`
- manual `with_lane_views(...)` rewrite
- traversal branch specialization
- schedule metadata container replacement
- later-group bookkeeping-only rewrite
- later-group accumulation bookkeeping rewrite
- chunk-size jump
- threshold-only change
- work-slices-only change

## 4. Current Interpretation

The accumulated evidence now supports three conclusions:

1. the easy local wins around the retained LeopardGF8 encode path were already captured in Task 17
2. nearby helper, traversal, metadata, and narrow design-variable first cuts are now more likely to regress than to
   improve
3. the next credible improvement must come from a genuinely different layer or a broader decision process

## 5. Candidate Next Routes

### Route A: Broader Measurement Before More Code

Do not patch LeopardGF8 encode yet. Expand measurement coverage first.

Examples:

- add one or two more nearby fanout topologies, such as `96x48_*`
- compare `1m` and `4m` behavior across more than the current retained pairs
- add retained-vs-classic or retained-vs-setup ratios to the benchmark note

Why this may be best:

- lowest immediate risk
- helps prevent further rejected first cuts
- may expose a new boundary where current assumptions stop holding

### Route B: Higher-Level Design Outside The Current Encode Loop

Leave the current encode hot path alone and move up one layer.

Examples:

- revisit codec-family routing and when LeopardGF8 should be selected at all
- revisit setup/runtime amortization boundaries rather than encode inner-loop details
- examine whether a different work ownership model should be considered in a fresh task, rather than as another local
  patch on the retained path

Why this may be best:

- clearly different layer than the rejected encode-loop follow-ups
- less likely to repeat the same failure pattern

### Route C: Broader Benchmark Decision Task

Treat the next step as a benchmark decision study rather than an optimization task.

Examples:

- determine whether `128x64_*` is the right primary optimization target
- compare retained LeopardGF8 behavior against a small matrix of adjacent shapes and shard sizes
- decide whether the repo should continue investing in LeopardGF8 encode optimization or hold at the current retained
  baseline

Why this may be best:

- forces an explicit continuation-or-stop decision
- avoids unlimited local tuning without a larger product/repo rationale

## 6. Recommended Next Step

The recommended next step is Route A first:

- expand the retained benchmark decision surface before another implementation task

Reason:

- seven successive first cuts near the current encode layer have been rejected
- the retained baseline is strong enough that the next code change should be justified by broader measurement evidence

If Route A reveals a clear higher-leverage boundary, continue into Route B.

## 7. Execution Plan

### Step 1

Preserve the current retained artifacts:

- `target/benchmark-smoke/leopard-encode-profile-128x64_1m.csv`
- `target/benchmark-smoke/leopard-encode-128x64_1m.csv`
- `target/benchmark-smoke/leopard-encode-64x32_1m.csv`
- `target/benchmark-smoke/leopard-encode-128x64_4m.csv`
- `target/benchmark-smoke/leopard-encode-64x32_4m.csv`

### Step 2

Choose exactly one route:

1. broader measurement
2. higher-level design outside the current encode loop
3. broader benchmark decision task

### Step 3

Only after that route is validated should new LeopardGF8 implementation work resume.

## 8. Stable Benchmark Entry

The local `rustup` wrapper may be unreliable in this checkout.

Use:

```bash
RUSTC="$(rustup which rustc)" "$(rustup which cargo)" test <...>
```

as the stable local benchmark entrypoint.

## 9. Route Validation

### 9.1 Route A Validation

Route A was validated and proved useful.

What changed:

- the retained Leopard profile was extended from coarse counters to phase-level buckets
- the profile now distinguishes:
  - first-group / later-group / remainder IFFT calls
  - first-group / later-group / remainder input-copy bytes
  - later-group / remainder xor bytes
  - output writeback calls/bytes

What this revealed on the retained path:

- `remainder_group_ifft_calls = 0`
- `first_group_input_copy_bytes = 134217728`
- `later_group_input_copy_bytes = 134217728`
- `later_group_xor_bytes = 134217728`
- `output_writeback_bytes = 134217728`

Interpretation:

- Route A successfully narrowed the next plausible hotspot surface
- it proved that remainder handling is not the next cut
- it also showed that later-group accumulation was the only stage simultaneously owning a large input-copy bucket and
  the full xor bucket

Follow-up result:

- the first implementation cut opened from that evidence (Task 22) was still rejected

Conclusion:

- Route A was worth doing and should be kept as the retained measurement basis
- but it did not immediately justify further implementation work in the same nearby encode layer

### 9.2 Route B Validation

Route B was evaluated conceptually and did not yet earn implementation priority.

Reason:

- the current benchmark evidence still points most strongly inside Leopard encode behavior itself
- there is not yet a stronger case for codec-family routing, setup/runtime amortization, or ownership-model work than
  for broadening the decision surface first

Conclusion:

- Route B remains plausible
- but it is not the next recommended move yet

### 9.3 Route C Validation

Route C is now the best next route.

Reason:

- helper-level and nearby encode-loop first cuts have repeatedly been rejected
- broader design-variable first cuts have also been rejected
- the retained baseline is strong enough that continuation should now be justified by a broader benchmark/de
  cision surface, not another immediate patch

Existing evidence already pointing in this direction:

- retained baseline:
  - `128x64_1m = 11.4957 MB/s`
  - `64x32_1m = 32.2955 MB/s`
  - `128x64_4m = 8.5797 MB/s`
  - `64x32_4m = 20.8331 MB/s`
- rejected first cuts:
  - Tasks 18, 19, 20, 22, 23, 24, 25

Conclusion:

- Route C should now become the active route
- the repo should next expand the Leopard decision surface rather than continue immediate encode-path patching

## 10. Current Recommendation

After validating A/B/C in sequence:

1. keep Route A's richer profile instrumentation
2. defer Route B until stronger evidence exists
3. proceed with Route C next

The next concrete step should be a broader benchmark decision task, not another local LeopardGF8 encode optimization
task.
