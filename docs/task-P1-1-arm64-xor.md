# P1-1: ARM64 NEON XOR 专用优化 — 子任务详细文档

> 文档日期: 2026-05-31
> 预估总工作量: 1-2 周
> 前置依赖: 无

---

## 概述

优化 ARM64 NEON 后端的 `mul_slice_xor`，为常见系数 (c=0, c=1) 添加快速路径，消除不必要的 nibble-lookup 开销。同时统一两个独立实现为 const-generic 模式。

## 文件影响范围

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/galois_8/aarch64/neon.rs` | 修改 | 添加快速路径，const-generic 统一 |
| `src/galois_8/scalar.rs` | 修改 | 添加 scalar 快速路径 |
| `src/galois_8/backend.rs` | 可能修改 | 如果函数签名变化 |

---

## P1-1a: c=1 快速路径

### P1-1a-1: 实现 xor_slice_neon 函数

**目标**: 当 `c == 1` 时，跳过 nibble-lookup，直接使用 `veorq_u8` 做纯 XOR

**文件**: `src/galois_8/aarch64/neon.rs`

**当前状态**: `rust_neon_mul_slice_xor_impl` (line 132) 对所有系数执行相同的 nibble-lookup:
```rust
unsafe fn rust_neon_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    let (low_lut, high_lut) = build_neon_luts(c);
    // ... 64 bytes/iteration nibble-lookup + XOR ...
}
```

**新增函数**:
```rust
/// 纯 XOR 快速路径: 64 字节/迭代
/// 用于 c=1 时，MUL_TABLE[1][x] == x，乘法退化为恒等
#[inline]
unsafe fn xor_slice_neon(input: &[u8], out: &mut [u8]) {
    let len = input.len().min(out.len());
    let mut offset = 0;

    // 主循环: 64 字节/迭代 (4x16B NEON 寄存器)
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

    // 处理 16 字节对齐的剩余部分
    while offset + 16 <= len {
        let a = vld1q_u8(input.as_ptr().add(offset));
        let b = vld1q_u8(out.as_ptr().add(offset));
        vst1q_u8(out.as_ptr().add(offset), veorq_u8(a, b));
        offset += 16;
    }

    // 处理尾部 (< 16 字节)
    while offset < len {
        *out.get_unchecked_mut(offset) ^= *input.get_unchecked(offset);
        offset += 1;
    }
}
```

**性能分析**:
- 当前 nibble-lookup: 每 16 字节需要 2 次 `vqtbl1q_u8` + 1 次 `veorq_u8` + 加载 LUT
- 纯 XOR: 每 16 字节仅需 1 次 `veorq_u8`
- 预期吞吐量提升: 2-4x

**预估**: 1 天

### P1-1a-2: 集成到 mul_slice_xor

**目标**: 在 `rust_neon_mul_slice_xor_impl` 开头添加快速路径分支

**修改**: `src/galois_8/aarch64/neon.rs` line 132

```rust
unsafe fn rust_neon_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    // 快速路径: c=0 时 XOR 不改变结果
    if c == 0 {
        return;
    }

    // 快速路径: c=1 时退化为纯 XOR
    if c == 1 {
        return xor_slice_neon(input, out);
    }

    // 原有 nibble-lookup 逻辑
    let (low_lut, high_lut) = build_neon_luts(c);
    // ...
}
```

**预估**: 0.5 天

### P1-1a-3: 正确性测试

**测试**:
```rust
#[test]
fn test_neon_mul_slice_xor_c1_matches_scalar() {
    let input: Vec<u8> = (0..1024).map(|i| i as u8).collect();
    let mut out_neon = vec![0xABu8; 1024];
    let mut out_scalar = vec![0xABu8; 1024];

    unsafe {
        rust_neon_mul_slice_xor(1, &input, &mut out_neon);
        scalar_mul_slice_xor(1, &input, &mut out_scalar);
    }

    assert_eq!(out_neon, out_scalar);
}

