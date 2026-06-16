# P3 — 开发体验任务

> 优先级：低 | 提升 API 易用性和文档质量
> 预估总工作量：1-2 周

---

## 目录

- [P3-1: CodecOptions Builder 模式与 max_threads](#p3-1-codecoptions-builder-模式与-max_threads)
- [P3-2: 基于分片大小的自动并行度调优](#p3-2-基于分片大小的自动并行度调优)
- [P3-3: Leopard GF8 限制文档完善](#p3-3-leopard-gf8-限制文档完善)
- [P3-4: 与 Go 实现的跨平台基准对比](#p3-4-与-go-实现的跨平台基准对比)

---

## P3-1: CodecOptions Builder 模式与 max_threads

### 概述

当前 `CodecOptions` 是一个纯数据结构体，没有 builder 方法。`max_jobs` 仅通过环境变量 `RS_PARALLEL_POLICY_MAX_JOBS` 控制。需要添加 builder API 和 `max_parallel_jobs` 配置项。

### 当前状态

**`CodecOptions`** (`src/core/options.rs:17-23`):
```rust
pub struct CodecOptions {
    pub fast_one_parity: bool,           // default: false
    pub inversion_cache: bool,           // default: true
    pub inversion_cache_capacity: usize, // default: 0 (auto)
    pub codec_family: CodecFamily,       // default: Classic
    pub matrix_mode: MatrixMode,         // default: Vandermonde
}
```

无 builder 方法。用户直接构造:
```rust
let opts = CodecOptions {
    fast_one_parity: true,
    ..Default::default()
};
let rs = ReedSolomon::with_options(10, 4, opts);
```

### 子任务拆分

#### P3-1a: 添加 CodecOptions Builder 方法

**目标**: 为 `CodecOptions` 添加流式 builder API

**修改文件**: `src/core/options.rs`

**实现**:
```rust
impl CodecOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fast_one_parity(mut self, val: bool) -> Self {
        self.fast_one_parity = val;
        self
    }

    pub fn with_inversion_cache(mut self, val: bool) -> Self {
        self.inversion_cache = val;
        self
    }

    pub fn with_inversion_cache_capacity(mut self, capacity: usize) -> Self {
        self.inversion_cache_capacity = capacity;
        self
    }

    pub fn with_codec_family(mut self, family: CodecFamily) -> Self {
        self.codec_family = family;
        self
    }

    pub fn with_matrix_mode(mut self, mode: MatrixMode) -> Self {
        self.matrix_mode = mode;
        self
    }

    pub fn with_max_parallel_jobs(mut self, jobs: usize) -> Self {
        self.max_parallel_jobs = jobs;
        self
    }
}
```

**使用示例**:
```rust
let rs = ReedSolomon::with_options(
    10, 4,
    CodecOptions::new()
        .with_codec_family(CodecFamily::LeopardGF8)
        .with_max_parallel_jobs(4),
);
```

**预估**: 1 天

#### P3-1b: 添加 max_parallel_jobs 字段

**目标**: 在 `CodecOptions` 中添加 `max_parallel_jobs` 字段，替代纯环境变量控制

**修改文件**:
- `src/core/options.rs` — 添加字段
- `src/core/parallel.rs` — 在 `resolve_policy_cache` 中读取此字段
- `src/core/codec.rs` — 将 options 传递到 policy 解析

**实现**:
```rust
// options.rs
pub struct CodecOptions {
    // ... 现有字段
    /// 最大并行任务数。0 表示使用环境变量或系统默认值。
    pub max_parallel_jobs: usize,  // default: 0
}
```

```rust
// parallel.rs — resolve_policy_cache
fn resolve_policy_cache(options: &CodecOptions) -> RuntimeParallelPolicyCache {
    let mut policy = ParallelPolicy::default().with_env_overrides();
    if options.max_parallel_jobs > 0 {
        policy.max_jobs = options.max_parallel_jobs;
    }
    // ... 现有逻辑
}
```

**优先级说明**: `CodecOptions.max_parallel_jobs` 优先于环境变量 `RS_PARALLEL_POLICY_MAX_JOBS`。环境变量作为全局覆盖仍然可用。

**预估**: 1 天

#### P3-1c: 测试与文档

**测试**:
- builder 模式产生的 options 与直接构造的一致
- `max_parallel_jobs` 确实限制了并行度
- `max_parallel_jobs = 0` 回退到环境变量/默认值

**文档**:
- 在 `CodecOptions` 文档中说明 builder 用法
- 在 README 中添加示例

**预估**: 0.5 天

### 依赖关系

```
P3-1a + P3-1b → P3-1c
```

---

## P3-2: 基于分片大小的自动并行度调优

### 概述

Go 的 `WithAutoGoroutines(shardSize)` 根据分片大小自动计算最优 goroutine 数量。Rust 的并行策略基于 `available_parallelism()`，未考虑分片大小对缓存的影响。

### 当前并行决策算法

`src/core/parallel.rs:56-109`:
```
max_jobs = min(max_jobs or available_parallelism, available_parallelism)
chunk_count = shard_size / min_bytes_per_job
max_useful_jobs = chunk_count (if output <= 2) or output * chunk_count
jobs = min(max_jobs, max_useful_jobs)
```

问题: 未考虑 L2/L3 缓存大小。当分片很大时，每个 chunk 可能超出 L2 缓存，导致缓存抖动。

### 子任务拆分

#### P3-2a: 缓存感知的并行度计算

**目标**: 在并行决策中考虑缓存大小

**算法改进**:
```
// 估算每个核心的理想工作集大小
l2_cache_per_core = 256 KB (典型值，可配置)
ideal_chunk_size = l2_cache_per_core / (data_shards + output_shards)
chunk_count = shard_size / ideal_chunk_size
jobs = min(max_jobs, chunk_count)
```

**修改文件**: `src/core/parallel.rs`

**实现**:
```rust
impl ParallelPolicy {
    pub fn with_cache_aware_sizing(mut self, l2_cache_bytes: usize) -> Self {
        self.l2_cache_bytes = l2_cache_bytes;
        self
    }
}
```

**预估**: 2-3 天

#### P3-2b: 自动缓存大小检测

**目标**: 运行时检测缓存大小

**方案**:
- 在 Linux 上读取 `/sys/devices/system/cpu/cpu0/cache/` 获取 L2/L3 大小
- 在 macOS 上使用 `sysctl hw.l2cachesize`
- 在 Windows 上使用 `GetLogicalProcessorInformation`
- 回退到默认值 256KB

**修改文件**: `src/core/parallel.rs` 或新建 `src/core/cache_detect.rs`

**预估**: 2-3 天

### 依赖关系

```
P3-2a → P3-2b
```

---

## P3-3: Leopard GF8 限制文档完善

### 概述

Leopard GF8 的限制未在公共 API 文档中充分说明。需要在代码文档和 README 中清晰列出。

### 限制清单

从 Go 参考和 Rust 实现中总结:

1. **分片大小对齐**: 分片大小应为 64 字节的倍数（性能最佳）
2. **等长分片**: 所有分片必须等长；最后一个分片需要零填充
3. **分片数量**: data + parity 总数不超过 256 (GF8 的域大小)
4. **不支持 `update`**: 增量更新不适用于 Leopard 编码
5. **不支持 `encode_single`**: 逐分片编码不适用于 Leopard
6. **不兼容 Classic 输出**: Leopard 编码的 parity 与 Classic 编码不兼容
7. **单线程**: Leopard FFT 目前不支持并行（Go 也如此）

### 子任务拆分

#### P3-3a: 更新 CodecFamily 文档

**修改文件**: `src/core/options.rs`

```rust
/// Leopard GF(2^8) 编解码器。
///
/// 使用 FFT-based 算法，适用于分片数较多 (通常 >20-30) 的场景。
/// 复杂度 O(N log N)，优于 Classic 的 O(N²)。
///
/// # 限制
///
/// - 分片大小建议为 64 字节的倍数
/// - 所有分片必须等长 (最后一个分片需零填充)
/// - data + parity 总数不超过 256
/// - 不支持 `update()` 增量更新
/// - 不支持 `encode_single()` / `encode_single_sep()` 逐分片编码
/// - 编码输出与 Classic 模式不兼容
///
/// # 示例
///
/// ```rust
/// use rustfs_erasure_codec::{ReedSolomon, CodecOptions, CodecFamily};
///
/// let rs = ReedSolomon::with_options(
///     10, 4,
///     CodecOptions::new().with_codec_family(CodecFamily::LeopardGF8),
/// ).unwrap();
/// ```
LeopardGF8,
```

**预估**: 0.5 天

#### P3-3b: 运行时限制检查

**目标**: 在 Leopard GF8 路径中添加运行时限制检查

**修改文件**: `src/core/encode.rs`, `src/core/reconstruct.rs`

**检查项**:
```rust
// 在 leopard encode 入口
if shards.iter().any(|s| s.len() != shard_size) {
    return Err(Error::IncorrectShardSize);
}
if shard_size % 64 != 0 {
    // 警告: 非 64 字节对齐可能影响性能
}
if self.total_shard_count() > 256 {
    return Err(Error::TooManyShards);
}
```

**预估**: 0.5 天

#### P3-3c: README 更新

**修改文件**: `README.md`, `README_CN.md`

添加 Leopard GF8 使用示例和限制说明。

**预估**: 0.5 天

### 依赖关系

```
P3-3a + P3-3b + P3-3c (全部独立，可并行)
```

---

## P3-4: 与 Go 实现的跨平台基准对比

### 概述

需要建立系统化的基准测试框架，与 `klauspost/reedsolomon` 进行性能对比。

### 子任务拆分

#### P3-4a: 定义标准基准测试配置

**目标**: 定义与 Go 实现对齐的测试配置矩阵

**配置矩阵**:

| data_shards | parity_shards | shard_size | SIMD 后端 |
|-------------|---------------|------------|-----------|
| 4 | 2 | 4KB | Scalar |
| 4 | 2 | 4KB | AVX2 / NEON |
| 4 | 2 | 1MB | AVX2 / NEON |
| 10 | 4 | 4KB | Scalar |
| 10 | 4 | 4KB | AVX2 / NEON |
| 10 | 4 | 1MB | AVX2 / NEON |
| 10 | 4 | 4MB | AVX2 / NEON |
| 12 | 4 | 1MB | AVX2 / NEON |
| 16 | 4 | 1MB | AVX2 / NEON |
| 32 | 4 | 1MB | AVX2 / NEON |

**操作**: Encode, Reconstruct, Verify

**预估**: 1 天

#### P3-4b: 实现标准化基准测试

**修改文件**: `benches/cross_platform_comparison.rs` (新建)

**使用 Criterion**:
```rust
fn bench_encode(c: &mut Criterion) {
    let configs = vec![
        (10, 4, 1024 * 1024),  // 10+4, 1MB
        // ...
    ];
    for (d, p, sz) in configs {
        let group_name = format!("encode_{}x{}_{}KB", d, p, sz / 1024);
        c.bench_function(&group_name, |b| {
            let rs = ReedSolomon::new(d, p).unwrap();
            let shards = alloc_test_shards(d + p, sz);
            b.iter(|| rs.encode_sep(&data, &mut parity).unwrap());
        });
    }
}
```

**预估**: 2-3 天

#### P3-4c: Go 基准测试对齐

**目标**: 在 Go 侧编写相同配置的基准测试

**新建文件**: Go 项目中的 `bench_comparison_test.go`

```go
func BenchmarkEncode_10x4_1MB(b *testing.B) {
    enc, _ := reedsolomon.New(10, 4)
    shards := make([][]byte, 14)
    for i := range shards {
        shards[i] = make([]byte, 1024*1024)
    }
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        enc.Encode(shards)
    }
}
```

**预估**: 1 天

#### P3-4d: 结果分析文档

**目标**: 记录跨平台、跨语言的性能对比结果

**输出**: `docs/ec-cross-platform-benchmark-results.md`

**内容**:
- 测试环境 (CPU 型号、OS、编译器版本)
- Rust vs Go 性能对比表
- SIMD 后端性能对比
- x86_64 vs aarch64 对比
- 性能瓶颈分析

**预估**: 1 天

### 依赖关系

```
P3-4a → P3-4b (Rust 基准)
P3-4a → P3-4c (Go 基准)
P3-4b + P3-4c → P3-4d (结果分析)
```

---

## P3 整体里程碑

```
Week 1:    P3-1a+P3-1b (builder) + P3-3a+P3-3b+P3-3c (leopard docs)
Week 2:    P3-2a (cache-aware) + P3-4a (benchmark configs)
Week 3:    P3-2b (cache detect) + P3-4b+P3-4c (benchmarks)
Week 4:    P3-4d (results)
```
