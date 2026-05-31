# P1-2: SIMD 生成式代码 (Codegen) — 子任务详细文档

> 文档日期: 2026-05-31
> 预估总工作量: 1-2 周
> 前置依赖: 无

---

## 概述

通过 `build.rs` 代码生成，为常见分片配置 (如 10+4, 12+4) 创建专用的编码函数，消除运行时循环开销和间接调用。

## 文件影响范围

| 文件 | 操作 | 说明 |
|------|------|------|
| `build.rs` | 修改 | 添加代码生成逻辑 |
| `src/galois_8/x86/codegen.rs` | **新建** | 生成的代码模块 |
| `src/galois_8/x86/mod.rs` | 修改 | 导出 codegen |
| `src/core/encode.rs` | 修改 | 添加 codegen dispatch |
| `src/tests/mod.rs` | 修改 | 添加 codegen 测试 |

---

## P1-2a: 收益评估

### P1-2a-1: 配置分布调研

**目标**: 确定哪些 (data_shards, parity_shards) 配置最常用

**调研范围**:
- MinIO 默认配置: 通常 4+2, 6+3, 8+3, 10+4
- Ceph 默认配置: 通常 4+2, 8+3, 8+4
- HDFS EC: 通常 6+3, 10+4, 12+4
- 本项目的测试配置

**输出**: 配置频率表

| 配置 | 使用场景 | 优先级 |
|------|----------|--------|
| 10+4 | MinIO, HDFS | 高 |
| 12+4 | HDFS | 高 |
| 8+3 | MinIO, Ceph | 高 |
| 6+3 | Ceph | 中 |
| 4+2 | MinIO, Ceph | 中 |
| 16+4 | 大规模存储 | 低 |

**预估**: 0.5 天

### P1-2a-2: 基准测试对比

**目标**: 量化 codegen 的性能收益

**方法**: 手动编写一个 10x4 的展开版本，与通用循环版本对比

**手动展开版本** (概念):
```rust
fn encode_10x4_unrolled_avx2(data: &[&[u8]; 10], parity: &mut [&mut [u8]; 4]) {
    let shard_len = data[0].len();
    for chunk in 0..shard_len / 32 {
        let offset = chunk * 32;
        // 展开 10 次 load + mul + xor，无循环
        let d0 = _mm256_loadu_si256(data[0][offset..].as_ptr());
        // ... 10 次
        // ... 4 个 parity 的计算
    }
}
```

**测试**: 1MB shard, 10+4 配置

**预估**: 1 天

### P1-2a-3: 评估报告

**输出**: `docs/ec-codegen-evaluation.md`

**内容**:
- 配置频率分布
- 展开 vs 通用循环的性能差距
- 推荐 codegen 的配置列表
- 实现方案选择

**预估**: 0.5 天

---

## P1-2b: build.rs 代码生成

### P1-2b-1: 生成器框架

**目标**: 在 `build.rs` 中实现代码生成框架

**文件**: `build.rs`

```rust
fn generate_encode_codegen(out_dir: &str) {
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_arch != "x86_64" {
        return; // 仅 x86_64
    }

    let path = std::path::Path::new(out_dir).join("codegen_encode.rs");
    let mut code = String::new();

    // 生成的配置列表
    let configs = [
        (4, 2),
        (6, 3),
        (8, 3),
        (8, 4),
        (10, 4),
        (12, 4),
        (16, 4),
    ];

    for (d, p) in configs {
        code.push_str(&generate_encode_function(d, p));
    }

    std::fs::write(&path, code).unwrap();
}

fn generate_encode_function(d: usize, p: usize) -> String {
    // 生成函数签名和展开的循环
    // ...
}
```

**预估**: 2 天

### P1-2b-2: 10x4 AVX2 生成

**目标**: 为 10+4 配置生成 AVX2 专用编码函数

