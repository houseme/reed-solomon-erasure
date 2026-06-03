# P3-4: 与 Go 实现的跨平台基准对比 — 子任务详细文档

> **状态: 基本完成** — 3 个 Criterion bench 目标 + 2 个 smoke test + 共享基础设施；P3-4c-1 Go 基准待实现
> 文档日期: 2026-05-31
> 预估总工作量: 3-5 天
> 前置依赖: 无

---

## 概述

建立系统化的基准测试框架，与 `klauspost/reedsolomon` 进行性能对比。

---

## P3-4a: 配置定义

### P3-4a-1: 配置矩阵

**标准测试配置**:

| ID | data | parity | shard_size | 说明 |
|----|------|--------|------------|------|
| C1 | 4 | 2 | 4KB | 小配置，小文件 |
| C2 | 4 | 2 | 1MB | 小配置，大文件 |
| C3 | 10 | 4 | 4KB | 中配置，小文件 |
| C4 | 10 | 4 | 1MB | 中配置，大文件 |
| C5 | 10 | 4 | 4MB | 中配置，超大文件 |
| C6 | 12 | 4 | 1MB | HDFS 常见 |
| C7 | 16 | 4 | 1MB | 大配置 |
| C8 | 32 | 4 | 1MB | Leopard 适用场景 |

**操作**: Encode, Reconstruct (缺失 1 个), Verify

**预估**: 0.5 天

---

## P3-4b: Rust 基准

### P3-4b-1: Criterion 框架

**新建文件**: `benches/cross_platform.rs`

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use reed_solomon_erasure::ReedSolomon;

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    let configs = [
        (4, 2, 4096),
        (4, 2, 1048576),
        (10, 4, 4096),
        (10, 4, 1048576),
        (10, 4, 4194304),
        (12, 4, 1048576),
        (16, 4, 1048576),
    ];

    for (d, p, sz) in configs {
        let id = BenchmarkId::new(format!("{}x{}", d, p), sz);
        group.bench_with_input(id, &sz, |b, &sz| {
            let rs = ReedSolomon::new(d, p).unwrap();
            let shards: Vec<Vec<u8>> = (0..d + p).map(|_| vec![0u8; sz]).collect();
            let mut data: Vec<&[u8]> = shards[..d].iter().map(|s| s.as_slice()).collect();
            let mut parity: Vec<&mut [u8]> = shards[d..].iter()
                .map(|s| unsafe { &mut *(s.as_slice() as *const [u8] as *mut [u8]) })
                .collect();
            b.iter(|| rs.encode_sep(&data, &mut parity).unwrap());
        });
    }

    group.finish();
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
```

**预估**: 1 天

### P3-4b-2: encode 基准

在 P3-4b-1 的框架中填充所有 encode 配置

**预估**: 0.5 天

### P3-4b-3: reconstruct 基准

```rust
fn bench_reconstruct(c: &mut Criterion) {
    let mut group = c.benchmark_group("reconstruct");

    for (d, p, sz) in &CONFIGS {
        let id = BenchmarkId::new(format!("{}x{}", d, p), *sz);
        group.bench_with_input(id, sz, |b, &sz| {
            let rs = ReedSolomon::new(*d, *p).unwrap();
            // ... 准备数据，随机擦除 1 个分片
            b.iter(|| rs.reconstruct(&mut shards).unwrap());
        });
    }

    group.finish();
}
```

**预估**: 0.5 天

---

## P3-4c: Go 基准

### P3-4c-1: Go 基准代码

**新建文件**: 在 Go 项目中创建 `bench_comparison_test.go`

```go
package reedsolomon

import (
    "testing"
)

var configs = []struct {
    data, parity, size int
}{
    {4, 2, 4096},
    {4, 2, 1048576},
    {10, 4, 4096},
    {10, 4, 1048576},
    {10, 4, 4194304},
    {12, 4, 1048576},
    {16, 4, 1048576},
}

func BenchmarkEncode(b *testing.B) {
    for _, cfg := range configs {
        name := fmt.Sprintf("%dx%d_%dKB", cfg.data, cfg.parity, cfg.size/1024)
        b.Run(name, func(b *testing.B) {
            enc, _ := New(cfg.data, cfg.parity)
            shards := make([][]byte, cfg.data+cfg.parity)
            for i := range shards {
                shards[i] = make([]byte, cfg.size)
            }
            b.ResetTimer()
            for i := 0; i < b.N; i++ {
                enc.Encode(shards)
            }
        })
    }
}
```

**预估**: 1 天

---

## P3-4d: 结果分析

### P3-4d-1: 数据收集

**在以下平台执行**:
- x86_64 (Intel/AMD)
- aarch64 (Apple M1/M2, AWS Graviton)

**记录**: 每个配置的吞吐量 (MB/s)

**预估**: 0.5 天

### P3-4d-2: 报告撰写

**新建文件**: `docs/ec-cross-platform-benchmark-results.md`

**内容**:
- 测试环境
- Rust vs Go 性能对比表
- SIMD 后端对比
- 架构对比
- 结论与优化建议

**预估**: 0.5 天

---

## 依赖关系

```
P3-4a → P3-4b (Rust 基准)
P3-4a → P3-4c (Go 基准)
P3-4b + P3-4c → P3-4d (结果分析)
```
