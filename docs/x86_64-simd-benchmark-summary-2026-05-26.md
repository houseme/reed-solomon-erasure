# x86_64 SIMD Benchmark Summary (2026-05-26, AMD EPYC 9V45)

## 范围

本摘要对应以下实测产物：

1. [benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json)
2. `target/benchmark-smoke/smoke-results-release-*.csv`
3. `cargo bench --bench galois_backend --features 'std simd-accel'` 的当前 Criterion 输出

机器环境：

1. `AMD EPYC 9V45 96-Core Processor`
2. 支持 `ssse3 / avx2 / avx512f / avx512bw / gfni`

## 关键结论

### 默认自动策略

当前代码中的默认自动顺序：

1. `rust-avx2`
2. `rust-avx512`
3. `rust-ssse3`
4. `simd-c`
5. `scalar-rust`

本轮结论：

1. 该默认顺序在当前机器上仍然成立，不应回滚
2. `auto` 实际选中的 backend 仍是 `rust-avx2`
3. `rust-avx2` 在关键集成场景里仍是综合最优默认路径

### 10x4_1m Release Smoke 排名

`encode`

1. `rust-avx2`: `315.6882 MB/s`
2. `rust-gfni-avx512`: `296.1271 MB/s`
3. `rust-avx512`: `291.1466 MB/s`

`verify`

1. `rust-avx2`: `534.4593 MB/s`
2. `rust-avx512`: `280.0226 MB/s`
3. `auto`: `276.7033 MB/s`

`reconstruct`

1. `rust-avx2`: `721.9651 MB/s`
2. `rust-avx512`: `398.2624 MB/s`
3. `rust-gfni-avx2`: `348.2552 MB/s`

`reconstruct_data`

1. `rust-avx2`: `753.3249 MB/s`
2. `rust-avx512`: `406.3120 MB/s`
3. `rust-gfni-avx512`: `400.0279 MB/s`

## 关于 `recommended_default_priority` 的解读

JSON 中的综合打分结果为：

1. `rust-avx2`
2. `rust-gfni-avx2`
3. `rust-gfni-avx512`
4. `rust-avx512`
5. `rust-ssse3`
6. `scalar`
7. `simd-c`

这个结果不应直接等价为“现在就调整 runtime dispatch”，原因如下：

1. 它是单机结果，当前只代表 `AMD EPYC 9V45`
2. `GFNI` 仍是实验 backend，尚未满足长期维护和默认启用前提
3. `GFNI` 设计说明、更多正确性证据和跨机器 benchmark 仍未完全收口
4. 当前策略需要同时服从“性能优先”和“风险可控”，不能只看单次打分

因此，本轮决策是：

1. 保持 `rust-avx2` 为默认首选
2. 保持 `GFNI` 为 `override-only`
3. 继续把 `GFNI` 视为“值得追踪的候选路径”，但还不是默认路径

## 本轮附带改进

1. 新增统一采集脚本 `scripts/run_x86_backend_smoke_matrix.sh`
2. 补齐 `mul_slice_xor` 的 x86 cross-backend conformance matrix
3. 更新 benchmark ledger，使每轮改进都能附着到对应压测结论

## 后续建议

1. 在第二台支持 `GFNI` 的 x86_64 机器上复跑同一套 smoke matrix
2. 为 `rust-gfni-avx2` 增补更多微基准与编码链路数据
3. 在 `GFNI` 设计文档补齐后，再评估是否允许其参与默认优先级讨论
