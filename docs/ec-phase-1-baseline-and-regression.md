# 阶段 1：基线、度量与回归框架

## 1. 阶段目标

建立本项目后续所有优化的统一度量体系，避免“做了大量优化但无法证明收益”的情况。

本阶段重点不是提升性能，而是建立：

- benchmark 基线
- correctness baseline
- 性能回归口径
- 黄金向量

## 2. 交付物

必须交付：

1. benchmark 组合矩阵
2. benchmark 结果导出能力
3. golden vector correctness 测试
4. benchmark/验证运行说明

建议新增文件：

- `benches/throughput_matrix.rs`
- `benches/common.rs`
- `tests/golden_vectors.rs`
- `benches/results/` 的本地输出约定
- `docs/benchmark-methodology.md` 或整合到现有文档

## 2.1 推荐目录结构

建议按以下方式组织阶段 1 产物：

```text
benches/
  bandwidth.rs
  throughput_matrix.rs
  common.rs
tests/
  golden_vectors.rs
```

说明：

- `common.rs` 负责固定 seed、数据生成、结果序列化、case 定义
- `throughput_matrix.rs` 负责统一基准矩阵
- `golden_vectors.rs` 负责固定输入和 hash 校验

## 3. 任务拆解

### 任务 1：建立统一 benchmark 维度

至少覆盖：

- `encode`
- `verify`
- `reconstruct`
- `reconstruct_data`

每个维度测试以下 shard 组合：

- `4+2`
- `8+4`
- `10+4`
- `16+8`
- `32+16`
- `64+32`

每个组合覆盖以下 shard size：

- `64KiB`
- `1MiB`
- `4MiB`

建议命名规则：

- `encode_10x4_1m`
- `verify_10x4_1m`
- `reconstruct_all_10x4_1m`
- `reconstruct_data_10x4_1m`

命名原则：

1. 包含 operation
2. 包含 data/parity
3. 包含 shard size
4. 保持字符串稳定，便于历史对比

建议引入统一 case 结构：

```rust
struct BenchCase {
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
    label: &'static str,
}
```

所有基准通过静态 case 列表统一驱动，避免手写重复基准代码。

### 任务 2：建立固定数据集策略

要求：

- bench 中使用固定 seed 或可复现 seed
- 同一 benchmark 在不同提交之间输入可复现
- 避免随机数据导致高波动

建议：

- benchmark 使用固定 `u64` seed
- 把 seed 记录到 benchmark 输出元信息中

进一步要求：

- 所有 benchmark case 使用相同 seed 派生策略
- 数据生成逻辑必须集中到 `benches/common.rs`
- 禁止每个 benchmark 文件自己生成随机策略

建议固定：

- `BASE_SEED: u64 = 0xEC5EED_20260524`

派生方式：

- `derived_seed = hash(operation, data_shards, parity_shards, shard_size, BASE_SEED)`

这样可以保证：

- 同一 case 可重复
- 不同 case 输入不同
- benchmark 输入变化是可追踪的

### 任务 3：结果导出

要求：

- benchmark 不只输出控制台结果
- 结果可对比、可归档、可脚本分析

建议输出：

- `JSON`
- `CSV`

关键字段：

- timestamp
- git revision
- profile
- feature flags
- target triple
- shard config
- operation kind
- throughput
- latency

建议 JSON schema：

```json
{
  "timestamp": "...",
  "git_revision": "...",
  "target_triple": "...",
  "profile": "bench",
  "features": ["std", "simd-accel"],
  "backend": "scalar|simd-c|runtime-dispatch",
  "operation": "encode",
  "data_shards": 10,
  "parity_shards": 4,
  "shard_size": 1048576,
  "seed": 123,
  "throughput_mb_s": 0.0,
  "ns_per_iter": 0.0
}
```

建议 CSV 字段顺序：

```text
timestamp,git_revision,target_triple,profile,features,backend,operation,data_shards,parity_shards,shard_size,seed,throughput_mb_s,ns_per_iter
```

建议输出文件命名：

- `bench-<gitsha>-<timestamp>.json`
- `bench-<gitsha>-<timestamp>.csv`

### 任务 4：correctness golden vectors

目标：

