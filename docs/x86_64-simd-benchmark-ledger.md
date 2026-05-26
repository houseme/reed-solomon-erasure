# x86_64 SIMD Benchmark Ledger

本文档用于持续记录每一轮与 `x86_64` SIMD runtime dispatch 相关的代码改进、压测命令、机器环境与结论。

## 2026-05-26 Baseline Revalidation

### 背景

本轮目标不是按既有文档强推默认策略，而是先以当前机器实测结果验证默认 runtime dispatch 是否满足性能优先。

机器：

1. `AMD EPYC 9V45 96-Core Processor`
2. 指令集能力包含 `ssse3 / avx2 / avx512f / avx512bw / gfni`
3. 采样环境来自 `lscpu`，完整信息已写入 [benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json)

### 已执行命令

1. `./scripts/run_x86_backend_smoke_matrix.sh 2026-05-26 amd-epyc-9v45`
2. `RSE_BACKEND_OVERRIDE=rust-avx2 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
3. `RSE_BACKEND_OVERRIDE=rust-avx512 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
4. `RSE_BACKEND_OVERRIDE=rust-gfni-avx512 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`

### Release Smoke 结论

关注案例：`10x4_1m`

1. `encode` 最快是 `rust-avx2`，`315.6882 MB/s`
2. `verify` 最快是 `rust-avx2`，`534.4593 MB/s`
3. `reconstruct` 最快是 `rust-avx2`，`721.9651 MB/s`
4. `reconstruct_data` 最快是 `rust-avx2`，`753.3249 MB/s`
5. `auto` 当前选中的 backend 是 `rust-avx2`，但实测仍略低于显式 `rust-avx2` override

综合排序：

1. `rust-avx2`
2. `rust-avx512`
3. `rust-gfni-avx512`
4. `rust-gfni-avx2`
5. `rust-ssse3`
6. `scalar`
7. `simd-c`

### Microbenchmark 观察

基于 `galois_backend`：

1. `rust-avx512` 在当前采样里没有稳定证明优于 `rust-avx2`
2. `rust-gfni-avx512` 在当前采样里也没有证明适合作为默认优先
3. 当前机器上，`AVX512 / GFNI` 更适合作为可选实验或定向 override，而不是自动主路径

### 本轮改进与结论

1. 保留 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust` 的默认自动顺序
2. 保留 `GFNI` 为 override-only 实验 backend
3. 更新 benchmark 汇总脚本中的 `current_runtime_priority_x86`，使其反映当前真实代码策略
4. 新增 `scripts/run_x86_backend_smoke_matrix.sh`，为后续每轮改进提供统一 smoke 采集入口

### 后续准入规则

1. 只有当 `AVX512` 在 release smoke 与关键微基准上稳定优于 `AVX2`，才考虑提升默认优先级
2. 只有当 `GFNI` 在正确性、性能与文档说明三方面同时收口，才考虑退出 override-only
3. 每次涉及 selector 或 backend 行为调整，都应先更新本 ledger，再同步更新摘要文档
