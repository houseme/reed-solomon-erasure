# x86_64 SIMD Benchmark Baselines

This directory stores machine-specific benchmark captures used to validate runtime-dispatch ordering decisions.

Each JSON file should include:

1. machine identity and CPU capabilities
2. `galois_backend` criterion snapshots
3. `benchmark_smoke` release snapshots
4. enough metadata to reproduce the run
