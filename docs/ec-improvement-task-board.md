# Reed-Solomon-Erasure 改进执行总看板

## 1. 使用方式

本文件用于执行期跟踪，不是背景说明文档。

使用规则：

1. 每个阶段启动前，先确认前置条件是否满足。
2. 每个阶段结束后，必须记录结果、性能数据、风险结论。
3. 若阶段内拆 PR，建议使用“一个成果一条任务”的方式推进。
4. 不满足验收条件时，不进入下一阶段。

## 2. 总阶段状态

> 状态更新时间：2026-05-26（基于当前仓库代码与测试结果核实）

| 阶段 | 名称 | 目标 | 当前状态 | 依赖 |
|---|---|---|---|---|
| 1 | 基线与回归框架 | 建立性能与正确性基线 | 已完成 | 无 |
| 2 | API 与配置能力 | 补齐高价值 API 与 options | 已完成（第一批） | 阶段 1 |
| 3 | 并行调度 | 引入自动并发与分块执行 | 已完成（治理已补齐） | 阶段 1, 2 |
| 4 | SIMD 架构升级 | runtime ISA dispatch 与多后端 | 已完成（首批目标） | 阶段 1, 3 |
| 5 | 重建与缓存优化 | 降低 reconstruction 开销 | 部分完成 | 阶段 2, 3 |
| 6 | 自检与发布治理 | 稳定性、回归与发布保护 | 部分完成（治理闭环推进中） | 阶段 1-5 |

### 2.1 状态说明（核实结论）

- 阶段 1（已完成）：
  - 已有 `benches/common/mod.rs`、`tests/benchmark_smoke.rs`、`tests/golden_vectors.rs`。
  - `benchmark_smoke_matrix_runs_and_exports_results` 与 `golden_vectors` 已通过。
  - benchmark 运行说明文档已补齐（`docs/benchmark-methodology.md`）。
- 阶段 2（已完成第一批）：
  - `CodecOptions`、`with_options`、`split/join`、`fast_one_parity`、`reconstruct_some` 已在代码与测试中落地。
- 阶段 3（已完成，治理已补齐）：
  - 已有 chunk 化执行路径与 `std` 并行入口（`encode_*_par` / `verify_*_par` / `reconstruct_*_opt`）。
  - “线程数自动推导策略”已形成独立策略层（`ParallelPolicy` / `ParallelDecision`），并已接入自动选择路径。
- 阶段 4（已完成首批目标）：
  - runtime ISA dispatch、多后端抽象、GFNI/ARM64 runtime 路线已经在代码与阶段文档中落地。
  - 后续仍可继续做更细的性能追踪与新 ISA 扩展，但不再属于“未开始”状态。
- 阶段 5（部分完成）：
  - cache 可观测（stats/metrics）与部分 reconstruction 优化已具备。
  - cache 已补齐 `evictions`，并提供统一分析口径（`hit_rate/reuse_ratio/miss_cost_per_request`）。
  - cache 默认容量已改为按 workload 自动调优。
  - `reconstruct_data` / `reconstruct_some` 专项对照基准与结果导出已落地；剩余缺口是将热点场景进一步沉淀为稳定 gate。
- 阶段 6（部分完成）：
  - golden vectors 与 self-test 入口已存在并可运行（`cargo test --test selftest`）。
  - 发布前检查脚本已补齐（`scripts/release-check.sh`）。
  - benchmark regression gate 与 backend/ISA consistency 自动回归入口已补齐，阶段 3/5 的可比较 schema 与 baseline 更新治理规则也已写入 `docs/benchmark-methodology.md`。

## 3. 统一验收标准

所有阶段都必须满足：

1. 现有公开 API 不出现未说明的破坏性变化。
2. `cargo check --tests` 通过。
3. 若涉及基准变动，必须记录“前后对比数据”。
4. 若涉及热路径改动，必须补充至少一项针对性测试或 benchmark。
5. 若涉及 SIMD 或并行，必须考虑平台兼容与回退路径。

## 4. 第一批建议任务拆分

### 阶段 1

- 建 benchmark 配置矩阵
- 输出 benchmark 结果文件
- 固定 correctness golden vector
- 增加 benchmark 运行说明

### 阶段 2

- 引入 `CodecOptions`
- 增加 `reconstruct_some`
- 增加 `split` / `join`
- 增加 `fast_one_parity`
- 增加 cache enable/disable 选项

### 阶段 3

- 设计 chunk scheduler
- 引入线程数自动推导
- encode 并行化
- verify 并行化
- reconstruct 并行化

### 阶段 4

- 设计 ISA dispatch 层
- 抽象 SIMD backend trait
- 迁移现有 SIMD-C 路径
- 增加 GFNI 路线
- 增加 ARM64 runtime 路线

### 阶段 5

- cache metrics
- reconstruction 命中率分析
- `reconstruct_data` 专项优化
- partial recovery 路线

### 阶段 6

- self-test
- 黄金向量验证
- benchmark gate
- 跨 ISA 回归方案

## 5. 里程碑建议

### 里程碑 M1

完成阶段 1 和阶段 2。

完成标志：

- 有统一 benchmark 基线
- 有 options 层
- 有 `reconstruct_some`
- 有 `fast_one_parity`

### 里程碑 M2

完成阶段 3。

完成标志：

- `std` 下核心编码流程已并行化
- 能根据 shard 大小自动选择并发粒度

### 里程碑 M3

完成阶段 4 和阶段 5。

完成标志：

- SIMD 具备 runtime dispatch
- reconstruction cache 可观测且可配置

### 里程碑 M4

完成阶段 6。

完成标志：

- 有自检
- 有发布前回归机制
- 有性能退化检测
