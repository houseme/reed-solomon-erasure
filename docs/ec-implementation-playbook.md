# Reed-Solomon-Erasure 实施作战手册

## 1. 目的

本手册不是背景分析，而是面向实际动手改造时的执行指南。

如果主方案文档回答的是“做什么、为什么做”，本手册回答的是：

- 先做哪一步
- 每一步改哪些文件
- 每一步如何验证
- 每一步怎么切 PR
- 哪些风险必须在提交前拦住

## 2. 推荐实施顺序

### 第一波

目标：

- 最小风险建立可持续演进的骨架

任务顺序：

1. 阶段 1：benchmark 和 correctness 基线
2. 阶段 2：`CodecOptions`、`reconstruct_some`、`fast_one_parity`

这一波不要碰：

- runtime SIMD dispatch
- Leopard
- 大规模 reconstruction 重构

### 第二波

目标：

- 吃到最直接的性能红利

任务顺序：

1. chunk scheduler
2. auto thread policy
3. encode 并行
4. verify 并行
5. reconstruct 并行

### 第三波

目标：

- 让 SIMD 能力从“能用”升级为“现代化”

任务顺序：

1. backend 抽象
2. runtime dispatch
3. GFNI 优化
4. ARM64 路线强化

### 第四波

目标：

- 深化 reconstruction 性能

任务顺序：

1. cache metrics
2. `reconstruct_data` 优化
3. `reconstruct_some` 联动
4. 命中模式 benchmark

### 第五波

目标：

- 让项目进入可长期维护状态

任务顺序：

1. self-test
2. backend consistency
3. benchmark regression gate
4. 发布清单

## 3. 推荐 PR 切分

### PR 1

主题：

- benchmark matrix
- benchmark 输出
- golden vector baseline

建议改动文件：

- `benches/*`
- `tests/*`
- `docs/*`

### PR 2

主题：

- `CodecOptions`
- inversion cache 开关

建议改动文件：

- `src/core.rs`
- `src/lib.rs`
- `tests/*`
- `docs/*`

### PR 3

主题：

- `reconstruct_some`
- `split` / `join`

### PR 4

主题：

- `fast_one_parity`

### PR 5

主题：

- chunk scheduler
- auto threads

### PR 6

主题：

- encode/verify 并行化

### PR 7

主题：

- reconstruct 并行化

### PR 8

主题：

- SIMD backend abstraction

### PR 9

主题：

- runtime dispatch
- GFNI / ARM64 改进

### PR 10

主题：

- self-test
- regression gate

## 4. 文件级改造建议

### `src/core.rs`

优先负责：

- `CodecOptions`
- matrix mode 接口
- cache 策略
- reconstruction API
- 并行调度接入

### `src/galois_8.rs`

优先负责：

- backend 分发层
- scalar fallback
- SIMD 接口适配

### `simd_c/reedsolomon.c`

优先负责：

- 保留现有 SIMD 核
- 作为 runtime dispatch 的一个后端来源

### `build.rs`

优先负责：

- backend 编译组织
- 弱化固定 `-march=haswell`

### `benches/*`

优先负责：

- 统一 benchmark 口径
- ISA/feature 元数据输出

## 5. 每一步的验证要求

### 通用验证

每次核心改动后至少执行：

- `cargo check --tests`
- `cargo test --no-run`

若涉及 bench：

- `cargo check --benches`

若涉及 `no_std`：

- 至少做一次 `no_std` 方向编译检查

### 并行相关验证

必须新增：

- 单线程结果与多线程结果一致性测试
- 小 shard 和大 shard 的性能对比

### SIMD 相关验证

必须新增：

- scalar vs SIMD 输出一致性测试
- 至少一组随机输入 hash 对照
- 至少一组固定 golden vector 对照

## 6. 性能门槛建议

### 并行阶段门槛

- `10+4, 1MiB` encode 吞吐提升目标：`>= 2x`
- `64KiB` 场景退化限制：`<= 10%`

### SIMD 阶段门槛

- GFNI 对比 AVX2：目标 `15%~30%+`
- ARM64 路径：至少无退化，理想目标 `10%+`

### reconstruction 阶段门槛

- 常见重复缺失模式下 `reconstruct_data` 要优于当前基线

## 7. 风险拦截点

出现以下情况时，不建议继续扩大改造范围：

1. benchmark 波动过大，无法说明收益
2. scalar 和 SIMD 结果出现一次不一致
3. 并行路径在小 shard 明显恶化
4. options 设计已让 API 变得混乱

## 8. 实施纪律

1. 每一波改造都要有“前后 benchmark 对照”。
2. 不要在没有 benchmark 的情况下声称优化成功。
3. 不要把 SIMD、并行、API 改造混在一个大 PR 中。
4. 任意一步出现 correctness 不稳定，先停下来补护栏，再继续优化。

## 9. 建议的第一批实际落地内容

建议直接开始做下面这组内容：

1. benchmark matrix
2. golden vectors
3. `CodecOptions`
4. `reconstruct_some`
5. `fast_one_parity`

原因：

- 风险小
- 架构收益高
- 能为后续并行与 SIMD 提供稳定支点