#[test]
fn test_neon_mul_slice_xor_c0_noop() {
    let input: Vec<u8> = (0..1024).map(|i| i as u8).collect();
    let mut out = vec![0xABu8; 1024];
    let original = out.clone();

    unsafe {
        rust_neon_mul_slice_xor(0, &input, &mut out);
    }

    assert_eq!(out, original);
}
```

**预估**: 0.5 天

### P1-1a-4: 性能基准测试

**文件**: `benches/galois_backend.rs` 或新建 `benches/arm64_xor_benchmark.rs`

```rust
fn bench_mul_slice_xor_c1(c: &mut Criterion) {
    let mut group = c.benchmark_group("mul_slice_xor_c1");
    let input = vec![0xABu8; 1024 * 1024];
    let mut out = vec![0u8; 1024 * 1024];

    group.bench_function("neon", |b| {
        b.iter(|| unsafe { rust_neon_mul_slice_xor(1, &input, &mut out) });
    });

    group.bench_function("neon_c2", |b| {
        b.iter(|| unsafe { rust_neon_mul_slice_xor(2, &input, &mut out) });
    });

    group.finish();
}
```

**预期结果**: c=1 的吞吐量显著高于 c=2 (接近纯 memcpy 速度)

**预估**: 0.5 天

---

## P1-1b: c=0 快速路径

### P1-1b-1: 实现

**文件**: `src/galois_8/aarch64/neon.rs`

在 P1-1a-2 中已包含。额外需要为非 XOR 路径也添加:

```rust
unsafe fn rust_neon_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    // c=0: 输出全零
    if c == 0 {
        let len = input.len().min(out.len());
        out[..len].fill(0);
        return;
    }

    // c=1: 复制输入到输出
    if c == 1 {
        let len = input.len().min(out.len());
        out[..len].copy_from_slice(&input[..len]);
        return;
    }

    // 原有逻辑
    // ...
}
```

**预估**: 0.5 天

---

## P1-1c: const-generic 统一

### P1-1c-1: 合并函数签名

**目标**: 将 `rust_neon_mul_slice_impl` 和 `rust_neon_mul_slice_xor_impl` 合并为一个 `fn impl<const XOR: bool>()`

**文件**: `src/galois_8/aarch64/neon.rs`

**当前状态**: 两个独立函数，约 260 行重复的 nibble-lookup 代码

**合并后**:
```rust
#[inline]
unsafe fn rust_neon_mul_impl<const XOR: bool>(c: u8, input: &[u8], out: &mut [u8]) {
    if c == 0 {
        if XOR { return; }
        out[..input.len()].fill(0);
        return;
    }
    if c == 1 {
        if XOR { return xor_slice_neon(input, out); }
        out[..input.len()].copy_from_slice(&input[..input.len()]);
        return;
    }

    let (low_lut, high_lut) = build_neon_luts(c);
    let len = input.len().min(out.len());
    let mut offset = 0;

    while offset + 64 <= len {
        let a0 = vld1q_u8(input.as_ptr().add(offset));
        let a1 = vld1q_u8(input.as_ptr().add(offset + 16));
        let a2 = vld1q_u8(input.as_ptr().add(offset + 32));
        let a3 = vld1q_u8(input.as_ptr().add(offset + 48));

        let mut r0 = lut_xor_neon(a0, low_lut, high_lut);
        let mut r1 = lut_xor_neon(a1, low_lut, high_lut);
        let mut r2 = lut_xor_neon(a2, low_lut, high_lut);
        let mut r3 = lut_xor_neon(a3, low_lut, high_lut);

        if XOR {
            r0 = veorq_u8(r0, vld1q_u8(out.as_ptr().add(offset)));
            r1 = veorq_u8(r1, vld1q_u8(out.as_ptr().add(offset + 16)));
            r2 = veorq_u8(r2, vld1q_u8(out.as_ptr().add(offset + 32)));
            r3 = veorq_u8(r3, vld1q_u8(out.as_ptr().add(offset + 48)));
        }

        vst1q_u8(out.as_ptr().add(offset), r0);
        vst1q_u8(out.as_ptr().add(offset + 16), r1);
        vst1q_u8(out.as_ptr().add(offset + 32), r2);
        vst1q_u8(out.as_ptr().add(offset + 48), r3);

        offset += 64;
    }

    // 处理剩余字节...
}

pub fn rust_neon_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    unsafe { rust_neon_mul_impl::<false>(c, input, out); }
}

pub fn rust_neon_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    unsafe { rust_neon_mul_impl::<true>(c, input, out); }
}
```

**编译器优化**: Rust 编译器会为 `XOR=true` 和 `XOR=false` 各生成一份代码，`if XOR` 分支在编译时消除，无运行时开销。

**预估**: 1 天

### P1-1c-2: 调用方更新

**检查**: `backend.rs` 中的函数指针签名:
```rust
pub(crate) struct GaloisBackend {
    pub(crate) mul_slice: fn(u8, &[u8], &mut [u8]),
    pub(crate) mul_slice_xor: fn(u8, &[u8], &mut [u8]),
    // ...
}
```

签名不变，无需修改调用方。

**预估**: 0.5 天

### P1-1c-3: 回归测试

运行完整测试套件:
```bash
cargo test --features simd-accel
```

确保所有 SIMD 正确性测试通过。

**预估**: 0.5 天

---

## P1-1d: scalar 后端快速路径

### P1-1d-1: scalar_mul_slice 优化

**文件**: `src/galois_8/scalar.rs`

```rust
pub fn scalar_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    if c == 0 {
        out[..input.len()].fill(0);
        return;
    }
    if c == 1 {
        let len = input.len().min(out.len());
        out[..len].copy_from_slice(&input[..len]);
        return;
    }
    let table = &MUL_TABLE[c as usize];
    for (i, o) in input.iter().zip(out.iter_mut()) {
        *o = table[*i as usize];
    }
}
```

**预估**: 0.5 天

### P1-1d-2: scalar_mul_slice_xor 优化

**文件**: `src/galois_8/scalar.rs`

```rust
pub fn scalar_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    if c == 0 {
        return;
    }
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

**预估**: 0.5 天

---

## 依赖关系

```
P1-1a-1 → P1-1a-2 → P1-1a-3 + P1-1a-4
P1-1a-2 包含 P1-1b-1
P1-1a-2 + P1-1b-1 → P1-1c-1 → P1-1c-2 → P1-1c-3
P1-1d-1 + P1-1d-2 (独立，可与 P1-1a 并行)
```

**关键路径**: P1-1a-1 → P1-1a-2 → P1-1c-1 → P1-1c-3