- 固定输入
- 固定 shard 组合
- 固定输出 hash

建议覆盖：

- `4+2`
- `8+4`
- `10+4`

每组验证：

- `encode` 输出 hash
- 删除若干 data shard 后 `reconstruct_data`
- 删除若干混合 shard 后 `reconstruct`
- `verify` 正确与错误输入的结果

建议固定输入集：

1. 递增字节序列
2. 固定 seed 伪随机序列
3. 全零输入
4. 重复模式输入

建议固定 hash 算法：

- `blake3`
- 或 `xxhash64`

注意：

- hash 只用于 golden 校验，不用于证明跨实现编码字节级兼容
- 一旦编码矩阵策略未来允许切换，不同 matrix mode 需要维护独立 golden 集

建议 golden case 命名：

- `golden_encode_4x2_inc`
- `golden_reconstruct_data_8x4_seeded`
- `golden_reconstruct_10x4_pattern`

### 任务 5：跨实现对照基线

如果条件允许，加入与下列实现的离线对照：

- `klauspost/reedsolomon`
- 当前 crate 自身旧版本

目标不是要保持完全一致的矩阵输出，而是确认：

- reconstruction 行为正确
- 输入/输出契约稳定
- 常见场景结果可互证

更具体地说，对照应分成两类：

1. 行为对照：
   - reconstruct 后恢复出的原始数据是否一致
   - verify 对损坏输入是否能稳定拒绝
2. 性能对照：
   - encode/reconstruct_data/reconstruct 的吞吐对比

不建议一开始强求：

- 与 `klauspost/reedsolomon` 的 parity bytes 完全一致

因为若未来 matrix mode 不同，编码输出可能本就不要求字节级兼容。

## 3.1 benchmark 文件设计建议

### `benches/common.rs`

建议职责：

- case 定义
- 数据生成
- 元信息收集
- 输出结果序列化
- 公共 helper

### `benches/throughput_matrix.rs`

建议职责：

- 注册所有标准基准
- 区分 `smoke` 与 `full`
- 将 operation 统一纳入矩阵执行

### `tests/golden_vectors.rs`

建议职责：

- 固定输入与 golden hash
- reconstruction 回归
- verify 行为回归

## 3.2 smoke/full 两档矩阵建议

### smoke

用于本地快速回归，建议包含：

- `4+2, 64KiB`
- `10+4, 1MiB`
- `32+16, 1MiB`

operation：

- `encode`
- `reconstruct_data`
- `reconstruct`

### full

用于阶段性评估，包含完整矩阵：

- 6 组 shard 组合
- 3 组 shard size
- 4 种 operation

## 3.3 benchmark 结果判读规则

建议统一采用：

1. 同机对比
2. 中位数优先
3. 观察 throughput 与 latency 两个维度
4. 对小 shard 与大 shard 分别判断

避免错误结论：

- 只看单次结果
- 只看一种 shard 组合
- 只看 encode 不看 reconstruct

## 4. 验收标准

必须满足：

1. benchmark 结果可稳定复现
2. golden vector 测试可在本地重复通过
3. benchmark 至少覆盖 `encode` 与两类 reconstruction
4. 输出结果可用于后续 PR 回归对比
5. `smoke` 与 `full` 两档矩阵均有明确说明
6. benchmark 文件结构足够稳定，便于后续自动化

## 5. 风险点

- criterion 结果波动较大
- quickcheck 与 benchmark 混合时可能拉长 CI 时间
- 过大的 benchmark 矩阵会拖慢日常开发

## 6. 风险应对

- 提供 `full` 和 `smoke` 两档 benchmark
- correctness 与 benchmark 分离
- benchmark 文件输出走本地目录，不纳入版本控制

额外建议：

- benchmark 不与 quickcheck 共享运行入口
- 结果导出失败不应影响 correctness tests

## 7. 完成后的收益

一旦本阶段完成，后续每次并行化、SIMD 化、缓存优化，都能明确回答：

- 是否更快
- 哪类 shard 组合更快
- 哪类组合退化了
- 正确性有没有漂移

并且还能回答：

- 某条 SIMD 路径是否真的有收益
- 某个并行阈值是否合理
- 某项 reconstruction 优化是否只对特定 workload 生效
