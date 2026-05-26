# 子任务 01：backend 元数据模型与 runtime dispatch 重构

## 实施状态

已完成。

实际提交：

1. `6eaa202` `refactor(dispatch): introduce backend ids and feature-driven selection`

实际落地结果：

1. 已引入 `BackendId`
2. 已引入 `active_backend_id()`
3. selector 已改为 feature-driven helper 结构
4. override 已从只看名字扩展为可断言 backend id

## 1. 子任务目标

在平台拆分完成后，重构 `backend.rs`，建立稳定的 backend 标识、特性探测、需求表达、runtime 选路和 override 机制。

## 2. 本阶段必须解决的问题

当前 selector 存在以下不足：

1. backend 元信息过粗。
2. 没有独立的 `BackendId` 稳定标识。
3. `x86_64` 特性探测不够细，无法表达 `SSSE3 / AVX512 / GFNI`。
4. 测试预期容易和 selector 演进脱节。

## 3. 目标设计

### 3.1 新的数据模型

需要引入：

1. `BackendId`
2. `BackendImplKind`
3. `X86FeatureSet`
4. `BackendRequirement` 或隐式 requirement 判断函数

### 3.2 核心原则

1. backend 选择必须可解释。
2. selector 必须只做选择，不混入算法。
3. 特性探测只做一次并缓存。
4. override 行为必须可测。

## 4. 实施步骤

### 步骤 1：定义新类型

在 [src/galois_8/backend.rs](/data/rustfs/reed-solomon-erasure/src/galois_8/backend.rs:1) 中引入：

1. `BackendId`
2. `BackendImplKind`
3. `X86FeatureSet`
4. `GaloisBackend` 扩展字段

### 步骤 2：实现特性探测

`x86_64` 上探测：

1. `sse2`
2. `ssse3`
3. `avx2`
4. `avx512f`
5. `avx512bw`
6. `gfni`

`aarch64` 上保持：

1. `neon`

### 步骤 3：抽象 requirement 判断

建议对每个 backend 提供独立判断函数，例如：

1. `supports_rust_ssse3(&features)`
2. `supports_rust_avx2(&features)`
3. `supports_rust_avx512(&features)`
4. `supports_rust_gfni_avx2(&features)`
5. `supports_rust_gfni_avx512(&features)`
6. `supports_simd_c(&features)`

### 步骤 4：重构自动选择顺序

先实现完整顺序框架，即使某些 backend 还未真正提供实现，也要为后续扩展保留结构。

推荐策略：

1. 对未落地 backend 先不注册。
2. 选路逻辑用“按优先级依次尝试已注册 backend”实现。

### 步骤 5：增强 override

1. 扩展 `RSE_BACKEND_OVERRIDE` 的可选值。
2. 增加对非法值和不可用 backend 的处理。
3. 测试 override 不得只断言字符串，应断言 `BackendId`。

## 5. 测试要求

必须新增或增强：

1. override 解析测试
2. selector 优先级测试
3. 不同 CPU feature 组合的模拟测试
4. active backend 元数据测试

建议通过 helper 模拟：

1. “仅 SSSE3”
2. “AVX2 但无 AVX512”
3. “AVX512 但无 GFNI”
4. “GFNI + AVX2”
5. “GFNI + AVX512”
6. “无 SIMD 特性”

## 6. 设计注意事项

1. 不要把 `std::is_x86_feature_detected!` 调用散落到每个 backend 文件中。
2. 不要让 backend 文件自己决定优先级。
3. `BackendId` 必须稳定，不随文案变动。
4. `name` 保持对用户和 benchmark 友好。

## 7. 完成定义

满足以下条件视为完成：

1. 新元数据模型落地。
2. selector 可以表达后续所有目标 backend。
3. 现有可用 backend 仍可正常运行。
4. override 与自动分发都具备测试覆盖。

## 8. 推荐 commit

```text
refactor(dispatch): introduce backend ids and feature-driven runtime selection
```
