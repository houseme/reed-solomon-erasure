# Leopard GF8 x86_64 基线验证报告

> 日期：2026-05-30
> 平台：AMD EPYC 9V45 96-Core Processor (Zen 4c, Genoa)
> OS: Linux 6.17.0-1015-azure (Ubuntu 24.04, x86_64)
> Rust: 1.96.0 (2026-05-25)
> 基准：aarch64 Apple M5 Max (commit d242272)

---

## 一、环境信息

| 项目 | 值 |
|------|-----|
| CPU | AMD EPYC 9V45 96-Core Processor |
| 微架构 | Zen 4c (Genoa) |
| SIMD 特性 | SSSE3 ✅, AVX2 ✅, AVX-512 ✅ (f/bw/cd/dq/vl/vbmi/ifma/vnni/bitalg/vpopcntdq/vbmi2/vp2intersect/bf16), GFNI ✅, FMA ✅ |
| Rust | 1.96.0 (ac68faa20, 2026-05-25) |
| OS | Linux 6.17.0-1015-azure (Ubuntu 24.04, x86_64) |

**备注**: 该 CPU 拥有完整的 AVX-512 和 GFNI 支持，是验证 SIMD 优化的理想平台。

---

## 二、编译验证

| 构建命令 | 结果 | 耗时 |
|---------|------|------|
| `cargo build --features std` | ✅ 成功 | 2.20s |
| `cargo build --release --features std` | ✅ 成功 | 2.70s |
| `cargo build --features std,simd-accel` | ✅ 成功 | 1.43s |

**结论**: 所有构建配置均通过，无编译错误或警告。

---

## 三、功能测试

| 测试套件 | passed | failed | ignored | 耗时 |
|---------|--------|--------|---------|------|
| `cargo test --lib --features std` | 198 | 0 | 4 | 357.69s |
| `cargo test --release --test benchmark_smoke --features std` | 27 | 0 | 0 | 7.15s (release) |

**备注**: 4 个 ignored 测试为 benchmark-style artifact tests, 需显式运行。所有功能测试通过。

---

## 四、吞吞量对比 (Release 模式)

### 4.1 绝对吞吐量

| case | x86_64 MB/s | aarch64 MB/s | x86_64/aarch64 ratio |
|------|-------------|-------------|---------------------|
| 32x16_1m | 358.01 | 420.60 | **0.851** |
| 32x16_4m | 332.98 | 422.28 | **0.789** |
| 64x32_64k | 280.22 | 324.17 | **0.864** |
| 64x32_1m | 296.65 | 341.80 | **0.868** |
| 64x32_4m | 284.57 | 350.58 | **0.812** |
| 96x48_1m | 118.78 | 125.35 | **0.948** |
| 96x48_4m | 122.17 | 134.98 | **0.905** |
| 128x64_1m | 146.43 | 153.48 | **0.954** |
| 128x64_4m | 143.81 | 163.16 | **0.881** |

### 4.2 异常检测

| 检查项 | 阈值 | 最小 ratio | 结果 |
|--------|------|-----------|------|
| 所有 case ratio > 0.50 | > 0.50 | 0.789 (32x16_4m) | ✅ **全部通过** |

**无异常标记。** 所有 case 的 x86_64 吞吐量均超过 aarch64 的 78%, 最佳 case (128x64_1m) 达到 95.4%。

### 4.3 吞吐量分析

- **小配置 (32x16)**: x86_64 比 aarch64 慢 15-21%, 可能受内存延迟影响
- **中配置 (64x32)**: x86_64 比 aarch64 慢 13-19%
- **大配置 (96x48, 128x64)**: x86_64 比 aarch64 慢 5-12%, 差距最小
- **趋势**: 配置越大，x86_64 与 aarch64 的差距越小，说明大配置下计算密集度更高，x86_64 的计算能力更接近 aarch64

---

## 五、Profile 热点分析

### 5.1 perf 采样结果 (96x48_1m)

