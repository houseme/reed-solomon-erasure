# 阶段 3：并行调度核心改造

## 1. 阶段目标

让当前 crate 在 `std` 环境下具备对标 MinIO / klauspost 的自动并发调度能力，使 `encode`、`verify`、`reconstruct`、`reconstruct_data` 能在大 shard 场景下充分利用多核。

## 2. 设计原则

1. `no_std` 下保持串行实现。
2. `std` 下开启可选并行执行层。
3. 小 shard 禁止过度并行。
4. 并行策略必须与 benchmark 数据绑定。

## 3. 交付物

1. chunk scheduler
2. 自动线程数推导
3. encode 并行实现
4. verify 并行实现
5. reconstruct 并行实现

## 4. 核心思路

当前 `code_some_slices` 的结构是：

- 外层遍历 data shard
- 内层遍历 output shard

这会导致：

- 大块数据下无法并发
- SIMD 与 CPU cache 优势无法完全释放

建议改造方向：

- 以 shard byte range 为 chunk 切分单位
- 每个任务处理一段 `[start..end)` 字节区间
- 对所有输入与输出的这一段做编码

这样有三个好处：

1. 适合多线程分发
2. 适合 SIMD 后端重用
3. 更接近 cache-friendly 模式

## 5. 任务拆解

### 任务 1：抽离 chunk 级编码函数

建议新增：

- `code_some_slices_chunked`
- `code_single_slice_range`

要求：

- 串行和并行都复用同一分块逻辑
- 不复制输入数据

### 任务 2：设计线程数自动推导

建议参数：

- `shard_size`
- `data_shards`
- `parity_shards`
- `available_parallelism`

输出：

- 线程数
- chunk size

原则：

- 小于阈值不并行
- 大于阈值按 shard size 和 CPU 数量估算

### 任务 3：引入并行执行后端

实现方案可以二选一：

1. `rayon`
2. 手写线程池/分发器

建议：

- 若目标是快速落地与维护成本低，优先 `rayon`
- 若非常关注依赖和 `no_std` 边界，则自建 `std` only scheduler

### 任务 4：encode 并行化

重点优化：

- `encode`
- `encode_sep`
- `encode_single`
- `encode_single_sep`

建议先做：

- `encode`
- `encode_sep`

### 任务 5：verify 并行化

思路：

- parity buffer 计算按 chunk 并行
- 最后归并校验

### 任务 6：reconstruct 并行化

思路：

- 矩阵计算部分保持串行
- shard 数据恢复部分并行化

优先级：

- 先并行化 data shard 恢复
- 再并行化 parity shard 回填

## 6. 验收标准

1. `std` 下 benchmark 明显提升
2. 小 shard 场景退化不超过 10%
3. `no_std` 行为不变
4. 所有原有测试通过

## 7. 风险点

- 锁与调度开销吞噬收益
- chunk 过细导致调度过重
- chunk 过粗导致负载不均

## 8. 风险应对

- 先做基于 benchmark 的固定阈值
- 后续再引入更动态的 auto tuning

## 9. 完成后的收益

- 这是最有可能带来整体吞吐大幅提升的阶段
- 同时为阶段 4 的 SIMD 深化铺路

## 10. 当前落地状态（2026-05-24）

已完成：

- [x] chunk 级串行路径已落地（`code_some_slices_chunked`、`code_single_slice_range`）
- [x] `std` 下并行 encode/verify 路径已落地（`encode_sep_par`、`verify_with_buffer_par`）
- [x] `galois_8` 下并行 reconstruct 入口已落地（`reconstruct_opt`、`reconstruct_data_opt`）
- [x] 基础分块规则 `code_chunk_len` 已落地并有测试

未完成 / 差距：

- [x] 线程数自动推导策略已形成独立策略层（`ParallelPolicy` / `ParallelDecision`）
- [x] 并行入口已接入 `galois_8` 的 `*_opt` 自动选择路径
- [x] 并行策略与 benchmark 结果的参数联动机制已固化（输出 `policy_version` 与关键阈值字段）

## 11. 执行待办（按优先级）

### P0（先做，确保阶段可收口）

- [x] 新增并行策略结构体（`ParallelPolicy`）：
  - 文件：`src/core.rs`、`src/galois_8.rs`
  - 目标：明确 `min_parallel_shard_bytes`、`max_jobs`、`chunk_len_policy`
- [x] 实现自动线程/任务数推导函数：
  - 输入：`shard_size`、`data_shards`、`parity_shards`、`available_parallelism`
  - 输出：`jobs`、`chunk_len`
- [x] 将自动策略接入 `encode_opt` / `verify_opt` / `reconstruct_opt` 路径，并保留串行回退
- [x] 补充策略正确性测试（小 shard 不并行、大 shard 并行）

### P1（性能与可维护性）

- [x] 统一并行路径内部调度逻辑，减少 encode/verify/reconstruct 的重复并行样板代码
- [x] 对 `reconstruct_opt` 增加“data-only 与 full reconstruct”两档并行差异策略
- [x] 为 `code_chunk_len` 增加可覆盖更多规模的参数化测试（尤其极大 shard）

### P2（增强项）

- [x] 增加可选的并行策略调试输出（仅 `std` + debug / feature gate）
- [x] 在文档中记录不同机器建议阈值

建议阈值（首版经验值）：

- 4 核机器：`min_parallel_shard_bytes = 256KiB`，`min_bytes_per_job = 256KiB`
- 8 核机器：`min_parallel_shard_bytes = 256KiB`，`min_bytes_per_job = 256KiB`
- 16 核及以上：`min_parallel_shard_bytes = 128KiB`，`min_bytes_per_job = 256KiB`

说明：

- `reconstruct_data_opt` 当前默认使用更保守的 data-only 阈值（`512KiB`）
- `reconstruct_opt` 当前默认使用更激进的 full-reconstruct 阈值（`256KiB`）

## 12. 建议 PR 拆分

1. `phase3-policy-core`: `ParallelPolicy` + 自动推导函数 + 单元测试
2. `phase3-policy-integration`: 接入 `encode_opt`/`verify_opt`/`reconstruct_opt`
3. `phase3-bench-tuning`: 基准数据补齐 + 阈值调优 + 文档回填

## 13. 验收命令

```bash
cargo check --tests
cargo test --features std test_encode_sep_par_matches_encode_sep
cargo test --features std test_verify_with_buffer_par_matches_verify_with_buffer
cargo test --features std test_galois_8_reconstruct_data_opt_matches_reconstruct_data
cargo test --features std test_galois_8_reconstruct_opt_matches_reconstruct
cargo bench --bench throughput_matrix
```
