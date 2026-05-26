# x86_64 SIMD GFNI Benchmark Template (Second Machine)

本文档用于预留第二台支持 `GFNI` 的 x86_64 机器 benchmark 结果归档格式。

当第二台机器可用时，请复制本模板并替换文件名中的日期与机器标识，例如：

`docs/x86_64-simd-benchmark-summary-2026-06-XX-<machine>.md`

## 机器信息

1. CPU 型号：
2. 逻辑核 / 物理核：
3. 关键 ISA：`ssse3 / avx2 / avx512f / avx512bw / gfni`
4. 操作系统：
5. 测试日期：
6. 对应 JSON：

## 执行命令

1. `./scripts/run_x86_backend_smoke_matrix.sh <date> <machine-slug>`
2. `RSE_BACKEND_OVERRIDE=rust-avx2 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
3. `RSE_BACKEND_OVERRIDE=rust-avx512 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
4. `RSE_BACKEND_OVERRIDE=rust-gfni-avx2 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
5. `RSE_BACKEND_OVERRIDE=rust-gfni-avx512 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`

## Release Smoke 结论

关注案例建议保持为 `10x4_1m`，便于与现有 `AMD EPYC 9V45` 结果横向对比。

`encode`

1. 第一名：
2. 第二名：
3. 第三名：

`verify`

1. 第一名：
2. 第二名：
3. 第三名：

`reconstruct`

1. 第一名：
2. 第二名：
3. 第三名：

`reconstruct_data`

1. 第一名：
2. 第二名：
3. 第三名：

## 综合打分与策略结论

1. `recommended_default_priority`：
2. `policy_eligible_default_priority`：
3. `GFNI` 是否仅在 raw ranking 中靠前：
4. `GFNI` 是否满足默认启用前提：

## 与第一台机器的对比

请至少回答以下问题：

1. `rust-avx2` 是否仍然是综合最优默认路径？
2. `GFNI` 是否在更多 than one machine 上稳定优于 `rust-avx2`？
3. `AVX512` 是否在第二台机器上比第一台机器更有优势？
4. 是否出现与第一台机器相反的排序结论？

## 策略建议

1. 是否保持 `rust-avx2` 默认首选：
2. 是否继续保持 `GFNI` 为 `override-only`：
3. 是否需要补更多机器样本：
