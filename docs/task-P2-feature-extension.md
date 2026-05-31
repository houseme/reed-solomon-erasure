# P2 — 功能扩展任务

> 优先级：中 | 扩展平台支持和高级功能
> 预估总工作量：4-6 周

---

## 目录

- [P2-1: Leopard GF16 完整实现](#p2-1-leopard-gf16-完整实现)
- [P2-2: ppc64le SIMD 后端](#p2-2-ppc64le-simd-后端)
- [P2-3: 细粒度 SIMD Feature Flags](#p2-3-细粒度-simd-feature-flags)

---

## P2-1: Leopard GF16 完整实现

### 概述

Leopard GF16 支持高达 65,536 个分片，使用 GF(2^16) 域上的 FFT，复杂度 O(N log N)。当前 `LeopardGF16` variant 存在但所有操作返回 `UnsupportedLeopardPrototype`。

### 前置条件

- **P0-1 (Leopard GF8 完整编解码)** 必须先完成 — GF16 的解码算法与 GF8 同构，可复用架构

### Go 参考实现

`klauspost/reedsolomon` 的 Leopard GF16:
- 使用 GF(2^16) 域，元素为 `uint16`
- FFT/IFFT 使用与 GF8 相同的 DIT-4 蝶形策略
- 分片大小必须为 64 字节倍数
- 所有分片必须等长 (最后一个分片补齐)
- 仅单 goroutine

### 当前 Rust 状态

| 组件 | 状态 |
|------|------|
| `CodecFamily::LeopardGF16` | ✅ 定义存在 (`options.rs:4`) |
| `FamilyState::LeopardGF16` | ✅ 定义存在 (`leopard.rs`) |
| 编码 | ❌ 返回 `UnsupportedLeopardPrototype` |
| 解码/重建 | ❌ 返回 `UnsupportedLeopardPrototype` |
| 验证 | ❌ 返回 `UnsupportedLeopardPrototype` |
| GF(2^16) 域运算 | ✅ 已实现 (`galois_16.rs`) |

### 子任务拆分

#### P2-1a: Leopard GF16 表构建

**目标**: 构建 GF(2^16) 域上的 FFT 扭转因子、log/exp LUT、乘法 LUT

**新建文件**: `src/core/leopard_gf16/tables.rs`

**与 GF8 表的对应关系**:

| GF8 表 | GF16 表 | 大小 |
|--------|---------|------|
| `log_lut[256]` | `log_lut[65536]` | 64 KB |
| `exp_lut[256]` | `exp_lut[65536]` | 128 KB |
| `fft_skew[255]` | `fft_skew[65535]` | 128 KB |
| `log_walsh[256]` | `log_walsh[65536]` | 64 KB |
| `mul_luts[256]` | `mul_luts[65536]` | ~16 MB |

**挑战**: `mul_luts` 在 GF16 下需要 65536 个 65536 字节的乘法表，总大小约 4GB — 不可行。

**解决方案**: 使用 log-antilog 方案替代直接乘法表:
```
a * b = exp_lut[log_lut[a] + log_lut[b] (mod 65535)]
```
仅需 `log_lut[65536]` (64KB) + `exp_lut[65536]` (128KB) + `exp_lut` 的扩展版 (128KB)。

**SIMD 优化**: 对于 GF16 乘法，无法使用 nibble-lookup (元素是 16 位)。需要:
- 标量 log-antilog 乘法
- 或使用 SVE2 的 GF(2^16) 指令 (如果可用)
- 或查表 + 向量化 (将 16 位拆为高/低字节分别处理)

**预估**: 1 周

#### P2-1b: Leopard GF16 FFT/IFFT 实现

**目标**: 实现 GF(2^16) 域上的 DIT-4 蝶形 FFT/IFFT

**新建文件**: `src/core/leopard_gf16/fft.rs`

**算法**: 与 GF8 的 FFT 完全同构，只是域元素从 `u8` 变为 `u16`:
```
fft_dit2_gf16(a, b, skew):
    t = gf16_mul(a, skew)
    a = a ^ t  (GF 加法 = XOR)
    b = b ^ t

fft_dit4_gf16(a, b, c, d, skew0, skew1, skew2):
    fft_dit2_gf16(a, c, skew0)
    fft_dit2_gf16(b, d, skew1)
    fft_dit2_gf16(a, b, skew2)
    fft_dit2_gf16(c, d, skew2)
```

**复用**: `leopard_gf8/ops.rs` 中的蝶形运算框架可以泛化为 `<F: Field>` trait，但需要评估泛化开销。

**预估**: 1 周

#### P2-1c: Leopard GF16 编码实现

**目标**: 实现 Leopard GF16 编码

**新建文件**: `src/core/leopard_gf16/encode.rs`

**算法**: 与 GF8 编码同构:
1. 对 data shards 做 GF16 FFT
2. 截取前 parity_shards 个频域分量
3. IFFT 得到 parity shards

**限制**:
- 分片大小必须为 64 字节倍数
- 所有分片必须等长

**预估**: 1 周

#### P2-1d: Leopard GF16 解码/重建实现

**目标**: 实现 Leopard GF16 解码/重建

**新建文件**: `src/core/leopard_gf16/decode.rs`

**算法**: 与 GF8 解码同构，使用 Forney 算法在 GF(2^16) 上

**预估**: 1 周

#### P2-1e: 集成到公共 API

**目标**: 将 Leopard GF16 接入 `encode_sep`, `reconstruct`, `verify` 等公共方法

**修改文件**:
- `src/core/encode.rs` — 添加 `FamilyState::LeopardGF16` 分支
- `src/core/reconstruct.rs` — 同上
- `src/core/verify.rs` — 同上
- `src/core/leopard.rs` — 更新 `build_family_state` 支持 LeopardGF16

**预估**: 2-3 天

#### P2-1f: 测试与文档

**测试**:
- GF16 编码 roundtrip
- GF16 重建 (缺失 1, 多个, 边界)
- GF16 验证
- 大分片数测试 (如 1000 data + 100 parity)
- 限制验证 (非 64 字节倍数 → 错误)

**文档**:
- 在 README 中添加 Leopard GF16 使用说明
- 说明限制和性能特征

**预估**: 2-3 天

### 依赖关系

```
P0-1 (Leopard GF8) 完成后才能开始
P2-1a (表) → P2-1b (FFT) → P2-1c (编码) → P2-1d (解码) → P2-1e (集成) → P2-1f (测试)
```

---

## P2-2: ppc64le SIMD 后端

### 概述

Go 实现对 IBM POWER 架构有 AltiVec/VSX SIMD 优化，报告约 10 倍性能提升。Rust 实现目前在 ppc64le 上使用纯标量路径。

### 当前状态

| 组件 | 状态 |
|------|------|
| Rust `cfg(target_arch = "powerpc64")` | ❌ 不存在 |
| C SIMD (`simd_c/reedsolomon.c`) | ✅ 有 AltiVec 支持 |
| build.rs 编译支持 | ❌ 仅允许 x86_64/aarch64 |
| backend.rs dispatch | ❌ 无 ppc64le 分支 |

### 子任务拆分

#### P2-2a: 启用 C SIMD 的 ppc64le 编译

**目标**: 让 build.rs 允许 ppc64le 编译 C SIMD 代码

**修改文件**: `build.rs`

**修改**:
```rust
// build.rs:167-177
let arch_supported = matches!(
    target_arch.as_str(),
    "x86_64" | "aarch64" | "powerpc64"  // 添加 ppc64le
);
```

**验证**: 在 ppc64le 目标上编译并运行测试

**预估**: 1 天

#### P2-2b: 添加 Rust 原生 ppc64le SIMD 后端

**目标**: 实现基于 Rust intrinsics 的 ppc64le SIMD 后端

**新建文件**: `src/galois_8/ppc64le/`
- `mod.rs` — 模块定义
- `vsx.rs` — VSX (Vector Scalar Extension) 实现

**核心操作**: `mul_slice` 和 `mul_slice_xor` 使用 VSX intrinsics:
- `vec_ld` / `vec_st` — 向量加载/存储
- `vec_xor` — 向量 XOR
- `vec_perm` — 向量排列 (用于 nibble-lookup)
- `vec_sld` — 向量移位

**技术方案**: 使用与 x86/NEON 相同的 nibble-lookup 策略:
1. 将输入字节拆分为低 4 位和高 4 位
2. 使用 `vec_perm` 从预计算的 16 字节 LUT 中查找
3. XOR 合并结果

**预估**: 1-2 周

#### P2-2c: 后端注册与自动选择

**目标**: 在 backend.rs 中添加 ppc64le dispatch

**修改文件**: `src/galois_8/backend.rs`

**添加**:
```rust
#[cfg(target_arch = "powerpc64")]
fn select_ppc64le_backend(features: CpuFeatures) -> &'static GaloisBackend {
    if features.vsx {
        return RUST_VSX_BACKEND;
    }
    SCALAR_BACKEND
}
```

**自动选择优先级**: VSX > SIMD-C > Scalar

**预估**: 1-2 天

#### P2-2d: 测试

**测试**:
- VSX backend 正确性 (与 scalar 输出比对)
- 各种分片配置 roundtrip
- 性能基准测试

**预估**: 2-3 天

### 依赖关系

```
P2-2a (独立，可立即做)
P2-2b → P2-2c → P2-2d
```

---

## P2-3: 细粒度 SIMD Feature Flags

### 概述

当前仅有 `simd-accel` 一个 SIMD 相关 feature flag，粒度太粗。需要更细粒度的控制。

### Go 参考

Go 使用构建标签 `-tags=nopshufb` 移除所有 PSHUFB 等价指令。

### 子任务拆分

#### P2-3a: 定义 feature flag 方案

**目标**: 设计细粒度 SIMD feature flag 体系

**方案**:

```toml
[features]
# 现有
simd-accel = ["cc", "libc"]

# 新增细粒度控制
simd-avx2 = []       # 启用 AVX2 后端 (默认在 x86_64 上)
simd-avx512 = []     # 启用 AVX-512 后端
simd-gfni = []       # 启用 GFNI 后端
simd-neon = []       # 启用 NEON 后端 (默认在 aarch64 上)
simd-vscode = []     # 启用 VSX 后端 (ppc64le)
```

**设计决策**:
- 这些 flags 仅控制 Rust SIMD 后端的编译
- C SIMD 后端仍由 `simd-accel` 控制
- 运行时 `RSE_BACKEND_OVERRIDE` 仍然可用
- 默认行为不变 (自动检测)

**预估**: 1 天 (设计)

#### P2-3b: 实现条件编译

**目标**: 在 backend.rs 和各 SIMD 模块中添加 cfg guards

**修改文件**:
- `src/galois_8/backend.rs` — 使用 `cfg(feature = "simd-avx2")` 等
- `src/galois_8/x86/avx2.rs` — 添加 cfg guard
- `src/galois_8/x86/avx512.rs` — 同上
- `src/galois_8/x86/gfni.rs` — 同上
- `src/galois_8/aarch64/neon.rs` — 同上

**预估**: 2-3 天

#### P2-3c: 文档与测试

**文档**:
- 在 README 中说明各 feature flag 的用途
- 说明默认行为

**测试**:
- 仅启用 scalar → 编译通过
- 仅启用 avx2 → 编译通过 (x86_64)
- 组合启用 → 编译通过

**预估**: 1 天

### 依赖关系

```
P2-3a (设计) → P2-3b (实现) → P2-3c (测试)
```

---

## P2 整体里程碑

```
Week 1-2:  P2-1a (GF16 表) + P2-1b (GF16 FFT) + P2-3a (feature flag 设计)
Week 3-4:  P2-1c (GF16 编码) + P2-1d (GF16 解码) + P2-2a (ppc64le C SIMD)
Week 5:    P2-1e (GF16 集成) + P2-2b (ppc64le Rust SIMD)
Week 6:    P2-1f (GF16 测试) + P2-2c+P2-2d (ppc64le 集成测试) + P2-3b+P2-3c (feature flags)
```
