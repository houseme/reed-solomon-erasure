# Reed-Solomon-Erasure 对标 MinIO EC 的完整实战改进方案

## 1. 文档目标

本文档用于指导 `reed-solomon-erasure` 对标 MinIO EC 路线及其底层 `klauspost/reedsolomon` 的工程能力，形成一套完整、可执行、可验证、可分阶段落地的演进方案。

本文档关注四类目标：

1. 对比 MinIO 中 EC 算法与当前 crate 的实现差异。
2. 明确当前 crate 的优势、短板、适合借鉴的架构模式。
3. 输出可执行的分阶段技术方案、验证标准、风险控制和交付顺序。
4. 为后续实现、任务拆分、基准回归、SIMD 优化和发布治理提供统一依据。

## 2. 适用范围

适用范围包括：

- `src/core.rs`
- `src/galois_8.rs`
- `src/galois_16.rs`
- `src/matrix.rs`
- `simd_c/reedsolomon.c`
- `build.rs`
- `benches/bandwidth.rs`
- 后续新增的 `docs/`、`benches/`、`tests/`、`src/*` 中与 EC 核心相关的实现

不直接包含：

- MinIO 的对象存储层、磁盘仲裁、位腐坏校验、元数据写入等存储系统逻辑
- 非 Reed-Solomon 的上层业务协议封装

## 3. 对标对象与参考依据

### 3.1 MinIO 路线

MinIO 的 EC 在工程上有两个层次：

1. 上层 `Erasure` 封装：
   - 提供 `EncodeData`
   - 提供 `DecodeDataBlocks`
   - 提供 `DecodeDataAndParityBlocks`
   - 提供 shard size / file size / offset 计算
   - 提供运行时自检
2. 底层编码器：
   - 使用 `github.com/klauspost/reedsolomon`

因此，对标 MinIO 的正确方式不是照搬上层对象存储逻辑，而是吸收其工程化设计思路，并重点对标 `klauspost/reedsolomon` 的编码器能力。

### 3.2 当前 crate

当前 `reed-solomon-erasure` 的定位是底层通用编码库，而不是完整存储系统中的 EC 子系统。

其优势在于：

- `Field` 抽象清晰
- `GF(2^8)` 与 `GF(2^16)` 后端统一
- `no_std` 友好
- reconstruction 逻辑集中且易审计
- 当前代码路径相对短，适合重构与性能治理

## 4. 深入差异分析

### 4.1 设计定位差异

MinIO 的 `Erasure` 更像“面向系统使用方”的包装层。

它负责：

- 懒初始化 encoder
- 根据 shard size 调整并发策略
- 提供数据切片与重建调用入口
- 执行自检

当前 crate 则直接暴露 Reed-Solomon 算法核心：

- `encode`
- `verify`
- `reconstruct`
- `reconstruct_data`
- `encode_single`
- `encode_sep`
- `ShardByShard`

这意味着：

- 当前 crate 需要参考 MinIO 的“外层能力设计”
- 但不应该把对象存储语义硬塞进库本体

### 4.2 矩阵策略差异

MinIO 底层依赖的 `klauspost/reedsolomon` 支持多种矩阵策略：

- Vandermonde
- Jerasure Matrix
- PAR1 Matrix
- Cauchy Matrix
- Custom Matrix
- fast one parity

当前 crate 目前使用固定 Vandermonde 逻辑构造编码矩阵。

当前实现优点：

- 行为简单
- 输出稳定
- 易于审计

当前实现限制：

- 无法根据场景切换矩阵构造
- 无法针对 `parity = 1` 做更强的特化
- 无法支持上层兼容性需求或 LRC 风格的扩展矩阵

### 4.3 重建能力差异

MinIO 依赖的底层库支持：

- `Reconstruct`
- `ReconstructData`
- `ReconstructSome`

当前 crate 仅支持：

- `reconstruct`
- `reconstruct_data`

缺少 `reconstruct_some` 带来的问题：

- 无法只恢复上层真正需要的 shard
- 数据读取路径只能在“全量重建”和“仅数据重建”之间二选一
- 某些场景会多做无意义的 parity 重建

### 4.4 并行调度差异

MinIO / klauspost 路线有显式的自动并发调优：

- `WithAutoGoroutines(shardSize)`
- `WithMaxGoroutines`
- `WithMinSplitSize`

当前 crate 在核心 encode/reconstruct 路径上仍然是串行遍历。

这会带来几个后果：