**生成的代码结构**:
```rust
// 自动生成
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn encode_10x4_avx2(
    data: &[&[u8]; 10],
    parity: &mut [&mut [u8]; 4],
    matrix: &[[u8; 32]; 10], // AVX2 宽度的矩阵行
) {
    let shard_len = data[0].len();
    let chunks = shard_len / 32;

    for c in 0..chunks {
        let off = c * 32;

        // 加载 data shards
        let d0 = _mm256_loadu_si256(data[0][off..].as_ptr() as *const _);
        let d1 = _mm256_loadu_si256(data[1][off..].as_ptr() as *const _);
        let d2 = _mm256_loadu_si256(data[2][off..].as_ptr() as *const _);
        let d3 = _mm256_loadu_si256(data[3][off..].as_ptr() as *const _);
        let d4 = _mm256_loadu_si256(data[4][off..].as_ptr() as *const _);
        let d5 = _mm256_loadu_si256(data[5][off..].as_ptr() as *const _);
        let d6 = _mm256_loadu_si256(data[6][off..].as_ptr() as *const _);
        let d7 = _mm256_loadu_si256(data[7][off..].as_ptr() as *const _);
        let d8 = _mm256_loadu_si256(data[8][off..].as_ptr() as *const _);
        let d9 = _mm256_loadu_si256(data[9][off..].as_ptr() as *const _);

        // 计算 parity[0] = Σ matrix[i][0] * d[i]
        let mut p0 = gf_mul_avx2(d0, matrix[0]);
        p0 = _mm256_xor_si256(p0, gf_mul_avx2(d1, matrix[1]));
        p0 = _mm256_xor_si256(p0, gf_mul_avx2(d2, matrix[2]));
        // ... 展开所有 10 个 data shards

        // 计算 parity[1], parity[2], parity[3] 类似
        // ...

        // 存储 parity shards
        _mm256_storeu_si256(parity[0][off..].as_mut_ptr() as *mut _, p0);
        // ...
    }

    // 处理尾部 (< 32 字节)
    // ...
}
```

**关键**: `gf_mul_avx2` 使用 nibble-lookup 实现，与现有 `avx2.rs` 中的逻辑相同但内联展开。

**预估**: 2 天

### P1-2b-3: 其他配置生成

**为其他配置生成类似函数**:
- `encode_4x2_avx2`
- `encode_6x3_avx2`
- `encode_8x3_avx2`
- `encode_8x4_avx2`
- `encode_12x4_avx2`
- `encode_16x4_avx2`

**生成器**: 使用 Rust 模板字符串在 `build.rs` 中生成

**预估**: 1 天

---

## P1-2c: 集成

### P1-2c-1: encode dispatch

**目标**: 在 `encode_sep` 中检查是否有 codegen 快速路径

**文件**: `src/core/encode.rs`

```rust
// 在 encode_sep 的 classic 路径中
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
{
    if let Some(result) = x86::codegen::try_encode_codegen_avx2(
        data, parity, &self.matrix, self.data_shard_count, self.parity_shard_count,
    ) {
        return result;
    }
}

// 回退到通用路径
```

**dispatch 函数**:
```rust
// src/galois_8/x86/codegen.rs
pub fn try_encode_codegen_avx2(
    data: &[impl AsRef<[u8]>],
    parity: &mut [impl AsMut<[u8]>],
    matrix: &Matrix,
    data_count: usize,
    parity_count: usize,
) -> Option<Result<(), Error>> {
    match (data_count, parity_count) {
        (10, 4) => Some(unsafe { encode_10x4_avx2_dispatch(data, parity, matrix) }),
        (12, 4) => Some(unsafe { encode_12x4_avx2_dispatch(data, parity, matrix) }),
        // ...
        _ => None, // 无 codegen，回退通用路径
    }
}
```

**预估**: 1 天

### P1-2c-2: 测试

```rust
#[test]
fn test_codegen_encode_10x4_matches_generic() {
    let rs = ReedSolomon::new(10, 4).unwrap();
    let mut shards = alloc_test_shards(14, 1024);
    fill_random(&mut shards);

    let (data, mut parity) = split_data_parity(&mut shards, 10);
    rs.encode_sep(&data, &mut parity).unwrap();

    // 与通用路径的结果比较
    // (需要强制使用通用路径的方式，如 RSE_BACKEND_OVERRIDE=scalar)
}
```

**预估**: 1 天

---

## 依赖关系

```
P1-2a-1 + P1-2a-2 → P1-2a-3 (评估报告)
P1-2a-3 → P1-2b-1 → P1-2b-2 → P1-2b-3
P1-2b-3 → P1-2c-1 → P1-2c-2
```

**关键路径**: P1-2a → P1-2b → P1-2c
