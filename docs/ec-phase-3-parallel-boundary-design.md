# 阶段 3 并行边界设计说明

## 1. 目的

本文档用于明确阶段 3 的并行化为什么不能直接塞进当前 `ReedSolomon<F>` 的公共泛型热路径，以及下一步应该如何安全推进。

## 2. 当前问题

在当前架构下，若直接把 `rayon` 或其他并行执行层塞进以下路径：

- `encode`
- `encode_sep`
- `verify`
- `verify_with_buffer`
- `code_some_slices`

会立即触发两个问题：

1. `Send/Sync` 约束沿公共 API 扩散
2. `Field::Elem` 的并发约束会被提升为整个泛型设计的一部分

这会把阶段 3 从“并行调度优化”变成“公共 API 和 trait 边界重构”。

## 3. 为什么现在不该这么做

原因：

1. 当前 crate 的公开接口并没有承诺 `Send/Sync`
2. `GF(2^8)` 是最主要性能目标，`GF(2^16)` 和其他潜在 field 并不一定需要立即被并发约束
3. 并发优化的 ROI 首先集中在 `u8` 热路径，而不是整个泛型抽象
4. 过早把并发约束抬到公共 API，会抬高所有后续实现成本

## 4. 推荐并行边界

### 方案核心

并行执行层不直接挂在 `ReedSolomon<F>` 的公开泛型方法签名上，而是先落在：

1. `std` 环境
2. `galois_8`
3. `u8` 主路径

更具体地说，下一步建议：

- 保持当前 `ReedSolomon<F>` 公共 API 不变
- 在内部为 `Field = galois_8::Field` 增加受限并行入口
- 并行入口只处理 `&[u8]` / `&mut [u8]` 这类可明确验证 `Send/Sync` 的路径

## 5. 分层建议

### 层 1：通用串行核心

保留：

- `code_some_slices_chunked`
- `code_single_slice_range`
- `code_chunk_len`

职责：

- 所有后端共同依赖
- 无并发约束
- `no_std` 下继续可用

### 层 2：std 专用并行调度层

新增：

- `std` only 内部模块
- 明确要求 `u8` shards
- 可以引入 `rayon` 或手写线程池

职责：

- 不暴露给通用泛型 API
- 只在可明确证明 `Send/Sync` 的路径中使用

### 层 3：galois_8 专用高性能入口

后续可考虑：

- `galois_8::ReedSolomon` 专用 encode helper
- `encode_sep_u8_parallel(...)`
- `verify_u8_parallel(...)`

注意：

- 这一步是内部加速策略，不一定需要立即做成新的公开 API

## 6. 下一步推荐技术路线

### 第一步

先保留现在已经完成的：

- chunk 化基础层
- 自动 chunk 大小策略

### 第二步

设计一个 `std` only 的内部函数，例如：

```rust
#[cfg(feature = "std")]
fn code_some_slices_parallel_u8(...)
```

这个函数只服务：

- `u8`
- `galois_8`
- 明确的 `Vec<Vec<u8>>` / `&mut [&mut [u8]]` 风格路径

### 第三步

只让 `galois_8` 的 encode/verify 热路径在满足条件时走这个并行函数。

### 第四步

在 benchmark 里验证：

- `10+4, 1MiB`
- `32+16, 1MiB`
- `64+32, 4MiB`

## 7. 这样做的收益

1. 不污染公共泛型 API
2. 不强迫所有 `Field::Elem` 满足并发约束
3. 把复杂性收敛在最有价值的主路径
4. 后续 SIMD 和并行可以更好地组合

## 8. 风险与控制

风险：

- 会出现“泛型串行核心 + u8 专用并行层”的双层结构

控制：

- 这是有意的工程分层，不是坏味道
- 先把性能收益做出来，再决定是否继续向泛型层抽象

## 9. 当前结论

阶段 3 后续不应继续尝试把并行约束直接压进 `ReedSolomon<F>` 的公共泛型方法签名。

正确方向是：

1. 保留 chunk 化核心
2. 在 `std` 下新增受限并行边界
3. 从 `galois_8` / `u8` 主路径先吃到并发收益
4. 用 benchmark 决定后续是否扩大并行覆盖面

