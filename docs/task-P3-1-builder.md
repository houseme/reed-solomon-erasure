# P3-1: CodecOptions Builder 模式与 max_threads — 子任务详细文档

> 文档日期: 2026-05-31
> 预估总工作量: 2-3 天
> 前置依赖: 无

---

## 概述

为 `CodecOptions` 添加 builder API 和 `max_parallel_jobs` 配置项，提供比纯环境变量更友好的并行度控制。

---

## P3-1a: Builder 方法

### P3-1a-1: 实现 builder 方法

**文件**: `src/core/options.rs`

**当前状态**: `CodecOptions` 是纯数据结构体，无 builder 方法

**添加**:
```rust
impl CodecOptions {
    /// 创建默认配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置快速单校验路径
    pub fn with_fast_one_parity(mut self, val: bool) -> Self {
        self.fast_one_parity = val;
        self
    }

    /// 设置反转矩阵缓存
    pub fn with_inversion_cache(mut self, val: bool) -> Self {
        self.inversion_cache = val;
        self
    }

    /// 设置反转矩阵缓存容量
    pub fn with_inversion_cache_capacity(mut self, capacity: usize) -> Self {
        self.inversion_cache_capacity = capacity;
        self
    }

    /// 设置编解码器族
    pub fn with_codec_family(mut self, family: CodecFamily) -> Self {
        self.codec_family = family;
        self
    }

    /// 设置矩阵模式
    pub fn with_matrix_mode(mut self, mode: MatrixMode) -> Self {
        self.matrix_mode = mode;
        self
    }

    /// 设置最大并行任务数。0 表示使用系统默认值。
    pub fn with_max_parallel_jobs(mut self, jobs: usize) -> Self {
        self.max_parallel_jobs = jobs;
        self
    }
}
```

**预估**: 0.5 天

### P3-1a-2: builder 测试

```rust
#[test]
fn test_codec_options_builder() {
    let opts = CodecOptions::new()
        .with_fast_one_parity(true)
        .with_codec_family(CodecFamily::LeopardGF8)
        .with_max_parallel_jobs(4);

    assert!(opts.fast_one_parity);
    assert_eq!(opts.codec_family, CodecFamily::LeopardGF8);
    assert_eq!(opts.max_parallel_jobs, 4);
}

#[test]
fn test_codec_options_default() {
    let opts = CodecOptions::default();
    assert!(!opts.fast_one_parity);
    assert!(opts.inversion_cache);
    assert_eq!(opts.codec_family, CodecFamily::Classic);
    assert_eq!(opts.max_parallel_jobs, 0);
}
```

**预估**: 0.5 天

---

## P3-1b: max_parallel_jobs 字段

### P3-1b-1: 字段添加

**文件**: `src/core/options.rs`

```rust
pub struct CodecOptions {
    pub fast_one_parity: bool,
    pub inversion_cache: bool,
    pub inversion_cache_capacity: usize,
    pub codec_family: CodecFamily,
    pub matrix_mode: MatrixMode,
    /// 最大并行任务数。0 表示使用环境变量或系统默认值。
    pub max_parallel_jobs: usize,  // 新增
}

impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            fast_one_parity: false,
            inversion_cache: true,
            inversion_cache_capacity: 0,
            codec_family: CodecFamily::Classic,
            matrix_mode: MatrixMode::Vandermonde,
            max_parallel_jobs: 0,  // 新增
        }
    }
}
```

**预估**: 0.5 天

### P3-1b-2: policy 集成

**目标**: 将 `max_parallel_jobs` 传递到 `ParallelPolicy`

**文件**: `src/core/parallel.rs`, `src/core/codec.rs`

**修改 `resolve_policy_cache`** (parallel.rs:235):
```rust
fn resolve_policy_cache(
    data_shard_count: usize,
    parity_shard_count: usize,
    options: &CodecOptions,  // 新增参数
) -> RuntimeParallelPolicyCache {
    let mut policy = ParallelPolicy::default().with_env_overrides();

    // CodecOptions 的 max_parallel_jobs 优先于环境变量
    if options.max_parallel_jobs > 0 {
        policy.max_jobs = options.max_parallel_jobs;
    }

    // ... 现有逻辑
}
```

**修改 `ReedSolomon::with_options`** (codec.rs:257):
```rust
let policy_cache = Self::resolve_policy_cache(
    data_shard_count, total_shard_count, &options,
);
```

**预估**: 0.5 天

### P3-1b-3: 测试

```rust
#[test]
fn test_max_parallel_jobs_limits_parallelism() {
    let opts = CodecOptions::new().with_max_parallel_jobs(2);
    let rs = ReedSolomon::with_options(10, 4, opts).unwrap();

    // 验证并行策略被限制
    let policy = rs.effective_parallel_policy();
    // policy 中的 jobs 应不超过 2
}
```

**预估**: 0.5 天

---

## P3-1c: 文档

### P3-1c-1: doc comments

**文件**: `src/core/options.rs`

为 `CodecOptions` 和所有 builder 方法添加完整的文档注释，包含使用示例。

**预估**: 0.5 天

---

## 依赖关系

```
P3-1a-1 + P3-1b-1 → P3-1b-2 → P3-1b-3
P3-1a-1 → P3-1a-2
P3-1b-3 → P3-1c-1
```
