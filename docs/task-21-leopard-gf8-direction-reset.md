# Task 21: Leopard GF8 Direction Reset

## 1. Goal

Choose the next optimization direction for Leopard GF8 after Task 17's retained wins and the rejected first cuts from
Tasks 18, 19, and 20.

This task is intentionally a decision/reset slice, not an implementation slice.

## 2. Why This Task Exists

The current retained Leopard GF8 baseline is strong enough that continuing to iterate blindly on nearby loop shape is
now lower-value than pausing to choose a new direction deliberately.

Retained baseline:

- `128x64_1m`: `11.4957 MB/s`
- `64x32_1m`: `32.2955 MB/s`
- `128x64_1m` profile: `11.4999 MB/s`

Retained traffic/profile split:

- `input_copy_bytes = 268435456`
- `xor_bytes = 134217728`
- `output_writeback_bytes = 134217728`
- `zero_fill_bytes = 0`
- `encode_chunks = 16`
- `encode_full_groups = 32`
- `encode_later_group_calls = 16`
- `fft_stage_calls = 16`
- `ifft_stage_calls = 32`

At the same time, three higher-level follow-up tasks have now failed their first cut:

- Task 18: traversal branch specialization
- Task 19: schedule metadata container replacement
- Task 20: later-group bookkeeping tightening

That is enough evidence that the next step should not be “try another nearby loop tweak”.

## 3. What Is Already Settled

### 3.1 Retained

- execution-time `build_ifft_dit8_plan(...)` / `build_fft_dit8_plan(...)` reuse
- retained fused scalar `4x` butterfly kernel
- direct zero-fill for missing lanes
- retained batched `slice_xor(...)` path
- Leopard profile exporter for `128x64_1m`

### 3.2 Rejected

- `16-byte` scalar butterfly chunking
- aarch64 NEON fused butterfly prototype
- parity-output aliasing
- direct stage-1 materialization rewrite
- full `available == 4` copy-shape rewrite
- batched parity writeback helper
- `u64` word-wise `slice_xor(...)`
- manual `with_lane_views(...)` build path
- common-case traversal branch specialization
- unified later-group schedule metadata structure
- later-group bookkeeping-only rewrite

## 4. Current Interpretation

The evidence now points to this:

1. local helper/kernel work already captured the obvious wins
2. nearby traversal/schedule rewrites are easy to make slower
3. the next viable cut probably needs to move to a genuinely different layer

In other words: the next step should not be another encode-loop micro-refactor adjacent to the last three rejected
tasks.

## 5. Candidate Next Directions

### Direction A: Stronger Measurement Before More Code

Add richer profiling around the retained baseline before changing code again.

Examples:

- separate timing/counter export for first-group vs later-group vs fft tail work
- count per-stage bytes touched more explicitly
- add one more topology that distinguishes common-case assumptions from `128x64_1m`

Why this may be best:

- lowest risk
- likely to prevent more rejected cuts

### Direction B: Revisit Chunk/Work Sizing As A Design Slice

Do not micro-tune helper loops; instead re-open chunk/work partitioning as a more explicit design task.

Examples:

- whether `WORK_SIZE8_HIGH_FANOUT` is still the right retained size
- whether the common high-fanout path should use a different work-slice budget

Why this may be best:

- changes a larger lever than the rejected bookkeeping cuts
- still local to LeopardGF8 encode

### Direction C: Multi-Topology Decision Task

Broaden the benchmark matrix around the retained baseline before changing implementation.

Examples:

- pair `128x64_1m` with `128x64_4m`
- pair `96x48_*` or other nearby fanout shapes if available
- check whether the current retained baseline is truly topology-stable

Why this may be best:

- helps avoid overfitting to one topology
- may reveal a new hotspot boundary

## 6. Recommended Next Step

The recommended next step is Direction A first:

- add stronger measurement before another code change

Reason:

- three successive higher-level first cuts were rejected
- the next code change should be profile-backed at a finer grain than the current artifact provides

If Direction A reveals a clean larger lever, then continue into Direction B.

## 7. Execution Plan

### Step 1

Preserve the retained baseline artifacts:

- `target/benchmark-smoke/leopard-encode-profile-128x64_1m.csv`
- `target/benchmark-smoke/leopard-encode-128x64_1m.csv`
- `target/benchmark-smoke/leopard-encode-64x32_1m.csv`

### Step 2

Choose exactly one of:

1. richer profile instrumentation
2. chunk/work sizing design slice
3. multi-topology decision benchmark expansion

### Step 3

Only after that choice is validated should new implementation work resume.

## 8. Stable Benchmark Entry

The local `rustup` wrapper may be unreliable in this checkout.

Use:

```bash
RUSTC="$(rustup which rustc)" "$(rustup which cargo)" test <...>
```

as the stable local benchmark entrypoint.