- 大 shard 下无法充分利用多核
- `verify` 和 `reconstruct` 受限于单线程吞吐
- SIMD 路径即便优化，也会受制于上层串行调度

### 4.5 SIMD 策略差异

MinIO 底层库的优势是：

- 运行时 CPU 特性探测
- 动态选择 SSE2 / SSSE3 / AVX2 / AVX512 / GFNI / NEON / SVE
- ISA 选择与业务逻辑解耦

当前 crate 的现状：

- `simd-accel` 为编译期开关
- `build.rs` 默认偏向 `-march=haswell`
- SIMD 内核主要来自 `simd_c/reedsolomon.c`
- 使用方式接近“构建时绑定某类 CPU 路径”

主要短板：

- 不适合通用二进制分发
- 很难在单个发行包中同时覆盖不同 CPU
- 无统一的 runtime dispatch 层
- 缺少 GFNI 这类现代 ISA 的显式策略层

### 4.5.1 SIMD 实现语言路线差异

当前 crate 的 SIMD 主要依赖 `simd_c/reedsolomon.c`，属于“C 内核 + Rust 封装”的方式。

MinIO 所依赖的底层 `klauspost/reedsolomon` 则在工程上更接近“库本身统一管理 ISA 能力与路径选择”的模型，虽然具体实现并不等价于当前 crate，但其设计目标非常明确：

- 运行时根据 CPU 能力选路
- 不把 SIMD 选择权完全锁死在编译阶段
- 上层编码器不直接感知具体 ISA 后端

从长期演进角度看，当前 crate 如果继续完全依赖 C 路线，会面临以下问题：

1. 构建与交叉编译复杂度持续上升
2. 不同 ISA 路径的调试与验证成本高
3. 与 Rust 核心逻辑的边界较硬，不利于统一调度与后端治理
4. 后续接入更多现代 ISA 时，维护成本会持续增长

但如果现在立刻全量迁移到 Rust SIMD，也有明显风险：

1. 一次性重写范围过大
2. 高复杂度路径如 GFNI、AVX512、NEON 容易先丢性能
3. 在没有完成 runtime dispatch 与 benchmark 基线前，难以证明重写收益

因此，本方案明确采用以下 SIMD 路线决策：

1. 短期：继续保留现有 C SIMD 内核
2. 中期：将 C 内核降级为一个 backend，而不是唯一实现
3. 中长期：逐步引入 Rust `std::arch` 后端，并以 Rust 为主实现新架构
4. 长期目标：Rust 成为主实现，C 成为过渡 fallback 或可选 legacy 后端

结论上，这不是“继续用 C”与“立即全 Rust 重写”的二选一，而是：

- 架构方向：Rust 主导
- 迁移节奏：保留 C，渐进替换
- 验证方式：双实现并存，对照 benchmark 与 correctness 后再逐步退役 C

### 4.6 自检与防回归差异

MinIO 在启动阶段执行 EC 自检，这点工程价值很高。

价值体现在：

- 捕捉 ISA 分支错误
- 捕捉编译器/平台差异引发的隐性错误
- 捕捉 golden 输出漂移

当前 crate 测试覆盖很多，但主要集中在：

- 单元测试
- quickcheck
- bench

缺少：

- 运行时自检入口
- 固定 golden vector 集
- 跨 ISA 一致性回归用例

## 5. 当前 crate 是否适合参考 MinIO 设计架构

结论：非常适合参考，但应以“能力借鉴”而非“结构复制”为原则。

推荐直接借鉴的能力：

1. Option/Builder 配置模式
2. 自动并发调度
3. 细粒度 reconstruction API
4. runtime ISA dispatch
5. 自检与 golden 回归
6. inversion cache 策略开关
7. 高性能基准体系

不建议直接照搬的内容：

1. 对象存储层语义
2. 磁盘与 quorum 相关逻辑
3. 直接把系统层封装混进算法 crate
4. 在没有基准前贸然引入 Leopard

## 6. 总体改进原则

整个演进过程遵循以下原则：

1. 先建立性能与正确性基线，再做架构改造。
2. 先做低风险高收益项，再做高复杂度 SIMD 与大规模后端重构。
3. 保持 `no_std` 能力不退化。
4. `std` 能力下可以逐步引入并行和调度增强。
5. 所有优化必须有明确 benchmark 结果和一致性验证。
6. 每个阶段结束都要具备单独可合并、可回退、可量化的交付物。

## 7. 分阶段执行总览

