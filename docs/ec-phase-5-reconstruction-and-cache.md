# 阶段 5：重建与缓存深度优化

## 1. 阶段目标

在已有 API、并行和 SIMD 架构基础上，进一步优化 reconstruction 的实际成本，尤其是：

- 重建矩阵缓存命中率
- data-only 恢复路径
- 部分 shard 恢复
- 重建写回成本

## 2. 当前基础

当前 crate 已经有：

- `reconstruct`
- `reconstruct_data`
- `data_decode_matrix_cache`

这是很好的基础，但还缺：

- cache 可观测性
- cache 开关与容量治理
- required-only 的更细粒度恢复
- reconstruction 热点 benchmark

## 3. 交付物

1. cache metrics
2. reconstruction benchmark 分层
3. `reconstruct_data` 专项优化
4. 与 `reconstruct_some` 协同优化

## 4. 任务拆解

### 任务 1：增加 cache 可观测性

建议指标：

- requests
- hits
- misses
- evictions

暴露方式：

- `std` 下调试接口
- feature gated 统计

### 任务 2：优化 cache key 和容量策略

当前 key 为 `invalid_indices`。

需要评估：

- 是否足够高频复用
- 是否需要容量按 shard 数量动态调整
- 是否需要根据 workload 提供不同默认值

### 任务 3：`reconstruct_data` 专项优化

目标：

- 只恢复数据时，不做无意义 parity 处理
- 减少输出写回
- 与并行调度结合

### 任务 4：`reconstruct_some` 联动优化

目标：

- 当上层只需要部分数据 shard 时，只恢复被要求的 shard

### 任务 5：重建热点基准

至少区分：

- 缺 1 个 data shard
- 缺多个 data shard
- data + parity 混合缺失
- 重复缺失模式命中 cache
- 不重复模式压测 cache

## 5. 验收标准

1. cache 命中率可观测
2. 常见重复缺失模式下性能优于当前版本
3. `reconstruct_data` 路径 benchmark 有可见收益

## 6. 风险点

- 统计本身引入额外开销
- cache 策略过度复杂影响维护

## 7. 风险应对

- 指标统计可 feature gate
- 默认保持简单策略，复杂策略后置

## 8. 完成后的收益

- reconstruction 成本更可控
- 上层读路径可以避免多余恢复

## 9. 当前落地状态（2026-05-24）

已完成：

- [x] `reconstruct` / `reconstruct_data` / `reconstruct_some` 已存在
- [x] `inversion_cache` 与 `inversion_cache_capacity` 已接入 `CodecOptions`
- [x] cache metrics 已提供基础可观测项（`requests/hits/misses/inserts`）
- [x] reconstruct_some 已有“required-only”恢复语义与测试

未完成 / 差距：

- [x] 指标已覆盖 `evictions`
- [x] cache 命中率分析已形成统一口径（`hit_rate`/`reuse_ratio`/`miss_cost_per_request`）并写入方法学文档
- [x] 默认容量已改为按 workload 自动推导（基于 `data_shards + parity_shards` 与 parity 扇出估算，并做上下界裁剪）
- [x] 重建热点 benchmark 已可沉淀为稳定 gate 场景：
  - `scripts/check_reconstruction_hotspot_gate.py` 可对比 `reconstruction-hotspot-results.json`
  - `scripts/release-check.sh` 已支持通过 `RUN_RECONSTRUCTION_HOTSPOT_GATE=1` 接入发布前回归
  - gate 默认关注“场景覆盖 + 相对 baseline 的稳定回归”，不强行假设所有 hotspot candidate 在所有 ISA 上都绝对快于 baseline

## 10. 执行待办（按优先级）

### P0（先补齐可观测与策略闭环）

- [x] 为 cache metrics 增加 `evictions` 计数
- [x] 增加统一命中率计算辅助方法（命中率、复用率、单位请求开销）
- [x] 输出统一格式结果（JSON/CSV），与 `target/benchmark-smoke` 对齐

### P1（性能专项）

- [x] 评估并落地容量策略：
  - 显式容量 `> 0` 时按调用方配置生效
  - `0` 代表启用自动策略
  - 自动策略按 `total_shards * parity_shards * 2` 估算，并裁剪到 `128..=4096`
- [x] 已为 `reconstruct_data` 与 `reconstruct_some` 增加更直接的性能对照基准：
  - 缺 1 data
  - 缺多个 data
  - data+parity 混合
  - 重复缺失模式/非重复缺失模式
  - 当前已补 `reconstruction-hotspot-results.{json,csv}` 输出，覆盖：
    - `reconstruct` vs `reconstruct_data`
    - `reconstruct_data` vs `reconstruct_some`
    - 缺 1 data / 缺多个 data / data+parity 混合 / 32x16 大规模场景
  - 当前剩余缺口：是否将这些热点场景进一步提升为稳定 gate，而不是基准本身缺失

