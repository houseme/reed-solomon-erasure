# P1-3: GFNI 后端文档修正与评估 — 子任务详细文档

> **状态: ✅ 已完成** — GFNI+AVX2 和 GFNI+AVX-512 均已实现，含完整测试套件
> 文档日期: 2026-05-31
> 预估总工作量: 3-5 天
> 前置依赖: 无

---

## 概述

修正 GFNI 后端文档与代码的不一致：文档声称 "override-only"，代码实际自动选择。同时在真实 GFNI 硬件上进行性能验证。

## 文件影响范围

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/galois_8/backend.rs` | 修改 | 修正 doc comments |
| `docs/gfni-benchmark-results.md` | **新建** | 性能验证结果 |

---

## P1-3a: 文档修正

### P1-3a-1: 更新 doc comments

**目标**: 使 `backend.rs` 中的 doc comments 与代码行为一致

**文件**: `src/galois_8/backend.rs`

**当前代码** (line 303-304):
```rust
/// GFNI backends are override-only: never auto-selected due to limited deployment
/// and validation. Opt in via `RSE_BACKEND_OVERRIDE=rust-gfni-avx2`.
```

**实际行为** (line 391-394):
```rust
fn select_x86_backend(features: CpuFeatures) -> &'static GaloisBackend {
    if supports_rust_gfni_avx512(features) {
        return RUST_GFNI_AVX512_BACKEND; // 自动选择!
    }
    // ...
}
```

**修改为**:
```rust
/// GFNI backends are auto-selected on supporting hardware (Ice Lake+).
/// The priority order is: GFNI+AVX-512 > GFNI+AVX2 > AVX2 > AVX-512 > SSSE3.
/// Manual override is also available via `RSE_BACKEND_OVERRIDE=rust-gfni-avx2`.
```

**同时修正** line 385-390 的 `select_x86_backend` doc comment:
```rust
/// Selects the best available x86 SIMD backend.
///
/// Priority (highest to lowest):
/// 1. GFNI+AVX-512 — native GF(2^8) multiply, 512-bit (Ice Lake+)
/// 2. GFNI+AVX2 — native GF(2^8) multiply, 256-bit (Ice Lake+)
/// 3. AVX2 — nibble-lookup, 256-bit (preferred over AVX-512 for non-GFNI due to frequency throttling)
/// 4. AVX-512 — nibble-lookup, 512-bit
/// 5. SSSE3 — nibble-lookup, 128-bit
/// 6. SIMD-C — legacy C backend (requires `simd-accel` feature)
/// 7. Scalar — pure Rust fallback
```

**预估**: 0.5 天

---

## P1-3b: 性能验证

### P1-3b-1: 基准测试设计

**目标**: 设计 GFNI vs AVX2 的对比基准测试

**测试矩阵**:

| 配置 | shard_size | 操作 |
|------|------------|------|
| 10+4 | 4KB | encode |
| 10+4 | 64KB | encode |
| 10+4 | 1MB | encode |
| 10+4 | 4MB | encode |
| 12+4 | 1MB | encode |
| 16+4 | 1MB | encode |
| 10+4 | 1MB | reconstruct |
| 10+4 | 1MB | verify |

**测试方法**:
```bash
# GFNI
RSE_BACKEND_OVERRIDE=rust-gfni-avx2 cargo bench --bench galois_backend

# AVX2
RSE_BACKEND_OVERRIDE=rust-avx2 cargo bench --bench galois_backend
```

**需要**: Ice Lake 或更新的 CPU (支持 GFNI)

**预估**: 0.5 天

### P1-3b-2: 执行与记录

**目标**: 在 GFNI 硬件上执行基准测试并记录结果

**输出**: 原始基准数据

**预估**: 1 天 (含环境准备)

### P1-3b-3: 结果文档

**新建文件**: `docs/gfni-benchmark-results.md`

**内容**:
- 测试环境 (CPU 型号、OS、Rust 版本)
- GFNI vs AVX2 性能对比表
- 不同配置的性能差异分析
- 结论: GFNI 是否应该自动选择

**预估**: 0.5 天

---

## P1-3c: 策略决策

### P1-3c-1: 分析与决策

**目标**: 基于 P1-3b 的结果，决定 GFNI 的自动选择策略

**可能的决策**:

| 方案 | 条件 | 行动 |
|------|------|------|
| A: 保持现状 | GFNI 始终优于 AVX2 | 仅修正文档 |
| B: 条件选择 | GFNI 在某些配置下不如 AVX2 | 添加基于 shard_size 的条件 |
| C: 默认关闭 | GFNI 有兼容性问题 | 改回 override-only |

**输出**: 决策记录文档

**预估**: 1 天

---

## 依赖关系

```
P1-3a (独立，立即可做)
P1-3b-1 → P1-3b-2 → P1-3b-3 → P1-3c-1
```
