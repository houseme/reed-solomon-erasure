# x86_64 SIMD Runtime Dispatch 最终交付总结

## 2026-06-16 状态勘误

本文主体内容对应的是 `2026-05-26` 前后的阶段性收官总结，其中“当前默认策略”一节已经不再反映最新 `main`。

截至 `2026-06-16` 当前工作区：

1. 当前 `x86_64` 自动选路已是 `rust-gfni-avx512 -> rust-gfni-avx2 -> rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust`
2. 最新同机完整压测结果见 [x86_64-simd-benchmark-summary-2026-06-16-amd-epyc-9v45-96-core-processor.md](/data/rustfs/reed-solomon-erasure/docs/x86_64-simd-benchmark-summary-2026-06-16-amd-epyc-9v45-96-core-processor.md)
3. 因而本文中“`GFNI` 仅支持 override”“`AVX2` 当前默认高于 `AVX512`”等表述，应理解为历史快照，而不是当前代码结论

## 已完成

1. 平台与 ISA 已拆分到 `src/galois_8/{scalar,legacy,x86,aarch64}`
2. backend 元数据、稳定 `BackendId`、runtime dispatch 与 override 机制已落地
3. `rust-ssse3`、`rust-avx2`、`rust-avx512`、实验性 `rust-gfni-avx2` / `rust-gfni-avx512` 已接入
4. `simd_c` 已明确降级为 legacy fallback
5. `benchmark_smoke` 与 `galois_backend` benchmark 已输出 backend 元数据
6. x86 cross-backend conformance matrix 已覆盖 `mul_slice` 与 `mul_slice_xor`

## 当前默认策略

`x86_64` 自动选路当前为：

1. `rust-avx2`
2. `rust-avx512`
3. `rust-ssse3`
4. `simd-c`
5. `scalar-rust`

补充说明：

1. `GFNI` 仍为实验性 backend，仅支持 override
2. 2026-05-26 在 `AMD EPYC 9V45` 上的实测结果支持继续保持 `AVX2` 默认高于 `AVX512`

## 本轮核查后确认的未完成项

1. 仍缺跨机器、多轮次的 `AVX2 vs AVX512` 实测性能报告
2. 缺少 `GFNI` 的系统化性能报告
3. `GFNI` 虽已有设计说明草案，但仍缺少更正式的数学推导与准入材料沉淀

## 结论

当前代码与基于实测结果修正后的 `docs/` 已经对齐，可以视为“首轮实现完成、已有单机压测依据、跨机器性能收口未全部完成”的状态。

如果后续要宣告完全收官，建议以以下顺序补齐：

1. 先补更多 `AVX2 / AVX512 / GFNI` benchmark 证据
2. 再补 `GFNI` 设计与性能文档
3. 最后再决定是否调整默认优先级
