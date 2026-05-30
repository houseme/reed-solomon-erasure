# Leopard GF8 x86_64 SIMD 优化结果

> 日期: 2026-05-30
> 平台: AMD EPYC 9V45 96-Core Processor (Zen 4c, Genoa)
> Rust: 1.96.0 (edition 2024)
> 基线: x86_64 纯 Rust (commit 320f7e2)

---

## 一、优化总结

### 1.1 实施的优化

| Phase | 优化项 | 文件 | 风险 | 结果 |
|-------|--------|------|------|------|
| A | AVX2 `slice_xor` | `ops.rs` | 低 | ✅ 保留 (功能正确, 无回归) |
| B | AVX2 `fft_dit4_full_lut` / `ifft_dit4_full_lut` | `ops.rs` | 中 | ✅ **核心优化, +71%~+200%** |
| B | AVX2 `fft_dit2_lut` / `ifft_dit2_lut` | `ops.rs` | 中 | ✅ 保留 (需 >= 32 字节阈值) |

### 1.2 回退的优化

| 优化项 | 原因 |
|--------|------|
| `lut_xor` 无阈值用于 dit2 | 小 slice (< 32 字节) 时 SIMD 开销 > 收益, 导致回归 |

**修复**: 添加 `dst.len() >= 32` 阈值检查, 小 slice 回退到标量。

### 1.3 关键技术: Nibble-Lookup SIMD

将 256 字节 LUT 分解为 2 × 16 字节 nibble 表:
```
lut_low[i]  = lut[i]      (i=0..15)
lut_high[i] = lut[i*16]   (i=0..15)
lut[byte]   = lut_low[byte & 0xf] ^ lut_high[byte >> 4]
```

AVX2 实现: `_mm256_shuffle_epi8` 处理 32 字节/迭代。

---

## 二、测试结果

| 测试套件 | passed | failed | ignored |
|---------|--------|--------|---------|
| `cargo test --lib --features std` | 198 | 0 | 4 |
| `cargo test --release --test benchmark_smoke --features std` | 27 | 0 | 0 |

**所有测试通过, 无回归。**

---

## 三、吞吐量对比

### 3.1 绝对吞吐量

| case | Baseline MB/s | SIMD MB/s | Speedup | aarch64 MB/s | vs aarch64 |
|------|--------------|-----------|---------|-------------|------------|
| 32x16_1m | 359.86 | **616.20** | **1.71x** | 420.60 | **1.46x** |
| 32x16_4m | 333.07 | **502.51** | **1.51x** | 422.28 | **1.19x** |
| 64x32_64k | 283.13 | **492.79** | **1.74x** | 324.17 | **1.52x** |
| 64x32_1m | 296.43 | **536.19** | **1.81x** | 341.80 | **1.57x** |
| 64x32_4m | 283.14 | **529.62** | **1.87x** | 350.58 | **1.51x** |
| 96x48_1m | 120.08 | **355.04** | **2.96x** | 125.35 | **2.83x** |
| 96x48_4m | 123.34 | **370.23** | **3.00x** | 134.98 | **2.74x** |
| 128x64_1m | 144.17 | **408.71** | **2.84x** | 153.48 | **2.66x** |
| 128x64_4m | 146.26 | **417.05** | **2.85x** | 163.16 | **2.56x** |

### 3.2 性能分析

| 指标 | 值 |
|------|-----|
| 最小 speedup | **1.51x** (32x16_4m) |
| 最大 speedup | **3.00x** (96x48_4m) |
| 平算 speedup | **~2.2x** |
| 与 aarch64 最小比 | **1.19x** (32x16_4m) |
| 与 aarch64 最大比 | **2.83x** (96x48_1m) |

**结论**: x86_64 SIMD 优化后, 所有 case 吞吐量均超过 aarch64 Apple M5 Max 的 119%-283%。

### 3.3 Speedup 与配置大小的关系

| 配置大小 | 平均 speedup | 说明 |
|---------|-------------|------|
| 32x16 (小) | 1.61x | 较小收益, 编译器自动向量化已部分覆盖 |
| 64x32 (中) | 1.81x | 中等收益 |
| 96x48 (大) | 2.98x | **最大收益**, 蝶形运算占比最高 |
| 128x64 (大) | 2.85x | 接近最大收益 |

**原因**: 大配置下 FFT 蝶形运算占比更高 (profile 显示 76%), SIMD 优化效果更显著。

---

## 四、Profile 对比

### 4.1 热点函数分布 (96x48_1m)

| 函数 | Baseline | SIMD 优化后 | 变化 |
|------|----------|------------|------|
| `dit4_at` | **61.80%** | **0%** | ✅ **完全消除** |
| `ifft_dit2` | 14.24% | 0% | ✅ 通过 SIMD 优化消除 |
| `lut_xor_avx2` | — | **9.75%** | 新增 SIMD 核心 |
| `slice_xor_avx2` | — | 1.04% | 新增 SIMD XOR |
| `Map::fold` | 12.31% | 19.06% | 相对占比上升 (绝对时间下降) |
| 页错误/内核 | ~5% | **~30%** | 内存分配成为新瓶颈 |

### 4.2 瓶颈迁移

```
优化前:  计算密集 (FFT 蝶形 76%) → 内存操作 ~12%
优化后:  计算高效 (SIMD LUT ~11%) → 内存密集 (~30% 页错误/分配)
```

**结论**: 计算瓶颈已被完全消除。新瓶颈是内存分配 (FlatWork 堆分配 + 页错误)。进一步优化应聚焦内存:
- 预分配 FlatWork buffer (避免重复堆分配)
- 使用 `mmap` 或大页减少页错误
- 零拷贝 FFT (直接在 shard buffer 上操作)

