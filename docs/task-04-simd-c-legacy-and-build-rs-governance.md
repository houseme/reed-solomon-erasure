# 子任务 04：simd_c legacy fallback 治理与 build.rs 修正

## 实施状态

已完成。

实际提交：

1. `68b188a` `refactor(simd_c): demote c backend to legacy fallback`

实际落地结果：

1. `simd_c` 已明确降级为 legacy fallback
2. `build.rs` 默认只构建 baseline C 路径
3. `RUST_REED_SOLOMON_ERASURE_ARCH` 已变成显式 legacy/实验控制项

## 1. 子任务目标

将 `simd_c` 从“隐式高性能默认主路径”调整为“显式 legacy fallback”，并同步修正 `build.rs` 的架构治理方式。

## 2. 当前问题

现状中 `simd_c` 存在两个核心问题：

1. `build.rs` 默认 `-march=haswell` 容易让 C backend 与运行时策略耦合。
2. `simd_c` 的能力表达不够精细，无法自然融入未来多 ISA 体系。

## 3. 本阶段目标状态

1. `simd_c` 明确作为 fallback/过渡 backend。
2. Rust intrinsic backend 成为主路径。
3. `build.rs` 只负责“生成哪些可用产物”，不负责“最终最佳选路”。

## 4. 实施步骤

### 步骤 1：整理 `simd_c` backend 标识

建议最少明确：

1. `simd-c`
2. `simd-c-sse2` 或等价稳定命名

### 步骤 2：调整 selector 中的优先级

要求：

1. `simd_c` 放在 Rust SIMD 之后。
2. 仅在更高阶 Rust backend 不可用时才进入自动选择。

### 步骤 3：改造 `build.rs`

建议策略：

1. 默认构建 baseline-safe C backend。
2. 将 `RUST_REED_SOLOMON_ERASURE_ARCH` 定位为 opt-in 的 legacy/实验控制项。
3. 明确记录构建出来的 `simd_c` backend 能力边界。

### 步骤 4：修复测试假设

当前测试如果仍把 Haswell 默认 backend 视为 `simd-c`，需要同步修正为新契约。

## 5. 设计约束

1. 不能因为降级 `simd_c` 而破坏老平台可运行性。
2. 不能让 `build.rs` 变得比当前更难跨编译。
3. 保持用户已有 env override 习惯尽量兼容。

## 6. 测试要求

至少验证：

1. 无 Rust SIMD backend 可用时 `simd_c` 仍可工作。
2. Rust SIMD 可用时 `simd_c` 不再抢占自动优先级。
3. `RSE_BACKEND_OVERRIDE=simd-c` 仍可按预期生效。

## 7. 完成定义

1. `simd_c` 成为清晰的 fallback/legacy backend。
2. `build.rs` 不再把“最优选择”硬编码到编译期。
3. selector、新测试、README/说明文档语义一致。

## 8. 推荐 commit

```text
refactor(simd_c): demote c backend to legacy fallback
build(simd_c): decouple c backend build from runtime priority
```