### P2（治理增强）

- [x] 将 cache 分析方法写入文档（输入、样本规模、统计口径，见 `docs/benchmark-methodology.md`）
- [x] 已增加可选 feature gate：`benchmark-metrics`，允许在 release 配置中关闭重统计开销

## 11. 建议 PR 拆分

1. `phase5-cache-metrics`: 补 `evictions` + 统计接口完善
2. `phase5-cache-policy`: 容量策略实验与默认值调整
3. `phase5-reconstruct-bench`: 热点场景基准矩阵 + 输出规范

## 12. 验收命令

```bash
cargo check --tests
cargo test --features std test_reconstruct_some_recovers_only_required_data_shard
cargo test --features std benchmark_parallel_helpers_quantify_gain
cargo test --features std benchmark_reconstruction_cache_patterns
cargo test --features std benchmark_reconstruction_cache_stats
cargo test --features std benchmark_reconstruction_hotspots
```

## 13. reconstruct_data small-output data-stage A/B（2026-05-26）

本轮继续只围绕 `reconstruct_data` 的 data-stage 热点推进，目标是不再复用 shared
`code_some_small_output_chunk_parallel` 路径，而是在 `missing_data <= 2` 场景下走更窄的专用并行路径。

### 13.1 已落地实现

当前代码状态：

1. 当 `reconstruct_data_opt` 命中 `missing_data == 1` 或 `missing_data == 2` 时，data-stage 走专用路径。
2. 专用路径仍保持并行，但不再记入 `code_some_small_output_chunk_parallel_calls`。
3. 针对 `missing_data == 2` 的双输出场景，仅在 `data_shard_count <= 16` 时把 chunk 下限提高到 `512 KiB`。
4. `data_shard_count > 16` 的双输出场景保留默认 chunk 粒度，避免拖累更大规模矩阵。

对应测试：

1. `test_reconstruct_data_one_missing_skips_small_output_chunk_parallel_path`
2. `test_reconstruct_data_two_missing_skips_small_output_chunk_parallel_path`

### 13.2 本轮 profile 结论

基于：

```bash
RSE_BACKEND_OVERRIDE=rust-avx2 \
RSE_WRITE_PROFILE_REPORT=1 \
RSE_PROFILE_REPORT_PATH=/tmp/throughput-avx2-profile-reconstruct-data-two-output-512k-smallonly-v5.json \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

关键观察：

1. `reconstruct_data 10x4_1m` 的 `code_some_small_output_chunk_parallel_calls = 0`
2. `reconstruct_data 32x16_1m` 的 `code_some_small_output_chunk_parallel_calls = 0`
3. 说明 `missing_data == 1/2` 的 data-stage 已经完全绕开 shared small-output chunk 并行分支

### 13.3 reconstruction hotspot 结果

基于：

```bash
cargo test --release --features "std simd-accel" benchmark_reconstruction_hotspots -- --nocapture
```

关键场景结果：

1. `reconstruct_data_missing_1_data`: `1.0868x`
2. `reconstruct_data_missing_2_data`: `1.0962x`
3. `reconstruct_data_32x16_missing_2_data`: `1.0028x`

结论：

1. `missing_data == 1` 与 `missing_data == 2` 的 10x4 热点都已转为正收益
2. 32x16 的双输出热点至少保持不退化

### 13.4 throughput_matrix 结果摘要

同机、同参数下，本轮结果呈现：

1. `reconstruct_data_10x4_1m`：基本持平到小幅正向
2. `reconstruct_10x4_1m`：正向
3. `reconstruct_data_32x16_1m`：基本持平
4. `reconstruct_32x16_1m`：正向
5. `encode/verify` 未出现结构性回退

### 13.5 当前结论

这轮实现比前面的全局调度试验更稳，原因是：

1. 优化范围严格限制在 `reconstruct_data` data-stage
2. 仅对 `missing_data <= 2` 的少输出热点场景生效
3. 仅在 `data_shard_count <= 16` 时扩大双输出 chunk 粒度，避免把 32x16 这类大矩阵一起拖进实验路径

当前建议：

1. 可以把这版视为阶段 5 中 `reconstruct_data` 专项优化的有效增量
2. 后续若继续深化，应优先扩展更多 `missing_data <= 2` 的 data-stage 专用实现，而不是重新回到 shared `code_some` 路径上打转
