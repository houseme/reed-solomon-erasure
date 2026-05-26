# 子任务 03：x86_64 SSSE3 backend 新增

## 实施状态

已完成。

实际提交：

1. `be56c50` `feat(x86): add ssse3 mul_slice backends`

实际落地结果：

1. `rust-ssse3` backend 已实现
2. selector 已接入 `SSSE3`
3. override 已支持 `rust-ssse3`

残余风险：

1. 相关测试仍需增加 CPU 特性 gating，避免在不支持 `SSSE3` 的机器上直接执行

## 1. 子任务目标

为 `x86_64` 平台补齐 `SSSE3` backend，使没有 `AVX2` 的机器也能获得现代 SIMD 路径，建立 `scalar -> SSSE3 -> AVX2 -> AVX512 -> GFNI` 的清晰梯度。

## 2. 为什么 `SSSE3` 必须先于 `AVX512 / GFNI`

1. 实现复杂度低于 `AVX512 / GFNI`。
2. 它能先验证 selector、backend 注册、跨 ISA 测试框架是否健康。
3. 它补足老机器中间档位，对真实兼容性更有价值。

## 3. 实现策略

### 3.1 算法路线

使用与 AVX2 相同的 nibble-table 模型：

1. 低 4 bit 查 `MUL_TABLE_LOW`
2. 高 4 bit 查 `MUL_TABLE_HIGH`
3. 使用 `pshufb` 做字节表查
4. 结果异或合成

### 3.2 指令要求

1. `SSSE3` 是必须条件。
2. 不依赖 AVX2。
3. `SSE2` 仅作为更低一层 fallback，不承担 `pshufb` 路线。

### 3.3 文件布局

新增：

1. `src/galois_8/x86/ssse3.rs`

内容包括：

1. `rust_ssse3_mul_slice`
2. `rust_ssse3_mul_slice_xor`
3. `#[target_feature(enable = "ssse3")]` 的内部实现

## 4. 实施步骤

### 步骤 1：写基础实现

要求：

1. 每次处理 16B。
2. 先实现正确版本。
3. 暂不做复杂展开。

### 步骤 2：注册 backend

在 `backend.rs` 中新增：

1. `BackendId::RustSsse3`
2. `RUST_SSSE3_BACKEND`
3. `supports_rust_ssse3(...)`
4. override 解析

### 步骤 3：接入 selector

顺序要求：

1. 放在 `AVX2` 之后
2. 放在 `simd_c` 之前

### 步骤 4：新增定向测试

至少新增：

1. `SSSE3 vs scalar`
2. `SSSE3 vs simd_c`
3. `SSSE3 vs AVX2`
4. 边界长度测试
5. 非对齐 slice 测试

## 5. benchmark 重点

要特别测以下维度：

1. 小块与大块输入
2. `mul_slice`
3. `mul_slice_xor`
4. encode 链路是否可观察到收益

## 6. 完成定义

1. `SSSE3` backend 可运行。
2. 在仅 `SSSE3` 机器或模拟特性场景下能被 selector 选中。
3. 正确性与 scalar 完全一致。
4. 性能优于 scalar。

## 7. 推荐 commit

```text
feat(x86): add ssse3 mul_slice backends
```
