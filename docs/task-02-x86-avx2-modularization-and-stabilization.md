# 子任务 02：x86_64 AVX2 模块化迁移与稳定化

## 实施状态

已完成。

实际提交：

1. `159d729` `refactor(x86): consolidate avx2 backend validation`

实际落地结果：

1. AVX2 backend 已独立收口在 `src/galois_8/x86/avx2.rs`
2. AVX2 定向测试已从 `mod.rs` 回收到模块内
3. table preload 公共逻辑已模块化

## 1. 子任务目标

将当前 `x86_64` AVX2 实现从混合文件中迁出，整理为清晰、稳定、可扩展的专用 backend，为后续 `SSSE3`、`AVX512`、`GFNI` 实现提供参照模板。

## 2. 本阶段范围

本阶段只聚焦 AVX2：

1. 不新增新的数学路径。
2. 不改变 GF 表示。
3. 不引入 GFNI。
4. 可以做轻量结构优化，但不做大规模性能冒进改写。

## 3. 实施步骤

### 步骤 1：迁移现有实现

将现有 AVX2 逻辑从 [src/galois_8.rs](/data/rustfs/reed-solomon-erasure/src/galois_8.rs:1048) 迁入新文件。

迁移范围包括：

1. `rust_avx2_mul_slice`
2. `rust_avx2_mul_slice_xor`
3. 对应 `#[target_feature(enable = "avx2")]` 的实现体

### 步骤 2：统一符号导出

在 `x86/mod.rs` 中导出：

1. `pub(crate) fn rust_avx2_mul_slice(...)`
2. `pub(crate) fn rust_avx2_mul_slice_xor(...)`

### 步骤 3：检查尾部 fallback

重点确认：

1. `bytes_done` 计算不变。
2. 标量尾部处理仍由 scalar 路径兜底。
3. `mul_slice` 与 `mul_slice_xor` 的尾部处理逻辑一致性保持。

### 步骤 4：补充 AVX2 定向测试

新增或整理以下测试：

1. AVX2 与 scalar 的 `mul_slice` 对照
2. AVX2 与 scalar 的 `mul_slice_xor` 对照
3. AVX2 与 `simd_c` 的对照
4. 长度边界与尾部边界测试

## 4. 可选微优化

只有在正确性稳定后，才考虑以下可选项：

1. 减少重复 table broadcast
2. 评估循环展开
3. 改善 load/store 调度

注意：

1. 若微优化会降低可读性，应延后到单独优化 commit。
2. 本阶段主目标是“稳定模板”，不是“极限榨干”。

## 5. 验收标准

1. AVX2 逻辑已独立模块化。
2. 与当前主线结果一致。
3. 性能不低于当前 AVX2 主线明显阈值。
4. 作为 selector 主线之一可稳定选中。

## 6. 推荐 commit

```text
refactor(x86): move avx2 backend into dedicated module
```

若包含小幅增强，也可分两次提交：

```text
refactor(x86): move avx2 backend into dedicated module
perf(x86): stabilize avx2 mul_slice backend structure
```
