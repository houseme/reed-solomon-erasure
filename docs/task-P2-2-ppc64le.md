# P2-2: ppc64le SIMD 后端 — 子任务详细文档

> **状态: ❌ 未实现** — 零 ppc64le/VSX 代码，8 个子任务待实现
> 文档日期: 2026-05-31
> 预估总工作量: 1-2 周
> 前置依赖: 无

---

## 概述

为 IBM POWER (ppc64le) 架构添加 SIMD 加速支持，复用 C SIMD 中已有的 AltiVec 代码，并实现 Rust 原生 VSX 后端。

## 文件影响范围

| 文件 | 操作 | 说明 |
|------|------|------|
| `build.rs` | 修改 | 允许 ppc64le 编译 C SIMD |
| `src/galois_8/ppc64le/mod.rs` | **新建** | 模块定义 |
| `src/galois_8/ppc64le/vsx.rs` | **新建** | VSX 实现 |
| `src/galois_8/backend.rs` | 修改 | 添加 ppc64le dispatch |
| `src/galois_8/mod.rs` | 修改 | 导出 ppc64le 模块 |

---

## P2-2a: 启用 C SIMD 的 ppc64le 编译

### P2-2a-1: build.rs 修改

**目标**: 让 build.rs 允许 ppc64le 架构编译 C SIMD 代码

**文件**: `build.rs` (line 167-177)

**当前代码**:
```rust
let arch_supported = matches!(target_arch.as_str(), "x86_64" | "aarch64");
```

**修改为**:
```rust
let arch_supported = matches!(
    target_arch.as_str(),
    "x86_64" | "aarch64" | "powerpc64"
);
```

**同时检查**: C SIMD 源码中的 AltiVec 部分是否需要额外编译标志:
- `simd_c/reedsolomon.c` line 91-99 已有 `__ALTIVEC__` 检测
- 可能需要添加 `-maltivec` 编译标志

```rust
if target_arch == "powerpc64" {
    build.flag("-maltivec");
}
```

**预估**: 0.5 天

### P2-2a-2: 编译验证

**目标**: 在 ppc64le 交叉编译环境下验证 C SIMD 编译

```bash
# 交叉编译测试
cargo build --target powerpc64le-unknown-linux-gnu --features simd-accel
```

**需要**: ppc64le 交叉编译工具链

**预估**: 0.5 天

---

## P2-2b: Rust VSX 后端

### P2-2b-1: nibble-lookup VSX 实现

**目标**: 使用 VSX intrinsics 实现 nibble-lookup GF 乘法

**新建文件**: `src/galois_8/ppc64le/vsx.rs`

**VSX intrinsics** (通过 `core::arch::powerpc64`):
- `vec_ld` — 向量加载 (128-bit)
- `vec_st` — 向量存储
- `vec_xor` — 向量 XOR
- `vec_perm` — 向量排列 (可用于 nibble-lookup)

**核心思路**: 与 NEON/SSSE3 相同的 nibble-lookup 策略:
1. 将字节拆分为低 4 位和高 4 位
2. 使用 `vec_perm` 从 16 字节 LUT 中查找
3. XOR 合并

```rust
#[cfg(target_arch = "powerpc64")]
use core::arch::powerpc64::*;

#[inline]
unsafe fn vsx_lut_xor(
    input: &[u8],
    out: &mut [u8],
    low_lut: &[u8; 16],
    high_lut: &[u8; 16],
) {
    let l = vec_ld(0, low_lut.as_ptr());
    let h = vec_ld(0, high_lut.as_ptr());

    let mut offset = 0;
    while offset + 16 <= input.len() {
        let a = vec_ld(offset as i32, input.as_ptr());
        let lo = vec_and(a, vec_splat_u8(0x0F));
        let hi = vec_sr(a, vec_splat_u8(4));
        let r = vec_xor(vec_perm(l, l, lo), vec_perm(h, h, hi));

        let existing = vec_ld(offset as i32, out.as_ptr());
        let result = vec_xor(r, existing);
        vec_st(result, offset as i32, out.as_mut_ptr());

        offset += 16;
    }
    // 处理尾部...
}
```

**预估**: 3 天

### P2-2b-2: mul_slice 实现

```rust
pub fn rust_vsx_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    if c == 0 { out[..input.len()].fill(0); return; }
    if c == 1 { out[..input.len()].copy_from_slice(&input[..input.len()]); return; }
    let (low, high) = build_vsx_luts(c);
    unsafe { vsx_lut_xor(input, out, &low, &high); }
}
```

**预估**: 2 天

### P2-2b-3: mul_slice_xor 实现

```rust
pub fn rust_vsx_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    if c == 0 { return; }
    if c == 1 { /* pure XOR */ return; }
    let (low, high) = build_vsx_luts(c);
    unsafe { vsx_lut_xor(input, out, &low, &high); }
}
```

**预估**: 1 天

---

## P2-2c: 后端注册

### P2-2c-1: backend.rs dispatch

**文件**: `src/galois_8/backend.rs`

```rust
#[cfg(target_arch = "powerpc64")]
const RUST_VSX_BACKEND: GaloisBackend = GaloisBackend {
    mul_slice: ppc64le::vsx::rust_vsx_mul_slice,
    mul_slice_xor: ppc64le::vsx::rust_vsx_mul_slice_xor,
    name: "rust-vsx",
    id: BackendId::RustVsx,
    kind: BackendKind::RustSimd,
};
```

**预估**: 1 天

### P2-2c-2: 自动选择逻辑

```rust
#[cfg(target_arch = "powerpc64")]
fn select_ppc64le_backend() -> &'static GaloisBackend {
    // VSX 在 ppc64le 上始终可用
    RUST_VSX_BACKEND
}
```

**预估**: 0.5 天

---

## P2-2d: 测试

### P2-2d-1: 正确性测试

```rust
#[test]
fn test_vsx_mul_slice_matches_scalar() {
    // 与 scalar 输出逐字节比较
}
```

**预估**: 1 天

### P2-2d-2: 性能基准

**预估**: 1 天

---

## 依赖关系

```
P2-2a (独立)
P2-2b-1 → P2-2b-2 + P2-2b-3
P2-2b-3 → P2-2c-1 + P2-2c-2
P2-2c-2 → P2-2d-1 + P2-2d-2
```