### 阶段 1：基线、度量与回归框架

目标：

- 建立统一 benchmark 口径
- 补充 correctness baseline
- 为后续改造建立量化标准

核心成果：

- benchmark 结果导出
- 常见配置压测矩阵
- golden vector baseline

### 阶段 2：公共 API 与编码器配置能力补齐

目标：

- 对标 `klauspost/reedsolomon` 的高价值 API
- 引入可扩展配置模式

核心成果：

- `CodecOptions`
- `reconstruct_some`
- `split` / `join`
- `fast_one_parity`
- inversion cache 开关

### 阶段 3：并行调度核心改造

目标：

- 对齐 MinIO/klauspost 的自动并发设计思路
- 让 encode/verify/reconstruct 真正能吃满多核

核心成果：

- `with_auto_threads(shard_size)`
- chunk scheduler
- `std` 下统一并行执行层

### 阶段 4：SIMD 架构升级

目标：

- 从构建期 ISA 绑定升级到运行时分发
- 形成稳定的 SIMD 后端架构

核心成果：

- runtime dispatch
- 分 ISA 后端抽象
- GFNI / NEON / AVX2 / AVX512 优化路径
- C backend 与 Rust backend 的并存治理
- Rust 主导的 SIMD 迁移路线

### 阶段 5：重建与缓存深度优化

目标：

- 进一步优化 reconstruction 成本
- 提升 cache 有效性与可观测性

核心成果：

- cache metrics
- `reconstruct_data` 专项优化
- required shards only 路径

### 阶段 6：自检、发布治理与长期演进机制

目标：

- 让优化结果长期稳定
- 防止 SIMD 与平台差异引入 silent corruption

核心成果：

- self-test
- benchmark gate
- regression gate
- 跨平台一致性验证

## 8. 优先级建议

建议严格按以下优先级推进：

1. 阶段 1
2. 阶段 2
3. 阶段 3
4. 阶段 4
5. 阶段 5
6. 阶段 6

理由：

- 没有基线，后面所有“优化”都无法判断价值
- 没有 API 与选项层，后面很多优化无法优雅暴露
- 没有并行调度，SIMD 的收益会被上层串行瓶颈吞掉

## 8.1 SIMD 路线决策

本项目后续 SIMD 演进采用以下固定决策，避免执行中反复摇摆：

1. 不继续把 C 作为长期唯一 SIMD 实现。
2. 不在当前阶段直接一次性全量改写为 Rust SIMD。
3. 先做 backend 抽象与 runtime dispatch。
4. 先保留 C 后端作为稳定基线。
5. 再逐步新增 Rust SIMD 后端。
6. 当 Rust 后端在性能、正确性、可维护性三方面都稳定后，逐步退役 C 后端。

此决策的核心含义是：

- 当前 C SIMD 是性能资产，不应急于删除
- 最终 Rust SIMD 是架构目标，不应长期缺席
- 所有迁移必须建立在 benchmark 与一致性验证上

## 9. 实施过程中必须重点关注的风险

### 9.1 行为兼容性风险

- 不同矩阵模式可能改变输出
- `fast_one_parity` 可能影响与现有编码数据兼容性
- 运行时 dispatch 错误会导致 silent corruption

### 9.2 性能回退风险

- 小 shard 并行化可能变慢
- 过度抽象可能影响热路径内联
- cache 机制可能带来额外锁竞争

### 9.3 维护复杂度风险

- 多 ISA 后端会显著增加测试矩阵
- 若过早引入 Leopard，会把复杂度拉高过快

## 10. 最终目标状态

当全部阶段完成后，期望该 crate 具备以下能力：

- 提供完整的通用 Reed-Solomon API
- 在 `std` 场景下具备自动并行调度能力
- 在主流 CPU 上具备 runtime SIMD 最优选路
- 具备 reconstruction 细粒度能力
- 具备高质量 benchmark、回归、自检与发布治理体系
- 继续保持当前 crate 的透明性与可审计性

## 11. 文档索引

本方案的子任务文档如下：

- `docs/ec-improvement-task-board.md`
- `docs/ec-phase-1-baseline-and-regression.md`
- `docs/ec-phase-2-api-and-config.md`
- `docs/ec-phase-3-parallel-scheduler.md`
- `docs/ec-phase-4-simd-runtime-dispatch.md`
- `docs/ec-phase-5-reconstruction-and-cache.md`
- `docs/ec-phase-6-selftest-release-governance.md`
