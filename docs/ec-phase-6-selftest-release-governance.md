# 阶段 6：自检、发布治理与长期演进机制

## 1. 阶段目标

建立一套长期可持续的工程防线，保证后续并行、SIMD、缓存、矩阵策略优化不会悄悄引入 correctness 问题或性能倒退。

## 2. 交付物

1. self-test 机制
2. golden output 校验
3. benchmark regression gate
4. 跨 ISA 一致性验证
5. 发布前检查清单

## 3. 任务拆解

### 任务 1：self-test

建议新增：

- 开发期显式自检入口
- debug / test / feature gated 自检

自检内容：

- 固定配置编码输出 hash
- 删除 shard 后重建
- verify 正确/错误行为

### 任务 2：cross-backend consistency

对同一输入比较：

- scalar
- SIMD backend A
- SIMD backend B

目标：

- 确认后端间结果一致

### 任务 3：benchmark regression gate

做法：

- 保存基准结果快照
- PR 或发布前对关键场景做对比

建议设定：

- 超过某阈值性能退化时报警

### 任务 4：发布前检查清单

至少包括：

- tests 通过
- benchmark smoke 通过
- SIMD consistency 通过
- `std` / `no_std` 检查通过
- 文档同步更新

### 任务 5：长期维护机制

建议在文档中维护：

- 新 ISA 接入流程
- 新矩阵模式接入流程
- benchmark 更新流程

## 4. 验收标准

1. 存在自检入口
2. 存在至少一组 golden outputs
3. 核心 benchmark 有回归对比机制
4. 不同 backend 的一致性可自动验证

## 5. 风险点

- 自检过重会影响开发效率
- benchmark gate 若设计不合理，容易产生误报

## 6. 风险应对

- smoke / full 两档校验
- 基准回归看中位数而非单次结果

## 7. 完成后的收益

- 该 crate 从“可用实现”升级为“可长期安全演进的高性能库”
- 后续任何 SIMD 或并发优化都有可靠护栏

## 8. 当前落地状态（2026-05-24）

已完成：

- [x] golden output 测试已具备（`tests/golden_vectors.rs`）
- [x] benchmark smoke 基础测试已具备（`tests/benchmark_smoke.rs`）
- [x] 并行 helper 对照基准已有测试侧输出能力

未完成 / 差距：

- [x] 已新增统一 self-test 显式入口（`cargo test --test selftest`）
- [x] benchmark regression gate 已提供可执行脚本入口（`scripts/check_benchmark_regression.py`），并接入 `scripts/release-check.sh`
- [x] 跨 backend / 跨 ISA 一致性验证已提供可复用自动流程入口（`scripts/check_backend_consistency.sh`）
- [x] 发布前检查清单已固化为可执行脚本（`scripts/release-check.sh`）

## 9. 执行待办（按优先级）

### P0（发布安全底线）

- [x] 新增 self-test 入口（`cargo test --test selftest`）
- [x] 固化 smoke/full 两档回归命令，并写入文档（`docs/benchmark-methodology.md`）
- [x] 增加发布前检查脚本（`scripts/release-check.sh`）：
  - `cargo check --tests`
  - `cargo test`
  - `cargo test --no-default-features`
  - `cargo test --features "std simd-accel"`（平台可用时）

### P1（回归治理）

- [x] 引入 benchmark 基线快照对比机制（阈值报警）
- [x] 将关键 case 纳入回归门槛（例如 10+4/32+16 的 encode/verify/reconstruct）
- [x] 已为阶段 3/5 的性能输出补统一核心 schema（`schema_version` / `artifact_kind` / 核心比较字段）

### P2（长期演进）

- [x] 完成跨 ISA consistency 自动化流程设计（本地复用脚本入口）
- [x] 已增加新 ISA、新矩阵模式接入模板流程（文档化）
- [x] 已增加“何时更新 benchmark 基线”的治理规范

## 10. 建议 PR 拆分

1. `phase6-selftest-entry`: 新增 self-test 入口与说明
2. `phase6-release-checks`: 发布前检查脚本与命令固化
3. `phase6-benchmark-gate`: benchmark 基线对比与阈值报警
4. `phase6-consistency-workflow`: 跨 backend/ISA 一致性自动化

## 11. 验收命令

```bash
cargo check --tests
cargo test --test golden_vectors --test benchmark_smoke
cargo test --no-default-features
cargo test --features std
cargo test --features "std simd-accel"
```
