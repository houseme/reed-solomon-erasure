# aarch64 架构源码分析

> 基于 rustfs-erasure-codec v7.0.1 (Rust Edition 2024) 源码<br>
> 分析日期：2026-05-30

---

## 目录

1. [项目总览](#1-项目总览)
2. [aarch64 相关文件索引](#2-aarch64-相关文件索引)
3. [GF(2^8) 有限域算术基础](#3-gf28-有限域算术基础)
4. [编译期查找表生成](#4-编译期查找表生成)
5. [运行时后端选择机制](#5-运行时后端选择机制)
6. [NEON SIMD 核心实现](#6-neon-simd-核心实现)
7. [Leopard GF8 编解码器中的 NEON 加速](#7-leopard-gf8-编解码器中的-neon-加速)
8. [C SIMD 后端（Legacy FFI）](#8-c-simd-后端legacy-ffi)
9. [并行执行策略（aarch64 专有）](#9-并行执行策略aarch64-专有)
10. [NEON 性能剖析指标](#10-neon-性能剖析指标)
11. [内存布局与对齐](#11-内存布局与对齐)
12. [SVE 预留扩展槽](#12-sve-预留扩展槽)
13. [编码与解码流程](#13-编码与解码流程)
14. [架构层次总结](#14-架构层次总结)

---

## 1. 项目总览

本项目是一个纯 Rust 实现的 Reed-Solomon 纠删码库，支持 GF(2^8) 和 GF(2^16) 有限域，提供 Classic（Vandermonde/Cauchy 矩阵）和 Leopard GF8（FFT/NTT 变换）两种编解码族。核心性能优化依赖 SIMD 指令集加速 GF(2^8) 域上的 slice 级乘法运算。

### 源码目录结构

```
src/
├── lib.rs                          # crate 根，Field trait 定义
├── macros.rs                       # 内部宏
├── errors.rs                       # Error / SBSError 枚举
├── matrix.rs                       # Matrix<F>：高斯消元、求逆、Vandermonde
├── galois_8/                       # GF(2^8) 主要实现
│   ├── mod.rs                      # Field 结构体，add/mul/div/exp，后端分发
│   ├── aligned.rs                  # AlignedShard（64 字节对齐分配）
│   ├── backend.rs                  # 运行时后端选择（scalar / SIMD-C / Rust-NEON / AVX2 等）
│   ├── scalar.rs                   # 纯 Rust 标量回退
│   ├── policy.rs                   # 并行执行策略，aarch64 专有覆盖
│   ├── profile.rs                  # NEON 性能剖析原子计数器
│   ├── tests.rs                    # 单元测试（含 aarch64 专有测试）
│   ├── aarch64/                    # aarch64 专有 SIMD 实现
│   │   ├── mod.rs                  # 模块声明（neon / sve）
│   │   ├── neon.rs                 # Rust NEON GF(2^8) mul_slice / mul_slice_xor
│   │   └── sve.rs                  # SVE 占位桩（未实现）
│   ├── x86/                        # x86_64 专有 SIMD 实现
│   └── legacy/                     # C SIMD 后端 FFI
│       ├── mod.rs
│       └── simd_c.rs               # extern "C" 绑定
├── galois_16.rs                    # GF(2^16)（无 SIMD 路径）
├── core/
│   ├── mod.rs                      # ReedSolomon<F> 结构体
│   ├── codec.rs                    # 构造器、矩阵初始化
│   ├── encode.rs                   # 编码（串行 + 并行）
│   ├── reconstruct.rs              # 重建
│   ├── verify.rs                   # 校验
│   ├── parallel.rs                 # ParallelPolicy / ParallelDecision
│   ├── options.rs                  # CodecOptions / CodecFamily
│   ├── leopard.rs                  # Leopard 编解码族
│   └── leopard_gf8/                # Leopard GF8 FFT 编解码器
│       ├── mod.rs                  # LeopardGf8Tables / FftDit8Plan
│       ├── tables.rs               # FFT 偏斜表 / log/exp LUT
│       ├── encode.rs               # FFT 编码
│       ├── driver.rs               # 编码驱动
│       ├── ops.rs                  # SIMD 加速 ops（lut_xor / slice_xor / FFT butterfly）
│       └── work.rs                 # 工作缓冲区管理
└── tests/
    └── mod.rs                      # 集成测试
simd_c/
├── reedsolomon.c                   # C SIMD 实现（NEON / SSE2 / SSSE3 / AVX2 / AVX512）
└── reedsolomon.h                   # C 头文件
build.rs                            # 构建脚本：查找表生成 + C SIMD 编译
```

---

## 2. aarch64 相关文件索引

### 2.1 专有实现文件

| 文件路径 | 职责 |
|---------|------|
| `src/galois_8/aarch64/mod.rs` | 模块声明，条件编译门控 |
| `src/galois_8/aarch64/neon.rs` | **核心 NEON GF(2^8) 乘法** — `rust_neon_mul_slice` / `rust_neon_mul_slice_xor` |
| `src/galois_8/aarch64/sve.rs` | SVE 预留占位桩 |

### 2.2 包含 aarch64 条件编译代码的文件

| 文件路径 | aarch64 相关内容 |
|---------|-----------------|
| `src/galois_8/backend.rs` | `RUST_NEON_BACKEND` 常量、`Aarch64FeatureSet` 结构体、`detect_aarch64_features()`、`select_aarch64_backend()` |
| `src/galois_8/policy.rs` | 5 个 aarch64 专有环境变量、`reconstruct_policy_cache_aarch64()` |
| `src/galois_8/profile.rs` | `RUST_NEON_PROFILE_METRICS` 静态变量、`record_call()` 方法 |
| `src/galois_8/tests.rs` | 6 个 aarch64 专有测试函数 |
| `src/galois_8/legacy/simd_c.rs` | FFI 绑定（`cfg` 包含 `target_arch = "aarch64"`） |
| `src/core/leopard_gf8/ops.rs` | `lut_xor_neon()` 函数、`lut_xor()` 中的 aarch64 分支 |
| `simd_c/reedsolomon.c` | C 级 NEON 内联函数 |
| `build.rs` | `should_compile_simd_c_for_target()` 包含 "aarch64" |

---

## 3. GF(2^8) 有限域算术基础

### 3.1 域定义

GF(2^8) 是包含 256 个元素的有限域，基于不可约多项式：

```
p(x) = x^8 + x^4 + x^3 + x^2 + 1  (十六进制 0x11D，十进制 29)
```

域上的运算规则：
- **加法**：`a + b = a XOR b`（GF(2^n) 上加法即异或）
- **乘法**：通过 log/exp 查找表实现
- **除法**：`a / b = exp(log(a) - log(b))`

### 3.2 核心运算实现

定义在 `src/galois_8/mod.rs`：

```rust
// 加法：XOR 即 GF(2^8) 加法
pub fn add(a: u8, b: u8) -> u8 {
    a ^ b
}

// 乘法：直接查表
pub fn mul(a: u8, b: u8) -> u8 {
    MUL_TABLE[a as usize][b as usize]
}

// 除法：通过 log/exp 表
pub fn div(a: u8, b: u8) -> u8 {
    if a == 0 { 0 }
    else if b == 0 { panic!("Divisor is 0") }
    else {
        let log_a = LOG_TABLE[a as usize];
        let log_b = LOG_TABLE[b as usize];
        let mut log_result = log_a as isize - log_b as isize;
        if log_result < 0 { log_result += 255; }
        EXP_TABLE[log_result as usize]
    }
}
```

### 3.3 Slice 级乘法分发

核心性能热点是 `mul_slice` 和 `mul_slice_xor`，它们通过后端函数指针分发到最优实现：

```rust
pub fn mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    (backend::active_backend().mul_slice)(c, input, out);
}

pub fn mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    (backend::active_backend().mul_slice_xor)(c, input, out);
}
```

- `mul_slice`：`out[i] = c * input[i]`（覆盖写入）
- `mul_slice_xor`：`out[i] ^= c * input[i]`（累加写入）

---

## 4. 编译期查找表生成

### 4.1 构建脚本 (`build.rs`)

构建脚本在编译时生成所有查找表，通过 `include!` 宏注入到 `src/galois_8/mod.rs`：

```rust
include!(concat!(env!("OUT_DIR"), "/table.rs"));
```

### 4.2 生成的表

| 表名 | 大小 | 用途 |
|------|------|------|
| `LOG_TABLE[256]` | 256 字节 | 对数表：`log_α(a)` |
| `EXP_TABLE[510]` | 510 字节 | 指数表（2×256-2 项，支持模约减） |
| `MUL_TABLE[256][256]` | 64 KB | 完整乘法表 |
| `MUL_TABLE_LOW[256][16]` | 4 KB | 低半字节乘法结果（仅 `simd-accel` 特性） |
| `MUL_TABLE_HIGH[256][16]` | 4 KB | 高半字节乘法结果（仅 `simd-accel` 特性） |

### 4.3 半字节表的关键算法

`MUL_TABLE_LOW` 和 `MUL_TABLE_HIGH` 是 SIMD 半字节查表算法的基础。对于任意字节 `b`：

```
b = (b_hi << 4) | b_lo    // 拆分为高 4 位和低 4 位

a * b = MUL_TABLE_LOW[a][b_lo] XOR MUL_TABLE_HIGH[a][b_hi]
```

**证明**：GF(2^8) 上乘法对加法（XOR）满足分配律：

```
a * b = a * (b_hi * 16 + b_lo)
      = a * (b_hi * 16) + a * b_lo        // 分配律
      = MUL_TABLE_HIGH[a][b_hi] XOR MUL_TABLE_LOW[a][b_lo]
```

生成代码（`build.rs` 第 77-101 行）：

```rust
fn gen_mul_table_half(
    log_table: &[u8; FIELD_SIZE],
    exp_table: &[u8; EXP_TABLE_SIZE],
) -> ([[u8; 16]; FIELD_SIZE], [[u8; 16]; FIELD_SIZE]) {
    let mut low: [[u8; 16]; FIELD_SIZE] = [[0; 16]; FIELD_SIZE];
    let mut high: [[u8; 16]; FIELD_SIZE] = [[0; 16]; FIELD_SIZE];

    for a in 0..low.len() {
        for b in 0..low.len() {
            let mut result = 0;
            if !(a == 0 || b == 0) {
                let log_a = log_table[a];
                let log_b = log_table[b];
                result = exp_table[log_a as usize + log_b as usize];
            }
            if (b & 0x0F) == b {
                low[a][b] = result;       // b 在 [0, 15] 范围
            }
            if (b & 0xF0) == b {
                high[a][b >> 4] = result;  // b 是 16 的倍数
            }
        }
    }
    (low, high)
}
```

---

## 5. 运行时后端选择机制

### 5.1 后端抽象 (`src/galois_8/backend.rs`)

后端通过 `GaloisBackend` 结构体抽象，包含两个函数指针：

```rust
pub type MulSliceFn = fn(u8, &[u8], &mut [u8]);

#[derive(Copy, Clone)]
pub struct GaloisBackend {
    pub id: BackendId,
    pub mul_slice: MulSliceFn,      // out[i] = c * input[i]
    pub mul_slice_xor: MulSliceFn,  // out[i] ^= c * input[i]
    pub name: &'static str,
    pub kind: BackendKind,
}

pub enum BackendKind {
    Scalar,   // 纯 Rust 标量
    SimdC,    // C SIMD FFI
    RustSimd, // Rust SIMD 内联函数
}

pub enum BackendId {
    ScalarRust,
    SimdC,
    RustNeon,
    RustSsse3,
    RustAvx2,
    RustAvx512,
    RustGfniAvx2,
    RustGfniAvx512,
}
```

### 5.2 aarch64 后端常量

```rust
#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
const RUST_NEON_BACKEND: GaloisBackend = GaloisBackend {
    id: BackendId::RustNeon,
    mul_slice: super::aarch64::neon::rust_neon_mul_slice,
    mul_slice_xor: super::aarch64::neon::rust_neon_mul_slice_xor,
    name: "rust-neon",
    kind: BackendKind::RustSimd,
};
```

### 5.3 特征检测

```rust
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
struct Aarch64FeatureSet {
    neon: bool,
    sve: bool,
}

fn detect_aarch64_features() -> Aarch64FeatureSet {
    let sve = super::aarch64::sve::detect_sve_features().available;
    Aarch64FeatureSet {
        neon: std::arch::is_aarch64_feature_detected!("neon"),
        sve,
    }
}
```

使用标准库的 `is_aarch64_feature_detected!("neon")` 宏进行运行时 CPU 特征检测。SVE 被检测但暂未使用。

### 5.4 aarch64 后端选择优先级

```rust
fn select_aarch64_backend(features: Aarch64FeatureSet) -> GaloisBackend {
    // SVE 已检测但尚未使用；预留给未来后端
    let _sve = features.sve;

    if supports_rust_neon(features) {
        return RUST_NEON_BACKEND;       // 优先：Rust NEON
    }
    if supports_simd_c_aarch64(features) {
        return SIMD_C_BACKEND;          // 次选：C SIMD FFI
    }
    SCALAR_BACKEND                       // 兜底：纯 Rust 标量
}
```

**选择优先级**：`rust-neon` > `simd-c` > `scalar-rust`

### 5.5 后端初始化

后端通过 `spin::Once` 在首次调用时一次性初始化：

```rust
static ACTIVE_BACKEND: Once<GaloisBackend> = Once::new();

pub(super) fn active_backend() -> &'static GaloisBackend {
    ACTIVE_BACKEND.call_once(runtime_select_backend)
}
```

### 5.6 环境变量覆盖

可通过 `RSE_BACKEND_OVERRIDE` 环境变量强制指定后端：

```bash
RSE_BACKEND_OVERRIDE=rust-neon   # 强制使用 NEON
RSE_BACKEND_OVERRIDE=scalar      # 强制使用标量
RSE_BACKEND_OVERRIDE=simd-c      # 强制使用 C SIMD
RSE_BACKEND_OVERRIDE=auto        # 自动选择（默认）
```

aarch64 上有效的覆盖值：`auto`、`scalar`/`scalar-rust`、`simd-c`、`rust-neon`。

---

## 6. NEON SIMD 核心实现

### 6.1 文件位置与条件编译

`src/galois_8/aarch64/neon.rs` 是核心 NEON 实现文件。所有函数都受到严格的条件编译门控：

```rust
#[cfg(all(
    feature = "simd-accel",          // 需要 simd-accel 特性
    target_arch = "aarch64",         // 目标架构为 aarch64
    not(target_env = "msvc"),        // 排除 MSVC
    not(any(target_os = "android", target_os = "ios"))  // 排除 Android/iOS
))]
```

### 6.2 公开入口函数

```rust
pub(crate) fn rust_neon_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() { return; }
    unsafe { rust_neon_mul_slice_impl(c, input, out) }
}

pub(crate) fn rust_neon_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() { return; }
    unsafe { rust_neon_mul_slice_xor_impl(c, input, out) }
}
```

### 6.3 `rust_neon_mul_slice_impl` 详细分析

这是 `out[i] = c * input[i]` 的 NEON 实现，使用 `#[target_feature(enable = "neon")]` 标注。

#### 6.3.1 初始化阶段

```rust
#[target_feature(enable = "neon")]
unsafe fn rust_neon_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::aarch64::{
        uint8x16_t, uint8x16x4_t, vandq_u8, vdupq_n_u8, veorq_u8,
        vld1q_u8, vld1q_u8_x4, vqtbl1q_u8, vshrq_n_u8, vst1q_u8, vst1q_u8_x4,
    };

    // 1. 加载半字节查找表到 NEON 寄存器
    let low_tbl  = unsafe { vld1q_u8(MUL_TABLE_LOW[c as usize].as_ptr()) };
    let high_tbl = unsafe { vld1q_u8(MUL_TABLE_HIGH[c as usize].as_ptr()) };

    // 2. 创建半字节掩码 0x0f 广播到所有通道
    let nibble_mask = vdupq_n_u8(0x0f);

    // 3. 计算 SIMD 处理边界
    let bytes_done          = input.len() & !15usize;  // 向下取整到 16 字节
    let bytes_done_unrolled = input.len() & !63usize;  // 向下取整到 64 字节
```

**关键设计**：
- `MUL_TABLE_LOW[c]` 和 `MUL_TABLE_HIGH[c]` 各 16 字节，正好放入一个 NEON 寄存器
- `bytes_done` 确保所有 SIMD 加载/操作都在 16 字节对齐的块上进行
- `bytes_done_unrolled` 确保展开循环在 64 字节对齐的块上进行

#### 6.3.2 数据分段

```rust
    let (simd_input, tail_input) = input.split_at(bytes_done);
    let (simd_out, tail_out)     = out.split_at_mut(bytes_done);
    let (unrolled_input, remainder_input) = simd_input.split_at(bytes_done_unrolled);
    let (unrolled_out, remainder_out)     = simd_out.split_at_mut(bytes_done_unrolled);
```

数据被分为三段：
1. **展开段**（64 字节对齐）：主循环处理
2. **余数段**（16 字节对齐）：次循环处理
3. **标量尾部**（0..15 字节）：标量回退处理

#### 6.3.3 展开主循环（64 字节/迭代）

```rust
    for (input_chunk, out_chunk) in unrolled_input
        .chunks_exact(64)
        .zip(unrolled_out.chunks_exact_mut(64))
    {
        // 加载 64 字节到 4 个 NEON 寄存器
        let inputs: uint8x16x4_t = unsafe { vld1q_u8_x4(input_chunk.as_ptr()) };
        let input0 = inputs.0;
        let input1 = inputs.1;
        let input2 = inputs.2;
        let input3 = inputs.3;

        // 提取低半字节：input & 0x0f
        let low0 = vandq_u8(input0, nibble_mask);
        let low1 = vandq_u8(input1, nibble_mask);
        let low2 = vandq_u8(input2, nibble_mask);
        let low3 = vandq_u8(input3, nibble_mask);

        // 提取高半字节：input >> 4
        let high0 = vshrq_n_u8::<4>(input0);
        let high1 = vshrq_n_u8::<4>(input1);
        let high2 = vshrq_n_u8::<4>(input2);
        let high3 = vshrq_n_u8::<4>(input3);

        // 半字节查表并组合结果
        // result = MUL_TABLE_LOW[c][low_nibble] XOR MUL_TABLE_HIGH[c][high_nibble]
        let result0 = veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
        let result1 = veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));
        let result2 = veorq_u8(vqtbl1q_u8(low_tbl, low2), vqtbl1q_u8(high_tbl, high2));
        let result3 = veorq_u8(vqtbl1q_u8(low_tbl, low3), vqtbl1q_u8(high_tbl, high3));

        // 存储 64 字节
        unsafe {
            vst1q_u8_x4(
                out_chunk.as_mut_ptr(),
                uint8x16x4_t(result0, result1, result2, result3),
            )
        };
    }
```

#### 6.3.4 余数循环（16 字节/迭代）

```rust
    for (input_chunk, out_chunk) in remainder_input
        .chunks_exact(16)
        .zip(remainder_out.chunks_exact_mut(16))
    {
        let input_vec = unsafe { vld1q_u8(input_chunk.as_ptr()) };
        let low  = vandq_u8(input_vec, nibble_mask);
        let high = vshrq_n_u8::<4>(input_vec);
        let result = veorq_u8(vqtbl1q_u8(low_tbl, low), vqtbl1q_u8(high_tbl, high));
        unsafe { vst1q_u8(out_chunk.as_mut_ptr(), result) };
    }
```

#### 6.3.5 标量尾部

```rust
    // 处理剩余 0..15 字节
    super::super::scalar::mul_slice_pure_rust(c, tail_input, tail_out);
}
```

### 6.4 `rust_neon_mul_slice_xor_impl` 详细分析

这是 `out[i] ^= c * input[i]` 的 NEON 实现，支持**可配置的展开因子**。

#### 6.4.1 展开因子选择

```rust
let unroll4 = {
    #[cfg(feature = "std")]
    { rust_neon_mul_slice_xor_unroll() != 2 }  // 默认 4，可通过环境变量设为 2
    #[cfg(not(feature = "std"))]
    { true }  // no_std 下默认 4
};

let bytes_done_unrolled = if unroll4 {
    input.len() & !63usize   // 展开 4:64 字节/迭代
} else {
    input.len() & !31usize   // 展开 2:32 字节/迭代
};
```

环境变量 `RS_NEON_MUL_SLICE_XOR_UNROLL` 可设为 `"2"` 或 `"4"`（默认 4）。

#### 6.4.2 展开 4 路径（64 字节/迭代）

```rust
if unroll4 {
    for (input_chunk, out_chunk) in unrolled_input
        .chunks_exact(64)
        .zip(unrolled_out.chunks_exact_mut(64))
    {
        let inputs: uint8x16x4_t = unsafe { vld1q_u8_x4(input_chunk.as_ptr()) };
        // ... 与 mul_slice 相同的半字节查表计算 ...

        let product0 = veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
        // ...

        // 关键区别：加载现有输出并 XOR
        let outs: uint8x16x4_t = unsafe { vld1q_u8_x4(out_chunk.as_ptr()) };
        unsafe {
            vst1q_u8_x4(
                out_chunk.as_mut_ptr(),
                uint8x16x4_t(
                    veorq_u8(outs.0, product0),
                    veorq_u8(outs.1, product1),
                    veorq_u8(outs.2, product2),
                    veorq_u8(outs.3, product3),
                ),
            )
        };
    }
}
```

#### 6.4.3 展开 2 路径（32 字节/迭代）

```rust
else {
    for (input_chunk, out_chunk) in unrolled_input
        .chunks_exact(32)
        .zip(unrolled_out.chunks_exact_mut(32))
    {
        let inputs: uint8x16x2_t = unsafe { vld1q_u8_x2(input_chunk.as_ptr()) };
        // ... 半字节查表计算 ...

        let outs: uint8x16x2_t = unsafe { vld1q_u8_x2(out_chunk.as_ptr()) };
        unsafe {
            vst1q_u8_x2(
                out_chunk.as_mut_ptr(),
                uint8x16x2_t(veorq_u8(outs.0, product0), veorq_u8(outs.1, product1)),
            )
        };
    }
}
```

### 6.5 使用的 NEON 内联函数汇总

| 内联函数 | 指令 | 用途 |
|---------|------|------|
| `vld1q_u8(ptr)` | `LD1 {V0.16B}, [ptr]` | 加载 16 字节到 NEON 寄存器 |
| `vld1q_u8_x2(ptr)` | `LD1 {V0.16B-V1.16B}, [ptr]` | 加载 32 字节（2 寄存器） |
| `vld1q_u8_x4(ptr)` | `LD1 {V0.16B-V3.16B}, [ptr]` | 加载 64 字节（4 寄存器） |
| `vst1q_u8(ptr, val)` | `ST1 {V0.16B}, [ptr]` | 存储 16 字节 |
| `vst1q_u8_x2(ptr, val)` | `ST1 {V0.16B-V1.16B}, [ptr]` | 存储 32 字节 |
| `vst1q_u8_x4(ptr, val)` | `ST1 {V0.16B-V3.16B}, [ptr]` | 存储 64 字节 |
| `vdupq_n_u8(imm)` | `DUP V0.16B, imm` | 将标量广播到所有 16 个通道 |
| `vandq_u8(a, b)` | `AND V0.16B, V1.16B, V2.16B` | 按位与（提取低半字节） |
| `vshrq_n_u8::<4>(a)` | `USHR V0.16B, V1.16B, #4` | 右移 4 位（提取高半字节） |
| `vqtbl1q_u8(tbl, idx)` | `TBL V0.16B, {V1.16B}, V2.16B` | 16 字节表查找 |
| `veorq_u8(a, b)` | `EOR V0.16B, V1.16B, V2.16B` | 按位异或（GF(2^8) 加法/结果组合） |

### 6.6 算法流程图

```
输入字节 b（假设 c 为乘法常数）
    │
    ├─→ b_lo = b & 0x0F          (vandq_u8)
    ├─→ b_hi = b >> 4            (vshrq_n_u8)
    │
    ├─→ r_lo = vqtbl1q_u8(MUL_TABLE_LOW[c], b_lo)
    ├─→ r_hi = vqtbl1q_u8(MUL_TABLE_HIGH[c], b_hi)
    │
    └─→ result = r_lo XOR r_hi   (veorq_u8)
         │
         ├─ mul_slice:     out = result
         └─ mul_slice_xor: out = out XOR result
```

### 6.7 性能剖析点注入

在 SIMD 实现内部注入了性能剖析点（`#[cfg(feature = "std")]` 守卫）：

```rust
#[cfg(feature = "std")]
{
    let vector_64b_chunks = bytes_done_unrolled / 64;
    let vector_16b_chunks = (bytes_done - bytes_done_unrolled) / 16;
    let tail_bytes = input.len() - bytes_done;
    RUST_NEON_PROFILE_METRICS.record_call(
        false,  // is_xor
        input.len(),
        vector_64b_chunks,
        vector_16b_chunks,
        tail_bytes,
    );
}
```

---

## 7. Leopard GF8 编解码器中的 NEON 加速

### 7.1 `lut_xor_neon` 函数

定义在 `src/core/leopard_gf8/ops.rs` 第 470-510 行，用于 Leopard FFT 蝶形运算中的查表 XOR 操作 `dst[i] ^= lut[src[i]]`。

```rust
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn lut_xor_neon(dst: &mut [u8], src: &[u8], lut: &[u8; 256]) {
    use core::arch::aarch64::{
        uint8x16_t, vandq_u8, vdupq_n_u8, vld1q_u8, vqtbl1q_u8,
        vshrq_n_u8, vst1q_u8, veorq_u8,
    };

    // 将 256 字节 LUT 分解为两个 16 字节半字节表
    let mut lut_low = [0u8; 16];
    let mut lut_high = [0u8; 16];
    lut_low.copy_from_slice(&lut[..16]);
    for i in 0..16 {
        lut_high[i] = lut[i * 16];
    }

    let low_tbl:  uint8x16_t = vld1q_u8(lut_low.as_ptr());
    let high_tbl: uint8x16_t = vld1q_u8(lut_high.as_ptr());
    let nibble_mask: uint8x16_t = vdupq_n_u8(0x0f);

    let (src16, src_tail) = src.as_chunks::<16>();
    let (dst16, dst_tail) = dst.as_chunks_mut::<16>();

    for (s_chunk, d_chunk) in src16.iter().zip(dst16.iter_mut()) {
        let sv = vld1q_u8(s_chunk.as_ptr());
        let dv = vld1q_u8(d_chunk.as_ptr());
        let lo = vandq_u8(sv, nibble_mask);
        let hi = vandq_u8(vshrq_n_u8::<4>(sv), nibble_mask);
        let product = veorq_u8(
            vqtbl1q_u8(low_tbl, lo),
            vqtbl1q_u8(high_tbl, hi),
        );
        vst1q_u8(d_chunk.as_mut_ptr(), veorq_u8(dv, product));
    }

    // 标量尾部（0-15 字节）
    for (d, s) in dst_tail.iter_mut().zip(src_tail.iter()) {
        *d ^= lut[*s as usize];
    }
}
```

### 7.2 `lut_xor` 分发器

```rust
#[inline]
fn lut_xor(dst: &mut [u8], src: &[u8], lut: &[u8; 256]) {
    #[cfg(target_arch = "aarch64")]
    {
        if dst.len() >= 16 {
            // aarch64 始终有 NEON，无需运行时检测
            unsafe { lut_xor_neon(dst, src, lut); }
            return;
        }
    }

    // 标量回退
    for (d, s) in dst.iter_mut().zip(src.iter()) {
        *d ^= lut[*s as usize];
    }
}
```

注意：在 aarch64 上，NEON 是基线指令集（所有 AArch64 处理器都支持），因此不需要运行时特征检测，直接使用即可。

### 7.3 `slice_xor` 的 aarch64 路径

在 `slice_xor` 函数中，aarch64 使用 `u64` 块 XOR 回退（而非专用 NEON 路径）：

```rust
pub(super) fn slice_xor(input: &[u8], out: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { slice_xor_avx2(input, out); }
            return;
        }
    }

    // aarch64 和非 AVX2 x86_64 使用 u64 块 XOR
    slice_xor_u64(input, out);
}
```

`slice_xor_u64` 每次迭代处理 64 字节（8 个 `u64`）：

```rust
fn slice_xor_u64(input: &[u8], out: &mut [u8]) {
    let (input64, input_tail64) = input.as_chunks::<64>();
    let (out64, out_tail64)     = out.as_chunks_mut::<64>();

    for (src, dst) in input64.iter().zip(out64.iter_mut()) {
        for i in 0..8 {
            let off = i * 8;
            let s = unsafe { core::ptr::read_unaligned(src[off..].as_ptr().cast::<u64>()) };
            let d = unsafe { core::ptr::read_unaligned(dst[off..].as_ptr().cast::<u64>()) };
            unsafe {
                core::ptr::write_unaligned(dst[off..].as_mut_ptr().cast::<u64>(), d ^ s);
            }
        }
    }
    // ... 8 字节块和标量尾部 ...
}
```

---

## 8. C SIMD 后端（Legacy FFI）

### 8.1 C 代码中的 NEON 支持

`simd_c/reedsolomon.c` 通过预处理器宏检测 NEON：

```c
#if ((defined(__ARM_NEON__) && __ARM_NEON__)      \
     || (defined(__ARM_NEON) && __ARM_NEON)       \
     || (defined(__aarch64__) && __aarch64__))
# define USE_ARM_NEON 1
# undef VECTOR_SIZE
# define VECTOR_SIZE 16
# include <arm_neon.h>
#else
# define USE_ARM_NEON 0
#endif
```

### 8.2 v128 联合体类型

C 代码定义了跨平台的 128 位向量类型：

```c
#define VSIZE 128
typedef union {
    T(uint8_t, u8);
    T(uint64_t, u64);
#if USE_ARM_NEON
    T1(uint8x16_t, uint8x16);
    T1(uint8x8x2_t, uint8x8x2);
#endif
    // ...
} v128 __attribute__((aligned(1)));
```

### 8.3 NEON 内联函数映射

| C 抽象函数 | NEON 实现 |
|-----------|----------|
| `load_v(in)` | `vld1q_u8(in)` |
| `set1_epi8_v(c)` | `vdupq_n_u8(c)` |
| `srli_epi64_v(in)` | `vshrq_n_u8(in, 4)` |
| `and_v(a, b)` | `vandq_u8(a, b)` |
| `xor_v(a, b)` | `veorq_u8(a, b)` |
| `shuffle_epi8_v(vec, mask)` | `vqtbl1q_u8(vec, mask)` 或 `vtbl2_u8` 回退 |
| `store_v(out, vec)` | `vst1q_u8(out, vec)` |

`shuffle_epi8_v` 有两条 NEON 路径：
- 优先使用 `vqtbl1q_u8`（需要 `RS_HAVE_VQTBL1Q_U8` 定义）
- 回退到 `vtbl2_u8` + `vcombine_u8`（兼容旧 ARM 处理器）

### 8.4 Rust FFI 绑定

`src/galois_8/legacy/simd_c.rs` 声明了 extern C 函数：

```rust
unsafe extern "C" {
    fn reedsolomon_gal_mul(
        low: *const u8, high: *const u8,
        input: *const u8, out: *mut u8, len: libc::size_t,
    ) -> libc::size_t;

    fn reedsolomon_gal_mul_xor(
        low: *const u8, high: *const u8,
        input: *const u8, out: *mut u8, len: libc::size_t,
    ) -> libc::size_t;
}
```

Rust 包装器处理标量尾部：

```rust
pub(crate) fn simd_c_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    let low  = &MUL_TABLE_LOW[c as usize][0];
    let high = &MUL_TABLE_HIGH[c as usize][0];
    let bytes_done = unsafe {
        reedsolomon_gal_mul(low, high, input.as_ptr(), out.as_mut_ptr(), input.len())
    } as usize;
    // 标量尾部
    scalar::mul_slice_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}
```

---

## 9. 并行执行策略（aarch64 专有）

### 9.1 aarch64 专有环境变量

定义在 `src/galois_8/policy.rs` 第 16-29 行：

| 环境变量 | 默认值 | 用途 |
|---------|--------|------|
| `RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES` | — | 触发并行重建的最小分片大小 |
| `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB` | — | 每个并行作业的最小字节数 |
| `RS_AARCH64_RECONSTRUCT_MAX_JOBS` | — | 最大并行作业数 |
| `RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB` | — | 数据分片重建每作业最小字节数 |
| `RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB` | — | 校验分片重建每作业最小字节数 |

### 9.2 aarch64 策略缓存构建

```rust
#[cfg(all(feature = "std", target_arch = "aarch64"))]
fn reconstruct_policy_cache_aarch64(base: ParallelPolicy) -> RuntimeParallelPolicyCache {
    let mut reconstruct_full_data = reconstruct_parallel_policy_default(base, false);

    // 应用 aarch64 专有覆盖
    if let Some(value) = parse_positive_env_usize(RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES_ENV) {
        reconstruct_full_data.min_parallel_shard_bytes = value;
    }
    if let Some(value) = parse_positive_env_usize(RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB_ENV) {
        reconstruct_full_data.min_bytes_per_job = value;
    }
    if let Some(value) = parse_positive_env_usize(RS_AARCH64_RECONSTRUCT_MAX_JOBS_ENV) {
        reconstruct_full_data.max_jobs = value;
    }

    // ... 构建 reconstruct_data 和 reconstruct_full_parity ...

    RuntimeParallelPolicyCache {
        data: base,
        reconstruct_data,
        reconstruct_full_data,
        reconstruct_full_parity,
    }
}
```

### 9.3 架构分发

```rust
#[cfg(all(feature = "std", not(target_arch = "aarch64")))]
pub(crate) fn resolve_runtime_parallel_policy_cache(base: ParallelPolicy) -> RuntimeParallelPolicyCache {
    // 非 aarch64：使用默认策略
    let reconstruct_data = reconstruct_parallel_policy_default(base, true);
    let reconstruct_full = reconstruct_parallel_policy_default(base, false);
    RuntimeParallelPolicyCache { /* ... */ }
}

#[cfg(all(feature = "std", target_arch = "aarch64"))]
pub(crate) fn resolve_runtime_parallel_policy_cache(base: ParallelPolicy) -> RuntimeParallelPolicyCache {
    // aarch64：使用专有策略覆盖
    reconstruct_policy_cache_aarch64(base)
}
```

这些环境变量允许运维人员针对 ARM 服务器芯片（如 AWS Graviton）调优并行粒度，因为 ARM 芯片的核心数和缓存特性可能与 x86 不同。

---

## 10. NEON 性能剖析指标

### 10.1 数据结构 (`src/galois_8/profile.rs`)

```rust
pub(crate) struct RustNeonProfileMetrics {
    mul_calls:       AtomicUsize,  // mul_slice 调用次数
    mul_xor_calls:   AtomicUsize,  // mul_slice_xor 调用次数
    total_bytes:     AtomicUsize,  // 处理总字节数
    vector_64b_chunks: AtomicUsize, // 64 字节 SIMD 块数
    vector_16b_chunks: AtomicUsize, // 16 字节 SIMD 块数
    tail_bytes:      AtomicUsize,  // 标量尾部字节数
    tail_calls:      AtomicUsize,  // 有尾部的调用次数
    table_lookups:   AtomicUsize,  // 半字节表查找总数
}

pub static RUST_NEON_PROFILE_METRICS: RustNeonProfileMetrics = RustNeonProfileMetrics {
    mul_calls: AtomicUsize::new(0),
    // ... 所有字段初始化为 0 ...
};
```

### 10.2 记录方法

```rust
pub(crate) fn record_call(
    &self,
    is_xor: bool,
    input_len: usize,
    vector_64b_chunks: usize,
    vector_16b_chunks: usize,
    tail_bytes: usize,
) {
    if is_xor {
        self.mul_xor_calls.fetch_add(1, Ordering::Relaxed);
    } else {
        self.mul_calls.fetch_add(1, Ordering::Relaxed);
    }
    self.total_bytes.fetch_add(input_len, Ordering::Relaxed);
    self.vector_64b_chunks.fetch_add(vector_64b_chunks, Ordering::Relaxed);
    self.vector_16b_chunks.fetch_add(vector_16b_chunks, Ordering::Relaxed);
    if tail_bytes > 0 {
        self.tail_calls.fetch_add(1, Ordering::Relaxed);
        self.tail_bytes.fetch_add(tail_bytes, Ordering::Relaxed);
    }
    // 每个 64 字节块有 8 次表查找（4 路 × 2 表），每个 16 字节块有 2 次
    let lookups = vector_64b_chunks.saturating_mul(8)
        .saturating_add(vector_16b_chunks.saturating_mul(2));
    self.table_lookups.fetch_add(lookups, Ordering::Relaxed);
}
```

### 10.3 可配置参数

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `RS_NEON_MUL_SLICE_XOR_UNROLL` | `"4"` | mul_slice_xor 展开因子（`"2"` 或 `"4"`） |
| `RS_NEON_MUL_SLICE_XOR_SCHEDULE` | — | 设为 `"split"` 启用分拆调度 |

```rust
pub(crate) fn rust_neon_mul_slice_xor_unroll() -> usize {
    static UNROLL: OnceLock<usize> = OnceLock::new();
    *UNROLL.get_or_init(|| {
        std::env::var(RS_NEON_MUL_SLICE_XOR_UNROLL_ENV)
            .ok()
            .as_deref()
            .and_then(parse_rust_neon_xor_unroll)
            .unwrap_or(4)
    })
}
```

### 10.4 公开 API

```rust
pub fn rust_neon_profile_stats() -> RustNeonProfileStats {
    RUST_NEON_PROFILE_METRICS.snapshot()
}

pub fn reset_rust_neon_profile_stats() {
    RUST_NEON_PROFILE_METRICS.reset();
}
```

`RustNeonProfileStats` 支持 `saturating_sub` 方法，用于差量分析：

```rust
pub fn saturating_sub(self, baseline: Self) -> Self {
    Self {
        mul_calls: self.mul_calls.saturating_sub(baseline.mul_calls),
        // ... 所有字段 ...
    }
}
```

---

## 11. 内存布局与对齐

### 11.1 AlignedShard (`src/galois_8/aligned.rs`)

```rust
pub const SHARD_ALIGNMENT: usize = 64;  // 64 字节对齐

pub struct AlignedShard {
    ptr: NonNull<u8>,
    len: usize,
}
```

64 字节对齐的选择理由：
- NEON：16 字节寄存器（64 = 4 × 16）
- AVX2:32 字节寄存器（64 = 2 × 32）
- AVX-512:64 字节寄存器（64 = 1 × 64）
- 现代 CPU 缓存行：通常 64 字节

### 11.2 分配实现

```rust
pub fn new_zeroed(len: usize) -> Self {
    if len == 0 {
        return Self { ptr: NonNull::dangling(), len: 0 };
    }

    let layout = Layout::from_size_align(len, SHARD_ALIGNMENT)
        .expect("aligned shard layout must be valid");
    let ptr = unsafe { alloc_zeroed(layout) };
    let ptr = NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout));
    Self { ptr, len }
}
```

### 11.3 安全性保证

```rust
unsafe impl Send for AlignedShard {}  // 所有权转移不会产生别名
unsafe impl Sync for AlignedShard {}  // 共享引用只暴露不可变 [u8]
```

### 11.4 NEON 实现中的对齐处理

在 `neon.rs` 中，对齐通过位运算保证：

```rust
let bytes_done          = input.len() & !15usize;  // 16 字节对齐
let bytes_done_unrolled = input.len() & !63usize;  // 64 字节对齐
```

使用 `chunks_exact(64)` 和 `chunks_exact(16)` 迭代器确保每次 SIMD 加载/存储都在精确大小的块上操作，避免越界访问。

---

## 12. SVE 预留扩展槽

### 12.1 占位实现 (`src/galois_8/aarch64/sve.rs`)

```rust
//! Reserved aarch64 SVE backend slot.
//!
//! This file intentionally does not provide an active implementation yet.
//! Its purpose is to make the aarch64 backend layout explicit so that a future
//! SVE backend can be added without reworking the NEON-oriented module split.

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub(crate) struct SveFeatureSet {
    pub available: bool,
}

pub(crate) fn detect_sve_features() -> SveFeatureSet {
    // SVE 检测和后端启用被有意推迟，直到具体实现就绪
    SveFeatureSet { available: false }
}
```

### 12.2 后端选择中的 SVE 占位

```rust
fn select_aarch64_backend(features: Aarch64FeatureSet) -> GaloisBackend {
    // SVE 已检测但尚未使用；预留给未来后端
    let _sve = features.sve;
    // ... 仅基于 neon 选择 ...
}
```

### 12.3 设计意图

这个模块的存在是为了：
1. 明确 aarch64 后端的模块布局
2. 使未来添加 SVE 后端时不需要重组 NEON 模块结构
3. 提供 SVE 特征检测的接口占位

---

## 13. 编码与解码流程

### 13.1 Classic 族编码

编码矩阵基于 Vandermonde 矩阵。对于每个数据分片 `i`，校验分片 `j` 的计算：

```
parity[j] = Σ (matrix[data_count + j][i] * data[i])  对所有 i
```

实现中：
- 第一个乘法使用 `mul_slice`（覆盖写入）
- 后续乘法使用 `mul_slice_xor`（累加写入）

### 13.2 单校验优化

当只有 1 个校验分片时，校验值是所有数据分片的 XOR（无需 GF 乘法）。

### 13.3 Leopard GF8 族编码

使用 NTT/FFT 变换实现高吞吐量编码，适用于分片数量较多的场景。核心操作：
1. FFT 正变换（`fwht8`、`fft_dit2_lut`、`fft_dit4_full_lut`）
2. 频域乘法（`mulgf8`）
3. FFT 逆变换（`ifft_dit2_lut`、`ifft_dit4_full_lut`）

所有查表 XOR 操作通过 `lut_xor` 分发，在 aarch64 上使用 `lut_xor_neon`。

### 13.4 重建流程

1. 收集有效和无效分片索引
2. 从编码矩阵中提取有效行的子矩阵
3. 通过高斯消元求逆得到解码矩阵
4. 使用 LRU 缓存解码矩阵（键为无效索引集合）
5. 用解码矩阵行乘以可用分片恢复丢失的数据分片
6. 重新编码恢复的数据分片以恢复丢失的校验分片

### 13.5 并行编码

使用 rayon 库实现并行：
- 按校验分片并行（`par_iter_mut`）
- 按数据块并行（分片内分块）

---

## 14. 架构层次总结

### 14.1 分层架构图

```
┌─────────────────────────────────────────────────────────────┐
│                    应用层                                     │
│         ReedSolomon<F>::encode / reconstruct / verify         │
├─────────────────────────────────────────────────────────────┤
│                    核心层                                     │
│      core/encode.rs  │  core/reconstruct.rs  │  core/verify.rs│
├─────────────────────────────────────────────────────────────┤
│                    字段 trait 层                              │
│         F::mul_slice / F::mul_slice_add (Field trait)         │
├─────────────────────────────────────────────────────────────┤
│                    字段实现层                                  │
│           galois_8/mod.rs: mul_slice() / mul_slice_xor()      │
├─────────────────────────────────────────────────────────────┤
│                    后端分发层                                  │
│        galois_8/backend.rs: active_backend().mul_slice         │
├──────────┬──────────────┬──────────────┬────────────────────┤
│          │              │              │                     │
│  scalar.rs    aarch64/neon.rs    x86/avx2.rs     legacy/simd_c.rs
│ (纯 Rust)   (Rust NEON 内联)   (Rust AVX2)      (C FFI NEON)
│     │              │                                   │
│     └── 尾部 ──────┘         ← 标量处理 0..15 剩余字节 →  │
└─────────────────────────────────────────────────────────────┘
```

### 14.2 关键设计特性

1. **架构无关的核心层**：`core/` 中的代码通过 `Field` trait 完全与目标架构解耦
2. **运行时后端选择**：通过 `spin::Once` 在进程启动时一次性选择最优后端
3. **编译时条件门控**：aarch64 专有代码通过 `#[cfg(target_arch = "aarch64")]` 隔离
4. **渐进式回退**：Rust NEON → C SIMD → 纯 Rust 标量
5. **标量尾部处理**：所有 SIMD 后端都委托标量回退处理 0..15 剩余字节
6. **64 字节对齐分配**：`AlignedShard` 确保 SIMD 友好的内存布局
7. **可配置展开因子**：`RS_NEON_MUL_SLICE_XOR_UNROLL` 允许调优 SIMD 循环展开
8. **aarch64 专有并行策略**：5 个环境变量允许针对 ARM 服务器调优并行粒度
9. **SVE 预留**：模块布局已为未来 SVE 后端做好准备
10. **性能剖析基础设施**：原子计数器追踪 SIMD 吞吐量、尾部开销、表查找次数

### 14.3 aarch64 特有代码的文件分布

所有 aarch64 特有代码集中在 **6 个文件**中：

| 文件 | 行数（约） | 职责 |
|------|-----------|------|
| `aarch64/mod.rs` | 19 | 模块声明 |
| `aarch64/neon.rs` | 265 | NEON 核心实现 |
| `aarch64/sve.rs` | 53 | SVE 占位桩 |
| `backend.rs`（条件段） | ~100 | 后端选择逻辑 |
| `policy.rs`（条件段） | ~65 | 并行策略覆盖 |
| `profile.rs`（条件段） | ~50 | 性能剖析 |

其余代码库对目标架构完全无感知。

---

## 附录 A：aarch64 编译命令

```bash
# 基本编译
cargo build --release --features simd-accel --target aarch64-unknown-linux-gnu

# 运行测试
cargo test --features simd-accel --target aarch64-unknown-linux-gnu

# 指定 C SIMD 架构
RUST_REED_SOLOMON_ERASURE_ARCH=armv8.2a+dotprod \
  cargo build --release --features simd-accel --target aarch64-unknown-linux-gnu
```

## 附录 B：环境变量速查表

| 环境变量 | 作用域 | 默认值 | 说明 |
|---------|--------|--------|------|
| `RSE_BACKEND_OVERRIDE` | 全局 | `auto` | 强制指定后端 |
| `RS_NEON_MUL_SLICE_XOR_UNROLL` | NEON | `4` | mul_slice_xor 展开因子 |
| `RS_NEON_MUL_SLICE_XOR_SCHEDULE` | NEON | — | 设为 `"split"` 启用分拆调度 |
| `RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES` | aarch64 | — | 并行重建最小分片大小 |
| `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB` | aarch64 | — | 每作业最小字节数 |
| `RS_AARCH64_RECONSTRUCT_MAX_JOBS` | aarch64 | — | 最大并行作业数 |
| `RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB` | aarch64 | — | 数据重建每作业最小字节数 |
| `RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB` | aarch64 | — | 校验重建每作业最小字节数 |

## 附录 C：NEON 内联函数与 AArch64 指令对照

| Rust 内联函数 | AArch64 汇编指令 | 功能描述 |
|--------------|-----------------|---------|
| `vld1q_u8` | `LD1 {Vn.16B}, [Xm]` | 从内存加载 16 字节到 NEON 寄存器 |
| `vld1q_u8_x2` | `LD1 {Vn.16B, Vn+1.16B}, [Xm]` | 加载 32 字节到两个 NEON 寄存器 |
| `vld1q_u8_x4` | `LD1 {Vn.16B-Vn+3.16B}, [Xm]` | 加载 64 字节到四个 NEON 寄存器 |
| `vst1q_u8` | `ST1 {Vn.16B}, [Xm]` | 存储 16 字节从 NEON 寄存器到内存 |
| `vst1q_u8_x2` | `ST1 {Vn.16B, Vn+1.16B}, [Xm]` | 存储 32 字节 |
| `vst1q_u8_x4` | `ST1 {Vn.16B-Vn+3.16B}, [Xm]` | 存储 64 字节 |
| `vdupq_n_u8` | `DUP Vn.16B, Wm` | 将标量广播到 16 个通道 |
| `vandq_u8` | `AND Vd.16B, Vn.16B, Vm.16B` | 128 位按位与 |
| `vshrq_n_u8::<4>` | `USHR Vd.16B, Vn.16B, #4` | 无符号右移 4 位 |
| `vqtbl1q_u8` | `TBL Vd.16B, {Vn.16B}, Vm.16B` | 单寄存器表查找（字节索引） |
| `veorq_u8` | `EOR Vd.16B, Vn.16B, Vm.16B` | 128 位按位异或 |
