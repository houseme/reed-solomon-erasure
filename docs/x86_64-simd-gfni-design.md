# x86_64 SIMD GFNI Design Notes

## 文档目标

本文档记录当前 `GFNI` backend 的实现思路、代码中已经落地的 basis conversion 结构，以及默认不自动启用的原因。

它不是完整数学证明，但用于把“代码里做了什么”和“为什么现在还只是实验路径”说明白。

## 当前状态

1. 已实现 `rust-gfni-avx2`
2. 已实现 `rust-gfni-avx512`
3. 两者都只通过 `RSE_BACKEND_OVERRIDE` 暴露
4. 当前 runtime dispatch 不会自动选择 `GFNI`

相关实现见：

1. [src/galois_8/x86/gfni.rs](/data/rustfs/reed-solomon-erasure/src/galois_8/x86/gfni.rs)
2. [src/galois_8/backend.rs](/data/rustfs/reed-solomon-erasure/src/galois_8/backend.rs)

## 设计背景

本库当前有限域实现基于 `GF(2^8)`，其代码注释与现有设计记录都指向 `0x11d` 这一路径，而 `GFNI` 指令常见讨论更接近 AES 语境下的另一种域表示。

这带来两个工程事实：

1. 不能假设“把现有输入直接喂给 `GFNI` 乘法”就一定语义等价
2. 如果 basis conversion 设计有误，会出现 silent corruption，而不仅仅是性能回退

## 当前代码实现的乘法流程

以 `rust_gfni_avx2_mul_slice_impl()` 为例，当前实现流程是：

1. 先构造一个 8x8 仿射变换矩阵 `GFNI_ISOMORPHISM_ROWS`
2. 通过 `gfni_isomorphism_bytes()` 把它编码成 `GF2P8AFFINE` 所需的字节布局
3. 对输入字节先做一次 affine 映射
4. 对常量系数 `c` 也做同一套 affine 映射
5. 使用 `GF2P8MUL` 在映射后的域表示下做逐字节乘法
6. 再对乘法结果做一次 affine 映射，回到当前库使用的表示

对应代码路径：

1. `gfni_avx2_constants()`
2. `rust_gfni_avx2_mul_slice_impl()`
3. `rust_gfni_avx2_mul_slice_xor_impl()`
4. `gfni_avx512_constants()`
5. `rust_gfni_avx512_mul_slice_impl()`

## 当前代码中已经明确的假设

从实现可以看出，当前 `GFNI` backend 依赖以下假设：

1. `GFNI_ISOMORPHISM_ROWS` 描述的是一个可逆 basis change
2. 该 basis change 既适用于输入字节，也适用于常量系数
3. 对输入和乘法结果施加同一 affine 结构，可以在当前实现里完成“进域”和“回域”的闭环

这些假设已经通过现有 cross-backend correctness 测试得到工程层面的支持，但还没有在独立文档里写成完整推导。

## 为什么当前仍保持 override-only

即使本轮 benchmark 显示 `rust-gfni-avx2` 在当前机器上具备一定潜力，也仍不应自动启用，原因是：

1. 当前 benchmark 主要来自单机 `AMD EPYC 9V45`
2. `GFNI` 的设计说明还没有形成正式、可审阅的数学文档
3. 当前性能结论还没有完成跨机器复核
4. `GFNI` 属于高风险 backend，默认启用门槛应高于 `AVX2 / AVX512`

## 默认启用前应满足的条件

1. 补齐 basis conversion 的正式设计说明
2. 继续扩展 cross-backend correctness 覆盖
3. 在至少一台以上 `GFNI` 机器上完成同口径 smoke 与 benchmark 复核
4. 明确 `GFNI` 相比 `rust-avx2` 的收益是否稳定且可复现

## 建议的准入清单

若未来要讨论把 `GFNI` 放入默认 runtime dispatch，建议至少同时满足以下四类证据：

### 1. 数学与表示层证据

1. 明确记录当前库使用的域表示与 `GFNI` 工作域之间的映射关系
2. 明确说明 `GFNI_ISOMORPHISM_ROWS` 的来源或推导方法
3. 明确说明为什么“输入映射 -> 常量映射 -> GF2P8MUL -> 输出映射”保持语义等价

### 2. 正确性证据

1. `mul_slice` 与 `mul_slice_xor` 都已纳入 cross-backend conformance matrix
2. 对 `scalar / avx2 / avx512 / gfni` 都有稳定对照
3. 长度边界、非对齐输入、不同系数覆盖都已保留

### 3. 性能证据

1. 至少在两台支持 `GFNI` 的机器上复跑同口径 smoke matrix
2. 至少在一轮 `galois_backend` microbenchmark 中证明收益不是偶然噪音
3. 至少在 `encode / verify / reconstruct / reconstruct_data` 里说明收益集中在哪些场景

### 4. 策略证据

1. benchmark 结果需要区分“raw 性能排序”与“当前可进入默认策略的排序”
2. 若 `GFNI` 仅在单机获胜，不应直接提升为默认路径
3. 若 `GFNI` 只在微基准获胜，但在集成 smoke 上没有稳定优势，也不应提升

## 待补证据模板

后续每次补 `GFNI` 相关材料，建议至少按下面四项补充：

1. 机器信息：CPU 型号、关键 ISA、测试日期
2. 正确性信息：跑了哪些 cross-backend case、是否新增异常
3. 性能信息：与 `rust-avx2`、`rust-avx512` 的对比结论
4. 策略结论：是否仍保持 `override-only`

## 与 2026-05-26 Benchmark 的关系

根据 [docs/x86_64-simd-benchmark-summary-2026-05-26.md](/data/rustfs/reed-solomon-erasure/docs/x86_64-simd-benchmark-summary-2026-05-26.md)：

1. `rust-avx2` 仍是当前默认首选
2. `rust-gfni-avx2` 已经是值得继续追踪的候选实现
3. 但 `GFNI` 目前仍然是“性能有潜力、文档与验证未完全收口”的实验 backend
