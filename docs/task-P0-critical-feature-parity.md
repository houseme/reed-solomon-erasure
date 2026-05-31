# P0 — 关键功能对等性任务

> 优先级：最高 | 阻塞后续所有 Leopard 相关工作
> 预估总工作量：4-6 周

---

## 目录

- [P0-1: Leopard GF8 完整编解码](#p0-1-leopard-gf8-完整编解码)
- [P0-2: 流式 API (Read/Write)](#p0-2-流式-api-readwrite)

---

## P0-1: Leopard GF8 完整编解码

### 概述

当前 Leopard GF8 仅实现了编码路径，解码/重建/验证全部返回 `UnsupportedLeopardPrototype`。本任务将其补全至与 `klauspost/reedsolomon` 功能对等。

### 当前状态

| 操作 | 状态 | 位置 |
|------|------|------|
| 编码 (`encode_sep`) | ❌ 被 guard 拦截 | `encode.rs:359` 返回 `UnsupportedLeopardPrototype` |
| 编码引擎 | ✅ 已实现 | `leopard_gf8/encode.rs` (714 行) |
| 解码/重建 | ❌ 未实现 | `reconstruct.rs:423` 返回 `UnsupportedLeopardPrototype` |
| 验证 | ❌ 未实现 | `verify.rs:52` 返回 `UnsupportedLeopardPrototype` |

### 关键发现

1. **编码引擎已完成但未接入公共 API**: `leopard_gf8::encode_with_tables()` 完整实现，但 `encode_sep()` 在到达 FFT 逻辑前就返回错误
2. **Guard 机制分散**: 13 个方法各自检查 codec family，无集中式 dispatch
3. **FFT/IFFT 表已就绪**: `LeopardGf8Tables` 包含 `fft_skew`, `log_walsh`, `log_lut`, `exp_lut`, `mul_luts`
4. **SIMD 加速的 GF 乘法已就绪**: `ops.rs` 中的 `lut_xor` 支持 AVX2/SSSE3/NEON/Scalar

### 子任务拆分

#### P0-1a: 接入 Leopard GF8 编码到公共 API

**目标**: 让 `encode_sep` / `encode` 在 `LeopardGF8` codec family 下真正执行 FFT 编码

**修改文件**:
- `src/core/encode.rs` — 移除 `encode_sep` (line 359) 的 `UnsupportedLeopardPrototype` guard，替换为调用 `leopard_gf8::encode_with_tables()`
- `src/core/encode.rs` — 同样修改 `encode_sep_par` (line 456)
- `src/core/encode.rs` — 处理 `encode_single` / `encode_single_sep` 的 leopard 路径 (line 302, 323)

**实现要点**:
```rust
// encode.rs — encode_sep 中的 leopard 分支
if let FamilyState::LeopardGF8(ref leopard) = self.family_state {
    leopard::leopard_gf8::encode_with_tables(
        data, parity, self.data_shard_count, self.parity_shard_count,
        leopard, &leopard::leopard_gf8::init_leopard_gf8_tables(),
    );
    return Ok(());
}
```

**验证**:
- 编写测试：LeopardGF8 编码后，用 Classic 路径 verify 应通过（如果输出兼容）
- 或编写 LeopardGF8 自包含 roundtrip 测试：encode → decode → 比对原始数据

**预估**: 3-5 天

#### P0-1b: 实现 Leopard GF8 重建 (reconstruct / reconstruct_data)

**目标**: 实现基于 IFFT 的 Leopard GF8 重建

**算法参考**: `klauspost/reedsolomon` 的 Leopard 重建流程：
1. 对已有的 data shards 做 FFT 得到频域表示
2. 利用缺失分片位置构建插值多项式
3. 在频域做乘法和逆变换
4. IFFT 得到重建的分片

**新建文件**:
- `src/core/leopard_gf8/decode.rs` — Leopard GF8 解码/重建引擎

**修改文件**:
- `src/core/leopard_gf8/mod.rs` — 导出 decode 模块
- `src/core/reconstruct.rs` — 移除 leopard guard (line 423, 428, 437)，添加 leopard dispatch

**核心算法**:
```
输入: shards[0..total_shards] (部分为 None), shard_size
输出: 重建后的完整 shards

1. 收集已有分片索引，计算缺失分片数量
2. 如果缺失 > parity_shard_count，返回 Err(TooFewShardsPresent)
3. 对每个 chunk:
   a. 将已有 data shards 做 FFT (复用 encode 中的 fft_dit4)
   b. 构建 erasure locator 多项式
   c. Forney 算法恢复缺失分片
   d. IFFT 将频域结果转回时域
4. 写回重建的分片
```

**关键复用**:
- `ops.rs` 中的 `fft_dit4_full_lut` / `ifft_dit4_full_lut` — FFT/IFFT 蝶形运算
- `ops.rs` 中的 `lut_xor` — SIMD 加速 GF 乘法
- `tables.rs` 中的 `fft_skew`, `log_walsh` — FFT 扭转因子
- `work.rs` 中的 `FlatWork` — 工作缓冲区

**测试**:
- 随机缺失 1 个分片 → 重建 → 验证
- 随机缺失 parity_shard_count 个分片 → 重建 → 验证
- 缺失超过 parity_shard_count → 返回错误
- 所有 data shards 缺失 → 重建 data
- 所有 parity shards 缺失 → 重建 parity

**预估**: 2 周

#### P0-1c: 实现 Leopard GF8 验证 (verify)

**目标**: 实现 Leopard GF8 的 verify 操作

**算法**: 对 data shards 做 FFT 编码，将结果与现有 parity shards 比较

**修改文件**:
- `src/core/verify.rs` — 移除 leopard guard (line 52, 85, 99, 127, 154)，添加 leopard dispatch

**实现要点**:
```rust
// verify 中的 leopard 分支
if let FamilyState::LeopardGF8(ref leopard) = self.family_state {
    // 1. 分配临时 parity 缓冲区
    // 2. 调用 leopard encode 生成期望的 parity
    // 3. 逐字节比较生成的 parity 与现有 parity
    // 4. 返回 Ok(true) 或 Ok(false)
}
```

**测试**:
- 正确的 parity → verify 返回 true
- 篡改 1 字节 → verify 返回 false
- 篡改整个 shard → verify 返回 false

**预估**: 2-3 天

#### P0-1d: 实现 Leopard GF8 reconstruct_some

**目标**: 支持仅重建指定分片

**修改文件**:
- `src/core/reconstruct.rs` — 在 `reconstruct_some` 中添加 leopard 路径

**预估**: 1-2 天 (在 P0-1b 基础上)

#### P0-1e: 移除 prototype 标记与文档更新

**目标**: 将 Leopard GF8 从 prototype 状态升级为正式支持

**修改文件**:
- `src/errors.rs` — 保留 `UnsupportedLeopardPrototype` 仅用于 LeopardGF16
- `src/core/options.rs` — 更新 `CodecFamily::LeopardGF8` 文档
- `README.md` / `README_CN.md` — 更新 Leopard GF8 状态说明

**预估**: 1 天

### 测试矩阵

| 场景 | data_shards | parity_shards | shard_size | 缺失数 | 缺失位置 |
|------|-------------|---------------|------------|--------|----------|
| 最小配置 | 1 | 1 | 1B | 1 | data[0] |
| 标准配置 | 10 | 4 | 1KB | 4 | 随机 |
| 大分片 | 10 | 4 | 1MB | 2 | data[0], parity[0] |
| 边界: 全部缺失 | 10 | 4 | 1KB | 4 | 所有 data |
| 边界: 零缺失 | 10 | 4 | 1KB | 0 | — |
| 超限缺失 | 10 | 4 | 1KB | 5 | 随机 |

### 依赖关系

```
P0-1a (接入编码) → P0-1b (重建) → P0-1d (reconstruct_some)
                                   → P0-1e (移除 prototype)
P0-1a → P0-1c (验证) → P0-1e
```

---

## P0-2: 流式 API (Read/Write)

### 概述

当前 API 要求所有分片数据完整驻留内存。对于大文件（GB 级），需要基于 `std::io::Read`/`std::io::Write` 的流式编解码接口。

### Go 参考实现

```go
// klauspost/reedsolomon 流式 API
enc, _ := reedsolomon.NewStream(dataShards, parityShards)
enc.WithConcurrentStreams(true)       // 并发流读写
enc.WithStreamBlockSize(4 * 1024 * 1024) // 4MB 块

// 编码: 从 Reader 读取，写入 Writer
err = enc.Encode(dataReaders)  // []io.Reader → 写入内部

// 重建: 从 ReadWriter 读/写
err = enc.Reconstruct(shards)  // []io.ReadWriter
```

### 子任务拆分

#### P0-2a: 设计流式 API 接口

**目标**: 设计符合 Rust 习惯的流式编解码 API

**建议 API 设计**:

```rust
/// 流式编解码器配置
pub struct StreamOptions {
    /// 每次读/写的块大小 (默认 4MB)
    pub block_size: usize,
    /// 是否并发读写各流 (默认 false)
    pub concurrent_streams: bool,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            block_size: 4 * 1024 * 1024,
            concurrent_streams: false,
        }
    }
}

/// 流式错误，包装底层 I/O 错误并标识出错的分片索引
#[derive(Debug)]
pub struct StreamError {
    pub shard_index: usize,
    pub kind: StreamErrorKind,
}

#[derive(Debug)]
pub enum StreamErrorKind {
    Read(std::io::Error),
    Write(std::io::Error),
    Codec(crate::Error),
}

impl ReedSolomon {
    /// 流式编码：从 data readers 读取数据，编码后写入 parity writers
    pub fn encode_stream(
        &self,
        data: &mut [impl Read],
        parity: &mut [impl Write],
        options: &StreamOptions,
    ) -> Result<(), StreamError>;

    /// 流式重建：从可读写的 shards 流中重建缺失分片
    pub fn reconstruct_stream(
        &self,
        shards: &mut [Option<impl Read + Write>],
        options: &StreamOptions,
    ) -> Result<(), StreamError>;

    /// 流式验证：从 readers 读取并验证 parity
    pub fn verify_stream(
        &self,
        shards: &mut [impl Read],
        options: &StreamOptions,
    ) -> Result<bool, StreamError>;
}
```

**设计决策**:
- 使用 `impl Read` / `impl Write` 而非 `Box<dyn Read>` — 零开销泛型
- `StreamError` 包含 `shard_index` — 方便定位出错流
- `block_size` 默认 4MB — 平衡内存与吞吐
- `concurrent_streams` 默认 false — 避免不必要的并发开销

**预估**: 2-3 天 (含 API review)

#### P0-2b: 实现流式编码 (encode_stream)

**目标**: 实现基于 block 的流式编码

**算法**:
```
循环:
  1. 从每个 data reader 读取 block_size 字节到 shard buffers
  2. 如果所有 reader 都 EOF，退出
  3. 处理不等长读取 (短读/EOF):
     - 短读的 shard 用零填充到实际读取的最大长度
     - 记录每个 shard 的实际读取长度
  4. 调用 self.encode_sep(data_blocks, parity_blocks)
  5. 将 parity blocks 写入对应的 parity writers
  6. 如果所有写入完成，继续下一块
```

**修改文件**:
- 新建 `src/core/stream.rs` — 流式 API 实现
- `src/core/mod.rs` — 导出 stream 模块
- `src/lib.rs` — 导出 `StreamOptions`, `StreamError`

**关键实现细节**:
- 预分配 `data_buffers: Vec<Vec<u8>>` 和 `parity_buffers: Vec<Vec<u8>>`
- 使用 `Read::read_exact` 或 `Read::read` 处理短读
- 最后一个块可能不满 block_size，需正确处理
- 并发模式下使用 rayon `par_iter_mut` 并发读取各流

**测试**:
- 空输入 → 无输出
- 单块数据 → 正确编码
- 多块数据 → 正确编码
- 不等长输入 → 最后块正确处理
- reader 返回错误 → 正确传播

**预估**: 1 周

#### P0-2c: 实现流式重建 (reconstruct_stream)

**目标**: 实现基于 block 的流式重建

**算法**:
```
循环:
  1. 从每个非 None 的 shard reader 读取 block_size 字节
  2. 对 None 位置的 shard buffer 填充零
  3. 调用 self.reconstruct(buffers)
  4. 将重建的 shard blocks 写入对应的 writer
  5. 继续直到所有 reader EOF
```

**关键挑战**:
- `reconstruct` 需要知道哪些 shard 缺失 → 通过 `Option<impl Read + Write>` 的 `None` 判断
- 流式场景下每个 block 的缺失模式必须一致
- 最后一个 block 的 shard 长度可能不一致 → 需要记录每个 shard 的总长度

**测试**:
- 单分片缺失 → 正确重建
- 多分片缺失 → 正确重建
- 超限缺失 → 返回错误

**预估**: 1 周

#### P0-2d: 实现流式验证 (verify_stream)

**目标**: 实现基于 block 的流式验证

**算法**:
```
循环:
  1. 从所有 shard readers 读取 block_size 字节
  2. 分离 data 和 parity blocks
  3. 调用 self.verify(buffers)
  4. 如果任何块验证失败，返回 Ok(false)
  5. 所有块通过 → 返回 Ok(true)
```

**预估**: 2-3 天

#### P0-2e: 并发流读写支持

**目标**: 实现 `concurrent_streams` 选项

**实现**: 当 `concurrent_streams = true` 时：
- 使用 rayon `par_iter_mut` 并发从各 data reader 读取
- 编码后并发写入各 parity writer
- 注意：读取和编码必须串行（编码依赖完整数据），但读取各流可并发，写入各流可并发

**预估**: 2-3 天

#### P0-2f: 测试与文档

**测试场景**:
- 与内存 API 的结果一致性测试
- 大文件 (100MB+) 流式编解码 roundtrip
- 错误传播测试 (reader/writer 返回错误)
- 性能基准测试 vs 内存 API

**文档**:
- 在 README 中添加流式 API 使用示例
- 在 `StreamOptions` 和 `StreamError` 上添加完整文档注释

**预估**: 2-3 天

### 依赖关系

```
P0-2a (API 设计) → P0-2b (encode_stream) → P0-2c (reconstruct_stream)
                                           → P0-2d (verify_stream)
                 → P0-2e (并发流)
P0-2b + P0-2c + P0-2d → P0-2f (测试与文档)
```

---

## P0 整体里程碑

```
Week 1-2:  P0-1a (接入编码) + P0-2a (API 设计) + P0-2b (encode_stream)
Week 3-4:  P0-1b (重建) + P0-2c (reconstruct_stream)
Week 5:    P0-1c (验证) + P0-1d (reconstruct_some) + P0-2d (verify_stream)
Week 6:    P0-1e (移除标记) + P0-2e (并发) + P0-2f (测试文档)
```
