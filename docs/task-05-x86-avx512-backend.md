# 子任务 05：x86_64 AVX512 backend 新增与门禁验证

## 实施状态

已完成首轮实现，默认优先级已回退为保守策略。

实际提交：

1. `1d9db55` `feat(x86): add avx512 backend for mul_slice paths`

实际落地结果：

1. `rust-avx512` backend 已实现
2. selector 已支持 `AVX512`
3. override 已支持 `rust-avx512`

当前状态：

1. 默认自动优先级已回退为 `AVX2 -> AVX512`
2. `rust-avx512` 仍保留 override 能力
3. 是否重新升到 `AVX2` 之前，仍需 benchmark 证据支撑

## 1. 子任务目标

为支持 `AVX512F + AVX512BW` 的 `x86_64` 机器新增 64B 宽度 backend，并通过 benchmark 验证其是否适合作为自动优先路径。

## 2. 实施原则

1. 先正确，再判断是否默认启用。
2. 先沿用 nibble-table + shuffle 体系，不引入更高风险的数学改写。
3. 关注频率降档风险，不用单机单次数据做结论。

## 3. 前置条件

开始本阶段前必须已完成：

1. 平台拆分
2. backend selector 重构
3. AVX2 稳定化
4. `simd_c` 定位治理

## 4. 实施步骤

### 步骤 1：新增文件

创建：

1. `src/galois_8/x86/avx512.rs`

### 步骤 2：实现 `mul_slice`

要求：

1. `#[target_feature(enable = "avx512f,avx512bw")]`
2. 64B 处理块
3. 尾部仍由 scalar 兜底

### 步骤 3：实现 `mul_slice_xor`

要求：

1. 与 `mul_slice` 共用相同 table 思路
2. 保持对 output 的 xor 语义完全一致

### 步骤 4：注册 backend

新增：

1. `BackendId::RustAvx512`
2. `RUST_AVX512_BACKEND`
3. `supports_rust_avx512(...)`
4. override 值 `rust-avx512`

### 步骤 5：补充测试

新增或增强：

1. AVX512 与 scalar 对照
2. AVX512 与 AVX2 对照
3. AVX512 与 simd_c 对照
4. 长度边界与非对齐测试

## 5. benchmark 与默认启用规则

### 5.1 benchmark 场景

至少对比：

1. `rust-avx2`
2. `rust-avx512`
3. `scalar`

### 5.2 默认启用判定

仅当满足以下条件时，才允许将 `rust-avx512` 放在 `rust-avx2` 之前：

1. 多组长度下总体优于 AVX2
2. 无明显频率降档导致的回退
3. 实际编码链路而不仅是微基准有收益

若不满足，则保留：

1. 已实现
2. 可 override
3. 自动分发仍优先 `AVX2`

## 6. 完成定义

1. AVX512 backend 正确可用。
2. 门禁测试和 benchmark 数据齐全。
3. 自动优先级有明确证据支撑。

## 7. 推荐 commit

```text
feat(x86): add avx512 backend for mul_slice paths
```

若需要延后自动启用：

```text
feat(x86): add avx512 backend behind runtime gating
```
