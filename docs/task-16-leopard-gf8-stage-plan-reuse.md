# Task 16: Leopard GF8 Stage-Plan Reuse

## 1. Goal

Reduce the `128x64_1m` scaling cliff in the current Leopard GF8 encode path by turning the existing
`build_ifft_dit8_plan(...)` / `build_fft_dit8_plan(...)` helpers into real execution-time plan reuse.

This task starts from the conclusion that the current bottleneck is no longer local buffer micro-tuning, but repeated
stage scheduling work at higher fanout.

## 2. Why This Task Exists

The current Leopard GF8 encode path already has:

- a functioning pure-Rust encode kernel
- a `FlatWork` owner type
- lane-view-friendly helper signatures
- baseline wins on several `32x16` / `64x32` shapes

However, `128x64_1m` still shows a severe scaling cliff.

The current profile artifact:

- [target/benchmark-smoke/leopard-encode-profile-128x64_1m.csv](/Users/zhi/Documents/code/rust/rustfs/reed-solomon-erasure/target/benchmark-smoke/leopard-encode-profile-128x64_1m.csv)

shows:

- `encode_calls = 2`
- `encode_chunks = 64`
- `encode_later_group_calls = 64`
- `fft_stage_calls = 64`
- `ifft_stage_calls = 128`

This means the problem is dominated by repeated stage work rather than remainder handling or small local buffer knobs.

## 3. Current Baselines

Accepted high-confidence reference points:

- `64x32_1m`: `16.1089 MB/s`
- `64x32_4m`: `15.8374 MB/s`

Current problematic point:

- `128x64_1m`: `6.8649 MB/s`

## 4. Scope

### 4.1 In scope

- make `build_ifft_dit8_plan(...)` a real reused execution plan
- make `build_fft_dit8_plan(...)` a real reused execution plan
- validate impact primarily on `128x64_1m`
- use `64x32_1m` as a control case to avoid breaking the stronger path

### 4.2 Out of scope

- further `zero` / `xor_clone` style micro-knobs
- verify/reconstruct Leopard migration
- chunk-size experimentation unless forced by evidence

## 5. Current Code Anchors

- [src/core/leopard_gf8.rs](/Users/zhi/Documents/code/rust/rustfs/reed-solomon-erasure/src/core/leopard_gf8.rs:1)
  - `build_fft_dit8_plan(...)`
  - `build_ifft_dit8_plan(...)`
  - `fft_dit8(...)`
  - `ifft_dit_encoder8(...)`
  - `encode_with_tables(...)`

- [tests/benchmark_smoke.rs](/Users/zhi/Documents/code/rust/rustfs/reed-solomon-erasure/tests/benchmark_smoke.rs:1)
  - `benchmark_leopard_encode_profile_128x64_1m_exports_results`
  - `benchmark_leopard_encode_128x64_1m_exports_results`
  - `benchmark_leopard_encode_64x32_1m_exports_results`

## 6. Core Hypothesis

The current implementation still recomputes or re-derives stage-driving structure too often across chunks and later
group passes.

If those stage layouts are precomputed once per encode call and reused directly, then:

- `128x64_1m` should improve noticeably
- `64x32_1m` should stay roughly flat

## 7. Execution Plan

### Step 1

Keep the current profile artifact as the diagnostic baseline:

- `encode_chunks = 64`
- `encode_later_group_calls = 64`
- `fft_stage_calls = 64`
- `ifft_stage_calls = 128`

### Step 2

Turn `build_ifft_dit8_plan(...)` into a real plan object used by `ifft_dit_encoder8(...)`.

### Step 3

Turn `build_fft_dit8_plan(...)` into a real plan object used by `fft_dit8(...)`.

### Step 4

Ensure plans are built once per encode call and reused across every chunk.

### Step 5

Re-run:

```bash
cargo test benchmark_leopard_encode_profile_128x64_1m_exports_results -- --nocapture
cargo test benchmark_leopard_encode_128x64_1m_exports_results -- --nocapture
cargo test benchmark_leopard_encode_64x32_1m_exports_results -- --nocapture
```

## 8. Acceptance Criteria

This task should be considered a success only if:

1. the stage-plan helpers are no longer dead planning code
2. `128x64_1m` improves meaningfully from the current `~6.9 MB/s` band
3. `64x32_1m` does not regress materially from the current `~16 MB/s` band
4. the updated profile artifact is written back to docs

## 9. Risks

### R1. Plan reuse changes nothing

Mitigation:

- profile again immediately
- if no change, the next hotspot is likely deeper in the actual butterfly math rather than schedule reuse

### R2. Control case regression

Mitigation:

- always pair `128x64_1m` with `64x32_1m`
- do not keep the change if the control case drops materially

## 10. Current Recommendation

This is the correct next optimization slice for Leopard GF8.

At this point, stage repetition has been isolated well enough that continuing local micro-tuning would be lower-value
than making plan reuse real.
