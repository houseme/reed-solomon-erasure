# P2-3: 细粒度 SIMD Feature Flags — 子任务详细文档

> 文档日期: 2026-05-31
> 预估总工作量: 2-3 天
> 前置依赖: 无

---

## 概述

当前仅有 `simd-accel` 一个 SIMD 相关 feature flag。需要添加更细粒度的控制，允许用户选择性启用/禁用特定 SIMD 后端。

---

## P2-3a: 方案设计

### P2-3a-1: flag 定义

**新增 feature flags**:

```toml
[features]
# 现有
default = ["std"]
std = ["parking_lot", "rayon"]
simd-accel = ["cc", "libc"]
benchmark-metrics = []

# 新增
simd-avx2 = []       # 启用 AVX2 Rust 后端
simd-avx512 = []     # 启用 AVX-512 Rust 后端
simd-gfni = []       # 启用 GFNI Rust 后端
simd-neon = []       # 启用 NEON Rust 后端
simd-vscode = []     # 启用 VSX Rust 后端 (ppc64le)
simd-all = ["simd-avx2", "simd-avx512", "simd-gfni", "simd-neon", "simd-vscode"]
```

**默认行为**: 所有 Rust SIMD 后端默认启用 (通过 `cfg(target_arch)` 和 `cfg(target_feature)` 自动选择)。新增 flags 仅用于**禁用**特定后端。

**设计决策**:
- 这些 flags 是**排除性**的: 默认启用，设置 flag 为 false 时禁用
- `simd-accel` 控制 C SIMD 后端的编译
- 新 flags 控制 Rust SIMD 后端的编译
- 运行时 `RSE_BACKEND_OVERRIDE` 仍然可用

**预估**: 0.5 天

### P2-3a-2: 兼容性分析

**检查项**:
- [ ] 现有 `simd-accel` 行为不变
- [ ] 默认构建不引入新的依赖
- [ ] CI 测试矩阵需要更新
- [ ] `no_std` 场景兼容

**预估**: 0.5 天

---

## P2-3b: 实现

### P2-3b-1: Cargo.toml 修改

**文件**: `Cargo.toml`

```toml
[features]
simd-avx2 = []
simd-avx512 = []
simd-gfni = []
simd-neon = []
simd-vscode = []
simd-all = ["simd-avx2", "simd-avx512", "simd-gfni", "simd-neon", "simd-vscode"]
```

**预估**: 0.5 天

### P2-3b-2: cfg guards 添加

**文件**: 各 SIMD 后端模块

```rust
// src/galois_8/x86/avx2.rs
#[cfg(all(
    target_arch = "x86_64",
    feature = "simd-avx2",  // 新增
))]
pub fn rust_avx2_mul_slice(...) { ... }

// src/galois_8/x86/avx512.rs
#[cfg(all(
    target_arch = "x86_64",
    feature = "simd-avx512",  // 新增
))]
pub fn rust_avx512_mul_slice(...) { ... }

// src/galois_8/aarch64/neon.rs
#[cfg(all(
    target_arch = "aarch64",
    feature = "simd-neon",  // 新增
))]
pub fn rust_neon_mul_slice(...) { ... }
```

**backend.rs 修改**: 在 `select_x86_backend` 和 `select_aarch64_backend` 中添加 feature 检查:
```rust
fn select_x86_backend(features: CpuFeatures) -> &'static GaloisBackend {
    #[cfg(feature = "simd-gfni")]
    if supports_rust_gfni_avx512(features) {
        return RUST_GFNI_AVX512_BACKEND;
    }

    #[cfg(feature = "simd-avx2")]
    if features.avx2 {
        return RUST_AVX2_BACKEND;
    }

    // ...
    SCALAR_BACKEND
}
```

**预估**: 1 天

### P2-3b-3: 构建验证

```bash
# 仅启用 AVX2
cargo build --features simd-avx2

# 禁用所有 SIMD
cargo build --no-default-features --features std

# 仅启用 NEON (aarch64)
cargo build --features simd-neon --target aarch64-unknown-linux-gnu
```

**预估**: 0.5 天

---

## P2-3c: 测试与文档

### P2-3c-1: 组合测试

```bash
# CI 中添加组合测试
cargo test --features "std,simd-avx2"
cargo test --features "std,simd-avx512"
cargo test --features "std,simd-gfni"
cargo test --features "std,simd-neon"
cargo test --features "std,simd-all"
cargo test --features "std"  # 无 SIMD
```

**预估**: 0.5 天

### P2-3c-2: README 更新

添加 feature flags 说明:
```markdown
## SIMD Feature Flags

| Flag | 说明 | 默认 |
|------|------|------|
| `simd-accel` | 启用 C SIMD 后端 | 否 |
| `simd-avx2` | 启用 AVX2 Rust 后端 | 是 (x86_64) |
| `simd-avx512` | 启用 AVX-512 Rust 后端 | 是 (x86_64) |
| `simd-gfni` | 启用 GFNI Rust 后端 | 是 (x86_64) |
| `simd-neon` | 启用 NEON Rust 后端 | 是 (aarch64) |
| `simd-all` | 启用所有 SIMD 后端 | 否 |
```

**预估**: 0.5 天

---

## 依赖关系

```
P2-3a-1 + P2-3a-2 → P2-3b-1 → P2-3b-2 → P2-3b-3 → P2-3c-1 + P2-3c-2
```