---

## 五、实现细节

### 5.1 新增函数

| 函数 | 位置 | 说明 |
|------|------|------|
| `lut_xor()` | `ops.rs` | LUT-XOR 调度器 (AVX2/标量) |
| `lut_xor_avx2()` | `ops.rs` | AVX2 nibble-lookup, 32 字节/迭代 |
| `slice_xor_avx2()` | `ops.rs` | AVX2 XOR, 32 字节/迭代 |

### 5.2 修改的函数

| 函数 | 修改内容 |
|------|---------|
| `slice_xor()` | 添加 AVX2 分支 (运行时检测) |
| `fft_dit4_full_lut()` | 用 `lut_xor` 替换逐元素标量循环 |
| `ifft_dit4_full_lut()` | 用 `lut_xor` 替换逐元素标量循环 |
| `fft_dit2_lut()` | 用 `lut_xor` 替换 (带 >= 32 字节阈值) |
| `ifft_dit2_lut()` | 用 `lut_xor` 替换 (带 >= 32 字节阈值) |

### 5.3 代码行数变化

| 文件 | 新增 | 修改 | 删除 |
|------|------|------|------|
| `ops.rs` | ~80 行 | ~30 行 | ~15 行 |

### 5.4 安全性

- 所有 SIMD intrinsics 使用 `#[target_feature(enable = "avx2")]`
- 运行时特性检测: `is_x86_feature_detected!("avx2")`
- 始终保留标量 fallback 路径
- 小 slice (< 32 字节) 自动回退标量
- Rust 2024 所有 unsafe intrinsics 使用显式 `unsafe {}` 块

---

## 六、结论

### 6.1 优化效果评估

| 评估项 | 结果 |
|--------|------|
| 功能正确性 | ✅ 198 测试全部通过 |
| 基准测试 | ✅ 27 测试全部通过 |
| 平均吞吞量提升 | **2.2x** |
| 最大吞吞量提升 | **3.0x** (96x48) |
| vs aarch64 | **1.19x - 2.83x** (x86_64 现已全面超越 aarch64) |
| 代码侵入性 | 低 (仅修改 ops.rs, 不改变数据结构) |

### 6.2 关键发现

1. **x86_64 的瓶颈与 aarch64 完全不同**: aarch64 是内存瓶颈 (~89%), x86_64 是计算瓶颈 (~76%)
2. **SIMD nibble-lookup 在 x86_64 上极其有效**: AVX2 `_mm256_shuffle_epi8` 性能优秀
3. **aarch64 上 NEON nibble-lookup 回归的原因不适用于 x86_64**: aarch64 的 FFT 仅占 8%, 优化收益小
4. **大配置收益最大**: 蝶形运算占比更高, SIMD 优化更显著

### 6.3 后续优化方向

| 优先级 | 方向 | 预期收益 | 难度 |
|--------|------|---------|------|
| 1 | FlatWork 预分配 (减少堆分配/页错误) | 10-30% | 低 |
| 2 | 零拷贝 FFT (直接在 shard buffer 操作) | 10-20% | 高 |
| 3 | GFNI 原生 GF 乘法 (仅 Ice Lake+) | 5-10% | 中 |
| 4 | aarch64 NEON nibble-lookup 重新评估 | -5%~+5% | 中 |

---

## 七、附录: 原始数据

### 7.1 Baseline (x86_64 纯 Rust, release)

```json
{"case":"32x16_1m","throughput_mb_s":359.8621}
{"case":"32x16_4m","throughput_mb_s":333.0722}
{"case":"64x32_64k","throughput_mb_s":283.1328}
{"case":"64x32_1m","throughput_mb_s":296.4319}
{"case":"64x32_4m","throughput_mb_s":283.1418}
{"case":"96x48_1m","throughput_mb_s":120.0826}
{"case":"96x48_4m","throughput_mb_s":123.3361}
{"case":"128x64_1m","throughput_mb_s":144.1681}
{"case":"128x64_4m","throughput_mb_s":146.256}
```

### 7.2 SIMD 优化后 (x86_64 AVX2, release)

```json
{"case":"32x16_1m","throughput_mb_s":616.2035}
{"case":"32x16_4m","throughput_mb_s":502.5122}
{"case":"64x32_64k","throughput_mb_s":492.7851}
{"case":"64x32_1m","throughput_mb_s":536.1879}
{"case":"64x32_4m","throughput_mb_s":529.6176}
{"case":"96x48_1m","throughput_mb_s":355.0362}
{"case":"96x48_4m","throughput_mb_s":370.2254}
{"case":"128x64_1m","throughput_mb_s":408.7143}
{"case":"128x64_4m","throughput_mb_s":417.0493}
```

### 7.3 aarch64 基线 (Apple M5 Max, commit d242272)

```json
{"case":"32x16_1m","throughput_mb_s":420.60}
{"case":"32x16_4m","throughput_mb_s":422.28}
{"case":"64x32_64k","throughput_mb_s":324.17}
{"case":"64x32_1m","throughput_mb_s":341.80}
{"case":"64x32_4m","throughput_mb_s":350.58}
{"case":"96x48_1m","throughput_mb_s":125.35}
{"case":"96x48_4m","throughput_mb_s":134.98}
{"case":"128x64_1m","throughput_mb_s":153.48}
{"case":"128x64_4m","throughput_mb_s":163.16}
```

---

*报告生成时间: 2026-05-30*
*工具: perf 6.17.13, Rust 1.96.0, cargo test --release*
