# 子任务 00：平台与 ISA 拆分、行为冻结、风险隔离

## 实施状态

已完成。

实际提交：

1. `527a24e` `refactor(galois_8): isolate scalar baseline from simd backends`

实际落地结果：

1. `scalar` 基线已独立到 `src/galois_8/scalar.rs`
2. `mod.rs` 已收口为公共 API 与组装层
3. `aarch64 / x86 / legacy` 目录拆分已在代码中生效

## 1. 子任务目标

在不改变外部行为与性能承诺的前提下，将当前混合在 `galois_8` 大文件中的 `scalar`、`x86_64`、`aarch64`、`simd_c` 逻辑拆分到独立模块，为后续 `x86_64` SIMD 深化优化提供稳定基础。

## 2. 为什么先做这个子任务

如果不先拆分平台和 ISA，后续新增 `SSSE3 / AVX512 / GFNI` 时会同时放大以下风险：

1. `aarch64` 与 `x86_64` 分支条件越来越复杂。
2. selector 与实现绑定越来越深。
3. review 很难判断某个改动只影响哪一类机器。
4. 测试失败时不容易快速定位是平台拆分问题还是算法问题。

## 3. 本阶段范围

本阶段只做以下事情：

1. 代码目录拆分。
2. 模块边界定义。
3. 保持原函数签名与对外 API 不变。
4. 保持 runtime dispatch 表面行为不变。

本阶段不做以下事情：

1. 不新增 `SSSE3 / AVX512 / GFNI` 算法实现。
2. 不调整 backend 优先级。
3. 不大改 `build.rs` 策略。
4. 不引入新的 benchmark 结论。

## 4. 目标结构

建议落地结构：

```text
src/galois_8/
  mod.rs
  backend.rs
  scalar.rs
  legacy/mod.rs
  legacy/simd_c.rs
  x86/mod.rs
  x86/avx2.rs
  aarch64/mod.rs
  aarch64/neon.rs
```

## 5. 实施步骤

### 步骤 1：创建目录与模块骨架

1. 将现有 [src/galois_8.rs](/data/rustfs/reed-solomon-erasure/src/galois_8.rs:1) 重构为目录模块。
2. 把表、公共类型、公共 API 挪入 `src/galois_8/mod.rs`。
3. 将纯 Rust 标量逻辑移动到 `scalar.rs`。

### 步骤 2：拆分 `aarch64`

1. 将现有 NEON 代码完整迁入 `aarch64/neon.rs`。
2. 只保留必要的 `pub(crate)` 暴露。
3. 确保 `aarch64` 文件内不包含任何 `x86_64` intrinsic import。

### 步骤 3：拆分 `x86_64`

1. 将现有 AVX2 代码迁入 `x86/avx2.rs`。
2. 迁移时不做算法改写，保持逻辑等价。
3. 文件中仅允许 `x86_64` 条件编译与相关 intrinsic。

### 步骤 4：拆分 `simd_c`

1. 将 FFI 声明与包装迁入 `legacy/simd_c.rs`。
2. 保持 `mul_slice` / `mul_slice_xor` 接口不变。
3. 迁移尾部 fallback 逻辑时，不改变标量回退行为。

### 步骤 5：保持 selector 可编译

1. `backend.rs` 暂时仍可沿用当前简单逻辑。
2. 只把函数引用改向新模块。
3. 本阶段不调整选路优先级。

## 6. 关键检查点

### 检查点 A：公共 API 不变

必须保持以下接口签名不变：

1. `pub fn mul_slice(...)`
2. `pub fn mul_slice_xor(...)`
3. `pub fn active_backend_name()`
4. `pub fn active_backend_kind()`

### 检查点 B：条件编译边界清晰

要求：

1. `aarch64` 文件不出现 `x86_64` intrinsic。
2. `x86_64` 文件不出现 `aarch64` intrinsic。
3. `legacy/simd_c.rs` 中 FFI 与 ISA 逻辑隔离。

### 检查点 C：行为冻结

要求：

1. 原有测试仍应通过。
2. `active_backend_name()` 在当前平台上的结果不应因拆分而意外变化。

## 7. 测试要求

至少执行：

1. `cargo test`
2. `cargo test --features simd-accel`
3. `cargo test active_backend`
4. `cargo test mul_slice`

若环境依赖受限，至少记录未执行原因与待补验证项。

## 8. 完成定义

满足以下条件视为完成：

1. 模块结构拆分完成。
2. 对外 API 不变。
3. 当前可用测试通过。
4. 无新增功能，仅重构与平台隔离。

## 9. 推荐 commit

```text
refactor(galois_8): split simd code by platform and implementation kind
```

## 10. 风险与回退

风险：

1. 模块可见性调整导致编译失败。
2. 搬运过程中遗漏 `cfg` 条件。
3. 测试引用路径失效。

回退策略：

1. 仅在本阶段完成全部编译和核心测试后再 commit。
2. 若失败，优先恢复到“拆文件但不重命名过多符号”的中间状态，不进入下一阶段。
