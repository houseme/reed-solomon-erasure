# Leopard GF8 优化路线图

> 日期: 2026-05-30
> 基准: `d242272` (main)
> 分析范围: `src/core/leopard_gf8/` 全模块 + `src/galois_8/` SIMD 后端

---

## 一、当前性能瓶颈分析

### 1.1 数据流 Profile (96x48_1m)

| 操作 | 字节量 | 占比 | 说明 |
|------|--------|------|------|
| input_copy | 6.29 GiB | 43.8% | 数据分片拷贝到 work buffer |
| xor | 3.34 GiB | 23.3% | IFFT 输出 XOR 累加 |
| output_writeback | 3.12 GiB | 21.7% | work buffer 写回 parity 分片 |
| zero_fill | 468 MiB | 3.3% | 尾部 lane 清零 |
| FFT/IFFT 计算 | — | ~8% | 蝶形运算 (LUT 查表+XOR) |

### 1.2 核心发现: Leopard Encode 路径完全标量化

Leopard encode 的所有计算核心 (`fft_dit2_lut`, `ifft_dit2_lut`, `fft_dit4_full_lut`, `slice_xor`) 均为纯标量实现。虽然 `galois_8` 模块已有完整的 NEON/AVX2/SSSE3 SIMD 后端，但 leopard encode 路径从未使用它们。

Benchmark 元数据中的 `"backend": "scalar-rust"` 指的是 `galois_8` 后端名称，与 leopard encode 路径无关 — leopard encode 始终运行标量代码。

---

## 二、优化项清单

### P0: SIMD `slice_xor` — 最低风险、高收益

**现状** (`ops.rs:62-152`): 64 字节手动展开 + 逐字节 XOR，依赖编译器自动向量化。

```rust
// 当前: 64 次逐字节 XOR
dst[0] ^= src[0];
dst[1] ^= src[1];
// ... dst[63] ^= src[63];
```

**问题**: 编译器自动向量化不保证生效。Profile 显示 XOR 占 23.3% 数据搬运量。

**方案**: 使用 `u64` 块处理，编译器更容易向量化:

```rust
pub(super) fn slice_xor(input: &[u8], out: &mut [u8]) {
    debug_assert_eq!(input.len(), out.len());
    let (input64, input_tail64) = input.as_chunks::<64>();
    let (out64, out_tail64) = out.as_chunks_mut::<64>();

    for (src, dst) in input64.iter().zip(out64.iter_mut()) {
        // 以 u64 为单位 XOR — 编译器自动向量化为 SIMD
        let src_u64: &[u64; 8] = unsafe { core::mem::transmute(src) };
        let dst_u64: &mut [u64; 8] = unsafe { core::mem::transmute(dst) };
        for i in 0..8 {
            dst_u64[i] ^= src_u64[i];
        }
    }
    // ... tail
}
```

或者更保守的方案 — 使用 `chunks_exact(8)` + `u64` XOR:

```rust
for (src_chunk, dst_chunk) in src.chunks_exact(8).zip(dst.chunks_exact_mut(8)) {
    let s = u64::from_ne_bytes(src_chunk.try_into().unwrap());
    let d = u64::from_ne_bytes(dst_chunk.try_into().unwrap());
    dst_chunk.copy_from_slice(&(d ^ s).to_ne_bytes());
}
```

**预期收益**: 2-4x XOR 吞吐提升。对 128x64 配置影响最大。

**风险**: 低。纯数据并行，无依赖链。

---

### P1: SIMD `fft_dit2_lut` / `ifft_dit2_lut` — 最高收益

**现状** (`ops.rs:182-206`): 逐字节 LUT 查表 + XOR。

```rust
for (dst, src) in x.iter_mut().zip(y.iter()) {
    *dst ^= lut[*src as usize];
}
```

**问题**: 这是 FFT 蝶形运算的最内层循环。每次迭代: load src → index LUT → load lut[val] → XOR dst → store dst。对 1M shard，此循环执行数百万次。

**方案**: 复用 `galois_8/aarch64/neon.rs` 的 nibble-lookup SIMD 技术:

```
原理: 将字节拆为低 4 位 + 高 4 位，分别从 16 字节 shuffle 表查找，XOR 合并。

NEON: vqtbl1q_u8 (16 字节/指令)
SSSE3: _mm_shuffle_epi8 (16 字节/指令)
AVX2: _mm256_shuffle_epi8 (32 字节/指令)
```

