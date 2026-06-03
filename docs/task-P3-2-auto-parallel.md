# P3-2: 基于分片大小的自动并行度调优 — 子任务详细文档

> **状态: ✅ 已完成** — cache_detect 模块（Linux sysfs + macOS sysctl），cache-aware chunk sizing
> 文档日期: 2026-05-31
> 预估总工作量: 3-5 天
> 前置依赖: 无

---

## 概述

在并行决策中考虑 CPU 缓存大小，根据分片大小自动计算最优并行度，避免缓存抖动。

---

## P3-2a: 缓存感知并行度计算

### P3-2a-1: 算法设计

**当前算法** (parallel.rs:56-109):
```
max_jobs = min(max_jobs or available_parallelism, available_parallelism)
chunk_count = shard_size / min_bytes_per_job
jobs = min(max_jobs, chunk_count)
```

**问题**: 未考虑 L2/L3 缓存。当 shard_size 很大时，每个 chunk 可能超出 L2 缓存。

**改进算法**:
```
// 估算每个核心的理想工作集大小
l2_per_core = 256KB (可配置)
shard_elements = data_shards + output_shards
ideal_chunk = l2_per_core / shard_elements
chunk_count = shard_size / ideal_chunk
jobs = min(max_jobs, chunk_count)
```

**添加字段**:
```rust
pub struct ParallelPolicy {
    pub min_parallel_shard_bytes: usize,
    pub min_bytes_per_job: usize,
    pub max_jobs: usize,
    pub l2_cache_bytes: usize,  // 新增: L2 缓存大小估算
}
```

**预估**: 1 天

### P3-2a-2: 实现

**文件**: `src/core/parallel.rs`

```rust
impl ParallelPolicy {
    pub fn with_l2_cache_bytes(mut self, bytes: usize) -> Self {
        self.l2_cache_bytes = bytes;
        self
    }
}

// decide() 中:
if self.l2_cache_bytes > 0 {
    let ideal_chunk = self.l2_cache_bytes / (data_shards + output_shards).max(1);
    let cache_chunk_count = shard_size.div_ceil(ideal_chunk.max(1));
    chunk_count = chunk_count.min(cache_chunk_count);
}
```

**预估**: 1 天

### P3-2a-3: 测试

```rust
#[test]
fn test_cache_aware_reduces_chunks() {
    let policy = ParallelPolicy {
        l2_cache_bytes: 256 * 1024,
        ..Default::default()
    };
    let decision = policy.decide(1024 * 1024, 10, 4);
    // 验证 chunk 数量受缓存约束
}
```

**预估**: 0.5 天

---

## P3-2b: 缓存大小自动检测

### P3-2b-1: Linux 检测

**新建文件**: `src/core/cache_detect.rs`

```rust
#[cfg(target_os = "linux")]
pub fn detect_l2_cache_bytes() -> Option<usize> {
    // 读取 /sys/devices/system/cpu/cpu0/cache/index2/size
    let path = "/sys/devices/system/cpu/cpu0/cache/index2/size";
    let content = std::fs::read_to_string(path).ok()?;
    parse_cache_size(&content)
}

fn parse_cache_size(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Some(k) = s.strip_suffix('K') {
        k.parse::<usize>().ok().map(|v| v * 1024)
    } else if let Some(m) = s.strip_suffix('M') {
        m.parse::<usize>().ok().map(|v| v * 1024 * 1024)
    } else {
        s.parse().ok()
    }
}
```

**预估**: 1 天

### P3-2b-2: macOS 检测

```rust
#[cfg(target_os = "macos")]
pub fn detect_l2_cache_bytes() -> Option<usize> {
    // 使用 sysctl hw.l2cachesize
    let output = std::process::Command::new("sysctl")
        .arg("-n")
        .arg("hw.l2cachesize")
        .output().ok()?;
    let s = String::from_utf8(output.stdout).ok()?;
    s.trim().parse().ok()
}
```

**预估**: 0.5 天

### P3-2b-3: 回退默认值

```rust
pub fn detect_l2_cache_bytes() -> Option<usize> {
    #[cfg(target_os = "linux")]
    { detect_l2_cache_bytes_linux().or(Some(256 * 1024)) }

    #[cfg(target_os = "macos")]
    { detect_l2_cache_bytes_macos().or(Some(256 * 1024)) }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { Some(256 * 1024) } // 默认 256KB
}
```

**集成**: 在 `ParallelPolicy::default()` 中调用检测:
```rust
impl Default for ParallelPolicy {
    fn default() -> Self {
        Self {
            min_parallel_shard_bytes: PARALLEL_MIN_SHARD_BYTES,
            min_bytes_per_job: CODE_SLICE_LARGE_CHUNK_BYTES,
            max_jobs: 0,
            l2_cache_bytes: detect_l2_cache_bytes().unwrap_or(256 * 1024),
        }
    }
}
```

**预估**: 0.5 天

---

## 依赖关系

```
P3-2a-1 → P3-2a-2 → P3-2a-3
P3-2b-1 + P3-2b-2 + P3-2b-3 → P3-2a-2 (集成到 default)
```
