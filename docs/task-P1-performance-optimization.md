# P1 — 性能优化任务

> 优先级：高 | 直接影响吞吐量
> 预估总工作量：2-4 周

---

## 目录

- [P1-1: ARM64 NEON XOR 专用优化](#p1-1-arm64-neon-xor-专用优化)
- [P1-2: SIMD 生成式代码 (Codegen)](#p1-2-simd-生成式代码-codegen)
- [P1-3: GFNI 后端文档修正与自动启用评估](#p1-3-gfni-后端文档修正与自动启用评估)

---

## P1-1: ARM64 NEON XOR 专用优化

### 概述

当前 NEON 后端的 `mul_slice_xor` 对所有系数使用相同的 nibble-lookup 路径。对于常见系数 (c=0, c=1)，可以大幅优化。

### 当前状态

**文件**: `src/galois_8/aarch64/neon.rs`

| 函数 | 行号 | 功能 |
|------|------|------|
| `rust_neon_mul_slice` | 10 | 非 XOR 路径，64B/迭代 |
| `rust_neon_mul_slice_xor` | 25 | XOR 路径，64B/迭代 (unroll4) 或 32B/迭代 (unroll2) |
| `rust_neon_mul_slice_impl` | 41 | 非 XOR 核心实现 |
| `rust_neon_mul_slice_xor_impl` | 132 | XOR 核心实现 |

**问题**:
1. **c=1 未优化**: 当系数为 1 时，`MUL_TABLE[1][x] == x`，XOR 路径退化为纯 `out[i] ^= input[i]`，但仍执行完整 nibble-lookup
2. **c=0 未优化**: 当系数为 0 时，乘积恒为 0，XOR 路径是 no-op，但仍执行全部计算
3. **代码重复**: 两个独立实现，nibble-lookup 逻辑完全重复
4. **x86 后端已使用 const-generic 统一**: `avx2.rs` 用 `fn impl<const XOR: bool>()` 合并两个路径

### 子任务拆分

#### P1-1a: c=1 快速路径

**目标**: 当 `c == 1` 时，跳过 nibble-lookup，直接使用 `veorq_u8`

**修改文件**: `src/galois_8/aarch64/neon.rs`

**实现**:
```rust
unsafe fn rust_neon_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    // c=1: 纯 XOR 快速路径
    if c == 1 {
        return xor_slice_neon(input, out);
    }
    // c=0: no-op
    if c == 0 {
        return;
    }
    // ... 原有 nibble-lookup 逻辑
}

/// NEON 加速的纯 XOR: 64 字节/迭代
unsafe fn xor_slice_neon(input: &[u8], out: &mut [u8]) {
    let len = input.len().min(out.len());
    let mut offset = 0;
    while offset + 64 <= len {
        let a0 = vld1q_u8(input.as_ptr().add(offset));
        let a1 = vld1q_u8(input.as_ptr().add(offset + 16));
        let a2 = vld1q_u8(input.as_ptr().add(offset + 32));
        let a3 = vld1q_u8(input.as_ptr().add(offset + 48));
        let b0 = vld1q_u8(out.as_ptr().add(offset));
        let b1 = vld1q_u8(out.as_ptr().add(offset + 16));
        let b2 = vld1q_u8(out.as_ptr().add(offset + 32));
        let b3 = vld1q_u8(out.as_ptr().add(offset + 48));
        vst1q_u8(out.as_ptr().add(offset), veorq_u8(a0, b0));
        vst1q_u8(out.as_ptr().add(offset + 16), veorq_u8(a1, b1));
        vst1q_u8(out.as_ptr().add(offset + 32), veorq_u8(a2, b2));
        vst1q_u8(out.as_ptr().add(offset + 48), veorq_u8(a3, b3));
        offset += 64;
    }
    // 处理剩余字节...
}
```

**预期收益**: c=1 时吞吐量提升 2-4x（消除表查找开销）

**测试**:
- 正确性: 与 nibble-lookup 路径的输出逐字节比较
- 性能: c=1 基准测试 vs c=其他值

**预估**: 2-3 天

#### P1-1b: c=0 快速路径

**目标**: 当 `c == 0` 时，直接 return

**修改文件**: `src/galois_8/aarch64/neon.rs`

**实现**: 在 `rust_neon_mul_slice_xor_impl` 开头添加:
```rust
if c == 0 {
    return; // 乘积恒为 0，XOR 不改变结果
}
```

**同样适用于** `rust_neon_mul_slice_impl`: 输出全零

**预估**: 0.5 天

#### P1-1c: const-generic 统一

**目标**: 将两个独立实现合并为一个 `fn impl<const XOR: bool>()`，消除代码重复

**修改文件**: `src/galois_8/aarch64/neon.rs`

**实现**:
```rust
#[inline]
unsafe fn rust_neon_mul_impl<const XOR: bool>(c: u8, input: &[u8], out: &mut [u8]) {
    if c == 0 {
        if XOR { return; } // XOR 路径: no-op
        // 非 XOR 路径: 填充零
        out[..input.len()].fill(0);
        return;
    }
    if c == 1 && XOR {
        return xor_slice_neon(input, out);
    }

    let (low_lut, high_lut) = build_neon_luts(c);
    // ... 统一的 nibble-lookup 逻辑
    // XOR 路径在每个迭代多一步:
    //   let existing = vld1q_u8(out_ptr);
    //   result = veorq_u8(result, existing);
}

pub fn rust_neon_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    unsafe { rust_neon_mul_impl::<false>(c, input, out); }
}

pub fn rust_neon_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    unsafe { rust_neon_mul_impl::<true>(c, input, out); }
}
```

**预期收益**: 代码量减少约 40%，维护成本降低

**测试**: 所有现有 NEON 测试必须通过

**预估**: 2-3 天

#### P1-1d: scalar 后端 c=1/c=0 快速路径

**目标**: 在 scalar 后端也添加 c=1/c=0 快速路径

**修改文件**: `src/galois_8/scalar.rs`

**当前代码** (scalar.rs):
```rust
pub fn scalar_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    let table = &MUL_TABLE[c as usize];
    for (i, o) in input.iter().zip(out.iter_mut()) {
        *o = table[*i as usize];
    }
}
```

**优化后**:
```rust
pub fn scalar_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    if c == 0 {
        out[..input.len()].fill(0);
        return;
    }
    if c == 1 {
        out[..input.len()].copy_from_slice(&input[..input.len()]);
        return;
    }
    let table = &MUL_TABLE[c as usize];
    for (i, o) in input.iter().zip(out.iter_mut()) {
        *o = table[*i as usize];
    }
}

pub fn scalar_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    if c == 0 { return; }
    if c == 1 {
        for (i, o) in input.iter().zip(out.iter_mut()) {
            *o ^= *i;
        }
        return;
    }
    let table = &MUL_TABLE[c as usize];
    for (i, o) in input.iter().zip(out.iter_mut()) {
        *o ^= table[*i as usize];
    }
}
```

**预估**: 1 天

### 依赖关系

```
P1-1a + P1-1b → P1-1c (统一后自然包含快速路径)
P1-1d (独立，可并行)
```

---

## P1-2: SIMD 生成式代码 (Codegen)

### 概述

Go 实现通过代码生成为常见分片配置 (如 10+4, 12+4) 创建专用函数，避免循环开销和间接调用。Rust 可通过 `build.rs` 实现类似优化。

### Go 参考

`klauspost/reedsolomon` 的 `galois_gen_amd64.go` 包含类似:
```go
//go:noescape
func galMulSSSE3_10x1(in, out [][]byte, matrix []byte)
func galMulSSSE3_12x4(in, out [][]byte, matrix []byte)
```

每个函数硬编码循环次数和寄存器分配，消除运行时循环控制开销。

### 子任务拆分

#### P1-2a: 评估 codegen 收益

**目标**: 确定哪些分片配置值得代码生成

**方法**:
1. 统计 MinIO / 常见使用场景的分片配置分布
2. 对 top-10 配置 (如 4+2, 6+3, 8+4, 10+4, 12+4, 16+4) 做基准测试
3. 比较通用循环 vs 展开循环的性能差距

**输出**: 一份配置-收益矩阵，决定哪些配置值得 codegen

**预估**: 2-3 天

#### P1-2b: 实现 build.rs 代码生成

**目标**: 在 `build.rs` 中为选定的分片配置生成专用编码函数

**生成目标**:
```rust
// 自动生成的代码 (示例)
#[cfg(target_arch = "x86_64")]
pub fn encode_10x4_avx2(data: &[&[u8]], parity: &mut [&mut [u8]], matrix: &[[u8; 32]; 10]) {
    // 展开的 10 次 gf_mul_slice + xor，无循环
    for chunk_idx in 0..data[0].len() / 32 {
        let d0 = _mm256_loadu_si256(data[0][chunk_idx..].as_ptr());
        // ... 展开所有 10 个 data shards
        // ... 生成 4 个 parity shards
    }
}
```

**修改文件**:
- `build.rs` — 添加代码生成逻辑
- `src/galois_8/x86/codegen.rs` — 新建，存放生成的代码和 dispatch 逻辑
- `src/galois_8/x86/mod.rs` — 导出 codegen 模块
- `src/core/encode.rs` — 在 encode 路径中检查是否有 codegen 快速路径

**设计要点**:
- 仅在 `target_arch = "x86_64"` 且 `target_feature` 包含 avx2 时生成
- 生成的函数签名: `fn encode_DxP_avx2(data: &[&[u8]], parity: &mut [&mut [u8]], matrix_rows: &[[u8; 32]; D])`
- 使用 `include!()` 宏引入 build.rs 生成的文件
- 保留通用路径作为 fallback

**预估**: 1 周

#### P1-2c: 编码路径集成

**目标**: 在 `encode_sep` 中添加 codegen dispatch

**实现**:
```rust
// encode.rs — encode_sep 中
match (self.data_shard_count, self.parity_shard_count) {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    (10, 4) if shard_size >= 32 => {
        x86::codegen::encode_10x4_avx2(data_slices, parity_slices, &self.matrix_rows);
        return Ok(());
    }
    _ => { /* 通用路径 */ }
}
```

**预估**: 2-3 天

### 依赖关系

```
P1-2a (评估) → P1-2b (生成) → P1-2c (集成)
```

---

## P1-3: GFNI 后端文档修正与自动启用评估

### 概述

发现文档与代码不一致：文档声称 GFNI 是 override-only，但代码实际自动选择 GFNI。

### 当前状态

**文档** (`backend.rs:303-304`):
```
/// GFNI backends are override-only: never auto-selected due to limited deployment
/// and validation. Opt in via `RSE_BACKEND_OVERRIDE=rust-gfni-avx2`.
```

**代码** (`backend.rs:391-411`):
```rust
fn select_x86_backend(features: CpuFeatures) -> &'static GaloisBackend {
    if supports_rust_gfni_avx512(features) {
        return RUST_GFNI_AVX512_BACKEND;  // 自动选择!
    }
    // ...
}
```

**测试** (`backend.rs:631`): `test_select_x86_backend_priority` 确认 GFNI 自动选择

### 子任务拆分

#### P1-3a: 修正文档

**目标**: 更新 doc comments 使其与代码行为一致

**修改文件**: `src/galois_8/backend.rs`

**修改内容**:
```rust
// 将 line 303-304 从:
/// GFNI backends are override-only: never auto-selected due to limited deployment
// 改为:
/// GFNI backends are auto-selected on supporting hardware (Ice Lake+).
/// Manual override is also available via `RSE_BACKEND_OVERRIDE=rust-gfni-avx2`.
```

**预估**: 0.5 天

#### P1-3b: GFNI 性能验证

**目标**: 在支持 GFNI 的硬件上验证 GFNI 是否确实优于 AVX2

**方法**:
1. 在 Ice Lake / Sapphire Rapids 上运行基准测试
2. 比较 `RSE_BACKEND_OVERRIDE=rust-gfni-avx2` vs `RSE_BACKEND_OVERRIDE=rust-avx2`
3. 测试配置: (10+4, 1MB), (10+4, 4KB), (12+4, 1MB)
4. 记录结果到文档

**输出**: `docs/gfni-benchmark-results.md`

**预估**: 2-3 天 (需要 GFNI 硬件)

#### P1-3c: 决策 — 是否需要回退策略

**目标**: 如果 GFNI 在某些场景下不如 AVX2，是否需要智能切换

**方案**:
- **方案 A**: 保持当前行为 (自动选择 GFNI)，在文档中说明
- **方案 B**: 添加配置选项让用户选择是否启用 GFNI 自动选择
- **方案 C**: 根据分片大小动态选择 (小分片 AVX2，大分片 GFNI)

**预估**: 1 天 (决策) + 实现取决于方案选择

### 依赖关系

```
P1-3a (独立，立即可做)
P1-3b → P1-3c
```

---

## P1 整体里程碑

```
Week 1:    P1-1a (c=1 fast path) + P1-1b (c=0 fast path) + P1-1d (scalar) + P1-3a (docs fix)
Week 2:    P1-1c (const-generic) + P1-2a (codegen evaluation)
Week 3:    P1-2b (codegen generation)
Week 4:    P1-2c (codegen integration) + P1-3b (GFNI benchmark)
```