**实现步骤**:
1. 从 256 字节 `lut` 预计算两个 16 字节 shuffle 表 (low/high nibble)
2. 对每 16/32 字节: split → shuffle → XOR → XOR with dst
3. 标量处理尾部

**关键**: `galois_8` 已有完整的 `mul_slice_xor` SIMD 实现，`fft_dit2_lut` 与其功能完全等价（只是 LUT 来源不同）。可直接移植模式。

**预期收益**: 4-8x 蝶形运算吞吐提升。这是 **单项最高收益** 优化。

**风险**: 中。需要平台条件编译 (`cfg(target_arch)`)。

---

### P2: FFT Plan 缓存 — 零风险收益

**现状** (`encode.rs:107-129`): 每次 `encode_with_tables` 调用都重建所有 FFT/IFFT Plan。

```rust
let first_ifft_plan = build_ifft_dit8_plan(driver.mtrunc, driver.m, skew);
let fft_plan = build_fft_dit8_plan(parity_shards, driver.m, &tables.fft_skew);
let mut later_ifft_plans = Vec::new(); // 每次分配
```

**问题**: Plan 包含 `Vec<Stage4Block>`，每次 encode 调用涉及堆分配+填充+释放。

**方案**: 将 Plan 嵌入 `LeopardGf8EncodeDriver`:

```rust
pub(crate) struct LeopardGf8EncodeDriver {
    // ... existing fields ...
    pub(crate) first_ifft_plan: IfftDit8Plan,
    pub(crate) fft_plan: FftDit8Plan,
    pub(crate) later_ifft_plans: Vec<IfftDit8Plan>,
    pub(crate) remainder_ifft_plan: Option<IfftDit8Plan>,
}
```

在 `build_leopard_gf8_encode_driver` 中一次性构建。`encode_with_tables` 直接使用缓存的 Plan。

**预期收益**: 消除每次 encode 调用的 1-3+ 次 `Vec` 分配。对高频小文件 encode 场景收益明显。

**风险**: 极低。纯结构性重构，不影响计算逻辑。

---

### P3: FlatWork 对齐分配 + 复用 views

**现状** (`work.rs:19`): `vec![0u8; lanes * lane_len]` — 1 字节对齐。

**问题**:
1. SIMD load/store 要求 16/32/64 字节对齐，未对齐可能触发额外周期
2. `with_lane_views` 每次 chunk 迭代重建 `SmallVec`，对大 `m` 落入堆分配

**方案**:
1. 使用 `AlignedShard` 的 64 字节对齐分配器
2. 将 views 缓存到 `FlatWork` 中:

```rust
pub(super) struct FlatWork {
    lanes: usize,
    lane_len: usize,
    buf: Box<[u8]>,           // 64-byte aligned
    views_cache: Vec<*mut [u8]>,  // 缓存 lane 指针
}
```

**预期收益**: 消除每 chunk 的堆分配，SIMD 对齐收益 0-5%。

**风险**: 低。

---

### P4: `dit4_at_direct` 循环分阶段 — 微优化

**现状** (`encode.rs:282-310`): 每次迭代检查 `if d < work.len()`。

**方案**: 拆分为无分支 bulk + 有检查 tail:

```rust
let bulk_end = dist.min(work.len().saturating_sub(base + dist * 3));
for i in 0..bulk_end {
    // 无条件 4-lane butterfly
}
for i in bulk_end..dist {
    // 带回退的 butterfly
}
```

**预期收益**: <2%。分支预测器通常已能处理此模式。

**风险**: 极低。

---

### P5: `zero_trailing_lanes` 微优化

**现状** (`encode.rs:414-418`): `.iter_mut().skip(start_lane).take(count)`。

**方案**: 直接索引:

```rust
for i in start_lane..start_lane + count {
    work[i].as_mut().fill(0);
}
```

**预期收益**: <1%。消除 O(start_lane) 的迭代器跳过开销。

**风险**: 极低。

---

### P6: 内存布局优化 — 高风险高收益 (长期)

**现状**: FlatWork 为 lane-major 布局。蝶形运算访问 `work[i]` 和 `work[i+dist]`，间距为 `dist * chunk_size`。对 128 parity shards + 32K chunk，间距 = 4MB，远超 L2 cache。