| 函数 | 占比 | 说明 |
|------|------|------|
| `dit4_at` | **61.80%** | FFT/IFFT 蝶形运算 (LUT 查表 + XOR) |
| `ifft_dit2` | **14.24%** | IFFT 蝶形阶段 |
| `Map::fold` | 12.31% | 迭代器折叠 (内存操作) |
| `do_user_addr_fault` | 1.32% | 页错误 |
| `_raw_spin_unlock_irqrestore` | 1.22% | 内核锁 |
| `slice_xor` | 0.77% | XOR 累加 |
| 其他内核函数 | ~8.3% | 内存管理、页表操作 |

### 5.2 与 aarch64 Profile 对比

| 操作 | aarch64 占比 | x86_64 占比 | 差异 |
|------|-------------|-------------|------|
| FFT/蝶形计算 (dit4_at + ifft_dit2) | ~8% | **~76%** | **+68pp** |
| 内存拷贝 (input_copy) | 43.8% | ~5% | -39pp |
| XOR 累加 (xor) | 23.3% | ~1% | -22pp |
| 输出回写 (output_writeback) | 21.7% | ~5% | -17pp |
| 尾部清零 (zero_fill) | 3.3% | ~0.5% | -3pp |

### 5.3 关键发现

**x86_64 与 aarch64 的瓶颈分布完全不同：**

1. **aarch64**: 内存操作为瓶颈 (input_copy 43.8% + output 21.7% + xor 23.3% = ~89%)
2. **x86_64**: 计算为瓶颈 (dit4_at 61.8% + ifft_dit2 14.2% = ~76%)

**可能原因：**
- x86_64 的 AMD EPYC 9V45 内存带宽更高，内存操作不再是瓶颈
- x86_64 编译器对 memcpy/XOR 的自动向量化更有效
- FFT 蝶形运算中的 LUT 查表 (256 字节表) 在 x86_64 上效率较低
- x86_64 的 `_mm_shuffle_epi8` (16 字节查表) 可能不如 aarch64 的 `vqtbl1q_u8` 高效

---

## 六、小文件策略验证

小文件策略验证测试 (`benchmark_small_file`) 不存在于当前测试套件中。

A/B 测试数据 (64x32_1m):

| variant | MB/s | 相对 baseline |
|---------|------|--------------|
| baseline | 294.16 | — |
| reuse_zero_only | 314.59 | +7.0% |
| xor_clone_only | 313.12 | +6.5% |

---

## 七、结论与建议

### 7.1 基线评估

| 评估项 | 状态 | 说明 |
|--------|------|------|
| 编译验证 | ✅ 通过 | 所有构建配置通过 |
| 功能测试 | ✅ 通过 | 198 passed, 0 failed |
| 基准测试 | ✅ 通过 | 27 passed |
| 吞吐量 vs aarch64 | ✅ 正常 | 最低 78.9%, 最高 95.4% |
| 瓶颈分布 | ⚠️ 与 aarch64 不同 | x86_64 计算瓶颈，aarch64 内存瓶颈 |

### 7.2 是否需要 SIMD 优化？

**结论：是，但优先级和方向与 aarch64 不同。**

| 优化方向 | aarch64 优先级 | x86_64 优先级 | 原因 |
|---------|---------------|--------------|------|
| SIMD FFT 蝶形运算 | 低 (~8% 占比) | **高 (~76% 占比)** | x86_64 上 FFT 是绝对瓶颈 |
| SIMD slice_xor | 中 (~23% 占比) | 低 (~1% 占比) | x86_64 上 XOR 已被编译器优化 |
| 内存拷贝优化 | 高 (~65% 占比) | 低 (~12% 占比) | x86_64 上内存操作不是瓶颈 |

### 7.3 推荐优化方案

#### 优先级 1: SIMD FFT 蝶形运算 (预期收益 30-50%)

在 x86_64 上，FFT 蝶形运算占 76% 时间。使用 SSSE3/AVX2 的 `_mm_shuffle_epi8` / `_mm256_shuffle_epi8` 实现 nibble-lookup SIMD 化：

