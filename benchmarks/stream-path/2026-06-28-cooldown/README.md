# Stream Path Cooldown Archive - 2026-06-28

This directory archives stream path benchmark evidence for backlog
`rustfs/backlog#751` Phase 5 / `#757`.

## Environment

- Host triple: `aarch64-macos-unknown`
- Features: `std`
- Backend: `scalar-rust`
- Stream backend: Memory
- Cooldown rule: every benchmark invocation used at least a 20 second cooldown
  before the next invocation.

## Files

| File | Commit | Profile | Purpose |
|---|---|---|---|
| `phase0-51e0b64-fast.csv` | `51e0b64` | `fast` | Phase 0 stream benchmark harness baseline |
| `phase4-baseline-f1ad373-fast-auto-selected-iter10.csv` | `f1ad373` | `fast`, `auto`, selected cases, 10 iterations | Phase 4 baseline for focused cooldown comparison |
| `phase4-current-2a1aa88-fast-auto-selected-iter10.csv` | `2a1aa88` | `fast`, `auto`, selected cases, 10 iterations | Optimized Phase 4 focused cooldown comparison |
| `phase4-baseline-f1ad373-fast-serial-4x2_64k-iter20.csv` | `f1ad373` | `fast`, `serial`, `4x2_64k`, 20 iterations | Suspicious serial small-block baseline recheck |
| `phase4-current-2a1aa88-fast-serial-4x2_64k-iter20.csv` | `2a1aa88` | `fast`, `serial`, `4x2_64k`, 20 iterations | Suspicious serial small-block optimized recheck |
| `phase4-baseline-f1ad373-extended-auto.csv` | `f1ad373` | `extended`, `auto` | Extended default-mode baseline screen |
| `phase4-current-2a1aa88-extended-auto.csv` | `2a1aa88` | `extended`, `auto` | Extended default-mode optimized screen |

## Focused Cooldown Results

`fast/auto`, selected cases, `RSE_STREAM_ITERATIONS=10`,
`reconstruct_stream`:

| Case | Baseline MB/s | Current MB/s | Throughput Delta | Baseline ns/iter | Current ns/iter | Latency Delta |
|---|---:|---:|---:|---:|---:|---:|
| `4x2_64k` | 958.25 | 962.79 | +0.5% | 260891.60 | 259662.50 | -0.5% |
| `4x2_1m` | 1461.51 | 1496.85 | +2.4% | 2736895.80 | 2672270.90 | -2.4% |
| `10x4_64k` | 992.79 | 1103.17 | +11.1% | 629541.70 | 566550.00 | -10.0% |
| `10x4_1m` | 1334.64 | 1378.03 | +3.3% | 7492633.30 | 7256716.60 | -3.1% |

`fast/serial`, `4x2_64k`, `RSE_STREAM_ITERATIONS=20`:

| Operation | Baseline MB/s | Current MB/s | Throughput Delta | Baseline ns/iter | Current ns/iter | Latency Delta |
|---|---:|---:|---:|---:|---:|---:|
| `encode_stream` | 1790.38 | 1829.13 | +2.2% | 139635.40 | 136677.05 | -2.1% |
| `verify_stream` | 1229.07 | 1286.38 | +4.7% | 203406.25 | 194343.75 | -4.5% |
| `reconstruct_stream` | 1113.03 | 1273.26 | +14.4% | 224612.50 | 196345.85 | -12.6% |

`extended/auto`, full screen, `reconstruct_stream`:

- average delta: `-0.9%`
- range: `-5.6%..+2.4%`

## Gate Guidance

Use the stream gate with per-block latency for release or regression checks:

```bash
RUN_STREAM_PATH_GATE=1 \
RSE_STREAM_PROFILE=fast \
RSE_STREAM_IO_MODE=auto \
RSE_STREAM_PATH_BASELINE=/path/to/stream-path-results.json \
./scripts/release-check.sh
```

For noisy cases, rerun the exact case with a 20 second cooldown and
`RSE_STREAM_ITERATIONS=10` or higher before deciding whether the change is a
real regression.