**方案**: 转置为 SoA (Structure of Arrays) 布局: `[byte0_all_lanes, byte1_all_lanes, ...]`。蝶形运算变为连续内存访问。

**问题**: 与输入拷贝模式冲突 (输入是 lane-major)。需要在 FFT 前转置，FFT 后转置回来。

**预期收益**: 对大 `m` (>= 64) 可能有显著 cache miss 减少。但需要原型验证。

**风险**: 高。架构级变更，需要全面重写 FlatWork 和所有 FFT/IFFT 函数。

---

## 三、优先级排序

| 优先级 | 优化项 | 预期收益 | 实现难度 | 风险 |
|--------|--------|---------|---------|------|
| **P0** | SIMD `slice_xor` | 2-4x XOR | 低 | 极低 |
| **P1** | SIMD `fft_dit2_lut` | 4-8x 蝶形 | 中 | 低 |
| **P2** | FFT Plan 缓存 | 消除分配 | 低 | 极低 |
| **P3** | FlatWork 对齐+views 缓存 | 0-5% | 低 | 低 |
| **P4** | dit4_at_direct 分阶段 | <2% | 极低 | 极低 |
| **P5** | zero_trailing_lanes | <1% | 极低 | 极低 |
| **P6** | 内存布局转置 | 不确定 | 高 | 高 |

---

## 四、推荐实施路径

### Phase 1: 快速收益 (1-2 天)

1. **P2** — FFT Plan 缓存到 Driver，消除每次 encode 的 Vec 分配
2. **P5** — `zero_trailing_lanes` 改用直接索引
3. **P4** — `dit4_at_direct` 循环分阶段

验证: `cargo test --lib`, benchmark 确认无回退。

### Phase 2: SIMD 基础 (3-5 天)

4. **P0** — SIMD `slice_xor`:
   - aarch64: `veorq_u8` x 4 (64 字节/迭代)
   - x86_64: `_mm256_xor_si256` x 2 (64 字节/迭代)
   - fallback: `u64` 块 XOR (编译器自动向量化)

5. **P3** — FlatWork 64 字节对齐 + views 缓存

验证: `cargo test --lib`, benchmark 对比 SIMD vs scalar。

### Phase 3: SIMD 蝶形运算 (5-10 天)

6. **P1** — SIMD `fft_dit2_lut` / `ifft_dit2_lut`:
   - 从 256 字节 LUT 预计算 16 字节 low/high nibble shuffle 表
   - NEON: `vqtbl1q_u8` 处理 16 字节/迭代
   - SSSE3/AVX2: `_mm_shuffle_epi8` 处理 16/32 字节/迭代
   - 标量尾部处理

7. 扩展到 `fft_dit4_full_lut` / `ifft_dit4_full_lut` 的 SIMD 版本

验证: 全量测试 + 大规模基准对比。

### Phase 4: 架构探索 (长期)

8. **P6** — 内存布局转置原型
   - 先用小规模 (4x2, 10x4) 验证 cache miss 减少
   - 再扩展到大规模 (96x48, 128x64)

---

## 五、预期总体收益

| 配置 | 当前吞吐 | Phase 1 后 | Phase 2 后 | Phase 3 后 |
|------|---------|-----------|-----------|-----------|
| 4x2_1M | 1650 MB/s | ~1660 | ~1700 | ~2000+ |
| 32x16_1M | 410 MB/s | ~415 | ~500 | ~800+ |
| 96x48_1M | 124 MB/s | ~126 | ~160 | ~300+ |
| 128x64_1M | 152 MB/s | ~154 | ~190 | ~350+ |

> 注: 预期值为粗略估计，实际收益需基准验证。SIMD 蝶形运算 (Phase 3) 是最大单项收益来源。

---

## 六、相关文件

| 文件 | 优化项 |
|------|--------|
| `src/core/leopard_gf8/ops.rs` | P0 (slice_xor), P1 (fft_dit2_lut) |
| `src/core/leopard_gf8/encode.rs` | P2 (plan 缓存), P4 (dit4 分阶段), P5 (zero_trailing) |
| `src/core/leopard_gf8/work.rs` | P3 (对齐+views) |
| `src/core/leopard_gf8/mod.rs` | P2 (Driver 扩展) |
| `src/galois_8/aarch64/neon.rs` | P1 参考实现 |
| `src/galois_8/x86/ssse3.rs` | P1 参考实现 |