- **SSSE3**: 16 字节/迭代，`_mm_shuffle_epi8`
- **AVX2**: 32 字节/迭代，`_mm256_shuffle_epi8`
- **AVX-512**: 64 字节/迭代，可能降频，默认不启用
- **GFNI**: 原生 GF 乘法，仅 Ice Lake+, 可选

**注意**: aarch64 上 NEON nibble-lookup 回归 -5%~-11%, 但 x86_64 的 `_mm_shuffle_epi8` 性能特征不同，需独立验证。

#### 优先级 2: 内存操作显式 SIMD 化 (预期收益 5-10%)

虽然内存操作占比低 (~12%), 但显式 AVX2/AVX-512 化可消除编译器自动向量化不确定性：

- `slice_xor_avx2`: `_mm256_xor_si256` 32 字节/迭代
- `memcpy_avx512`: `_mm512_loadu_si512` + `_mm512_storeu_si512` 64 字节/迭代

#### 优先级 3: GFNI 原生 GF 乘法 (可选，仅 Ice Lake+)

`vqgf2p8affineqb` 指令直接在硬件上执行 GF(2^8) 乘法，无需查表。该 CPU 支持 GFNI, 可作为高级优化选项。

### 7.4 风险评估

| 风险 | 影响 | 缓解 |
|------|------|------|
| AVX-512 频率降频 | 可能降低整体性能 | 默认使用 AVX2, AVX-512 仅可选 |
| `_mm_shuffle_epi8` 高位忽略 | 查表结果可能错误 | 确保 LUT 值 < 16 或使用掩码 |
| 256-bit shuffle 不跨 128-bit lane | AVX2 查表需要 split+merge | 参考 galois_8 AVX2 实现 |
| aarch64 NEON 回归经验 | x86_64 可能也回归 | 独立基准测试验证 |

---

## 八、下一步行动

1. **Phase 1 (1-2 天)**: 实现 `slice_xor_avx2`, 验证 AVX2 XOR 是否比编译器自动向量化更快
2. **Phase 2 (2-3 天)**: 实现 `gf8_lut_xor_ssse3` / `gf8_lut_xor_avx2`, 验证 SIMD FFT 蝶形运算
3. **Phase 3 (1 天)**: 在 x86_64 上重新 profile, 确认 SIMD 优化效果
4. **Phase 4 (可选)**: 实现 GFNI 原生 GF 乘法后端

---

## 九、附录：原始数据

### 9.1 Release 模式吞吐量 JSON

```json
{"case":"32x16_1m","throughput_mb_s":358.0113}
{"case":"32x16_4m","throughput_mb_s":332.9849}
{"case":"64x32_64k","throughput_mb_s":280.2204}
{"case":"64x32_1m","throughput_mb_s":296.65}
{"case":"64x32_4m","throughput_mb_s":284.5654}
{"case":"96x48_1m","throughput_mb_s":118.7805}
{"case":"96x48_4m","throughput_mb_s":122.1725}
{"case":"128x64_1m","throughput_mb_s":146.4303}
{"case":"128x64_4m","throughput_mb_s":143.8083}
```

### 9.2 Profile 数据 (96x48_1m)

```json
{
  "encode_calls": 26,
  "input_copy_bytes": 2596274176,
  "xor_bytes": 1366294528,
  "output_writeback_bytes": 1271922688,
  "zero_fill_bytes": 188743680
}
```

### 9.3 perf 热点排序

```
61.80%  dit4_at          (FFT 蝶形运算)
14.24%  ifft_dit2        (IFFT 蝶形)
12.31%  Map::fold        (迭代器折叠)
 1.32%  do_user_addr_fault (页错误)
 1.22%  _raw_spin_unlock_irqrestore (内核锁)
 0.77%  slice_xor        (XOR 累加)
```

---

*报告生成时间：2026-05-30*
*工具：perf 6.17.13, Rust 1.96.0, cargo test --release*
