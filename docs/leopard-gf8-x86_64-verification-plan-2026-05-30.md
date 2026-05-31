# Leopard GF8 x86_64 架构验证与优化方案

> 日期：2026-05-30
> 基准：aarch64 Apple M5 Max 优化结果 (5 commits on main)
> 目标：在 x86_64 架构上验证已有优化、评估 SIMD 加速可行性

---

## 一、当前状态总结 (aarch64 基线)

### 1.1 已应用优化

| Commit | 优化项 | 效果 |
|--------|--------|------|
| `d242272` | 自适应 dit4 策略 (auto/decomposed/direct) | 小文件 +126%, 策略选择 100% 正确 |
| `142c0ac` | P5: zero_trailing 直接索引 + P4: dit4 分阶段 | 微优化，消除迭代器开销 |
| `d916098` | P0: slice_xor u64 块处理 | 编译器更易向量化为 SIMD |
| `950ab25` | P3: FlatWork 64 字节对齐分配 | SIMD 对齐，消除 SmallVec 堆分配 |

### 1.2 回退优化

| 优化项 | 原因 |
|--------|------|
| P1: NEON nibble-lookup fft_dit2_lut | 回归 -5%~-11%, FFT 仅占 ~8% 总时间 |

### 1.3 性能 Profile (aarch64, 96x48_1m)

| 操作 | 占比 | 说明 |
|------|------|------|
| input_copy | 43.8% | 数据分片拷贝到 FlatWork |
| xor 累加 | 23.3% | IFFT 输出 XOR |
| output_writeback | 21.7% | FlatWork 写回 parity |
| zero_fill | 3.3% | 尾部 lane 清零 |
| FFT/IFFT 计算 | ~8% | 蝶形运算 (LUT 查表+XOR) |

---

## 二、x86_64 架构特性分析

### 2.1 可用 SIMD 指令集

| 指令集 | 最低 CPU | 寄存器宽度 | `_mm_shuffle_epi8` | `_mm256_shuffle_epi8` | GFNI |
|--------|---------|-----------|-------------------|----------------------|------|
| SSSE3 | Core 2 (2006) | 128-bit | ✅ | — | — |
| AVX2 | Haswell (2013) | 256-bit | ✅ | ✅ | — |
| AVX-512 | Skylake-X (2017) | 512-bit | ✅ | ✅ | — |
| GFNI | Ice Lake (2019) | 128/256/512-bit | ✅ | ✅ | ✅ |

### 2.2 关键指令对比 (aarch64 vs x86_64)

| 操作 | aarch64 NEON | x86_64 SSSE3 | x86_64 AVX2 |
|------|-------------|-------------|-------------|
| 16 字节表查找 | `vqtbl1q_u8` | `_mm_shuffle_epi8` | `_mm256_shuffle_epi8` |
| 32 字节表查找 | — (需 2 次) | — (需 2 次) | `_mm256_shuffle_epi8` |
| XOR | `veorq_u8` | `_mm_xor_si128` | `_mm256_xor_si256` |
| Load | `vld1q_u8` | `_mm_loadu_si128` | `_mm256_loadu_si256` |
| Store | `vst1q_u8` | `_mm_storeu_si128` | `_mm256_storeu_si256` |
| Nibble mask | `vandq_u8` + `vshrq_n_u8` | `_mm_and_si128` + `_mm_srli_epi64` | `_mm256_and_si256` + `_mm256_srli_epi64` |

### 2.3 x86_64 vs aarch64 差异

| 维度 | aarch64 | x86_64 |
|------|---------|--------|
| SIMD 宽度 | 128-bit 固定 | 128/256/512-bit 可选 |
| 特性检测 | 编译时 `cfg(target_feature)` | 运行时 `is_x86_feature_detected!` |
| 对齐要求 | 宽松 (NEON 支持未对齐) | 严格 (AVX 要求 32 字节对齐) |
| 频率节流 | 无 | AVX-512 可能降频 |
| 编译器自动向量化 | 较积极 | 较保守 (需明确 intrinsics) |

---

## 三、galois_8 现有 x86_64 SIMD 基础设施

### 3.1 已实现后端

| 后端 | 文件 | 函数 | 算法 |
|------|------|------|------|
| SSSE3 | `src/galois_8/x86/ssse3.rs` | `rust_ssse3_mul_slice_xor` | 16 字节 nibble-lookup |
| AVX2 | `src/galois_8/x86/avx2.rs` | `rust_avx2_mul_slice_xor` | 32 字节 nibble-lookup |
| AVX-512 | `src/galois_8/x86/avx512.rs` | `rust_avx512_mul_slice_xor` | 64 字节 nibble-lookup |
| GFNI | `src/galois_8/x86/gfni.rs` | `rust_gfni_avx2_mul_slice_xor` | 原生 GF 乘法 |

### 3.2 可复用基础设施

| 组件 | 位置 | 复用方式 |
|------|------|---------|
| Nibble 表构建 | `build.rs` (`MUL_TABLE_LOW/HIGH`) | leopard_gf8 LUT 也可预计算 nibble 表 |
| 运行时检测 | `backend.rs` (`is_x86_feature_detected!`) | 相同模式用于 leopard_gf8 后端选择 |
| 后端覆盖 | `RSE_BACKEND_OVERRIDE` env var | 统一后端选择机制 |
| 性能指标 | `RustNeonProfileStats` | 扩展到 x86_64 profile |

### 3.3 不可直接复用的部分

| 组件 | 原因 |
|------|------|
| `mul_slice_xor` 函数 | leopard_gf8 的蝶形运算是 4 路同时操作，非单路 mul_slice |
| Nibble 分解算法 | leopard_gf8 需要 3 个不同 LUT 同时查表，非单 LUT |
| C SIMD 后端 | `simd_c/reedsolomon.c` 是 galois_8 专用 |

---

## 四、leopard_gf8 x86_64 SIMD 优化方案

### 4.1 方案 A: Nibble-Lookup SIMD (与 aarch64 NEON 同策略)

**适用函数**: `fft_dit2_lut`, `ifft_dit2_lut`, `fft_dit4_full_lut`, `ifft_dit4_full_lut`

**原理**: 将 256 字节 LUT 分解为 16 字节 low/high nibble 表：
```
lut_low[j]  = lut[j]      (j=0..15)
lut_high[j] = lut[j*16]   (j=0..15)
lut[byte]   = lut_low[byte & 0xf] ^ lut_high[byte >> 4]
```

**SSSE3 实现** (16 字节/迭代):
```rust
#[target_feature(enable = "ssse3")]
unsafe fn gf8_lut_xor_ssse3(dst: &mut [u8], src: &[u8], lut: &[u8; 256]) {
    let low_tbl  = _mm_loadu_si128(lut.as_ptr() as *const __m128i);
    let high_tbl = _mm_loadu_si128(lut.as_ptr().add(0) as *const __m128i);
    // 构建 high_tbl: lut[0], lut[16], lut[32], ..., lut[240]
    let nibble_mask = _mm_set1_epi8(0x0f);

    for (s_chunk, d_chunk) in src.chunks_exact(16).zip(dst.chunks_exact_mut(16)) {
        let s = _mm_loadu_si128(s_chunk.as_ptr() as *const __m128i);
        let d = _mm_loadu_si128(d_chunk.as_ptr() as *const __m128i);
        let lo = _mm_and_si128(s, nibble_mask);
        let hi = _mm_and_si128(_mm_srli_epi64(s, 4), nibble_mask);
        let product = _mm_xor_si128(
            _mm_shuffle_epi8(low_tbl, lo),
            _mm_shuffle_epi8(high_tbl, hi),
        );
        _mm_storeu_si128(d_chunk.as_mut_ptr() as *mut __m128i, _mm_xor_si128(d, product));
    }
    // scalar tail
}
```

**AVX2 实现** (32 字节/迭代):
```rust
#[target_feature(enable = "avx2")]
unsafe fn gf8_lut_xor_avx2(dst: &mut [u8], src: &[u8], lut: &[u8; 256]) {
    let low_tbl  = _mm256_broadcastsi128_si256(_mm_loadu_si128(...));
    let high_tbl = _mm256_broadcastsi128_si256(_mm_loadu_si128(...));
    let nibble_mask = _mm256_set1_epi8(0x0f);
    // 32 字节/迭代
}
```

**aarch64 回归风险**: 已验证 NEON nibble-lookup 在 Apple M5 Max 上回归 -5%~-11%。x86_64 的 `_mm_shuffle_epi8` 与 `vqtbl1q_u8` 性能特征不同，需独立验证。

**预期收益**: FFT 计算仅占 ~8% 总时间，即使 4x 加速，总体收益 ~2-3%。

### 4.2 方案 B: SIMD `slice_xor` 显式实现

**当前状态**: 依赖编译器自动向量化 u64 XOR。

**x86_64 显式实现**:
```rust
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn slice_xor_avx2(input: &[u8], out: &mut [u8]) {
    let (in32, in_tail) = input.as_chunks::<32>();
    let (out32, out_tail) = out.as_chunks_mut::<32>();

    for (src, dst) in in32.iter().zip(out32.iter_mut()) {
        let s = _mm256_loadu_si256(src.as_ptr() as *const __m256i);
        let d = _mm256_loadu_si256(dst.as_ptr() as *const __m256i);
        _mm256_storeu_si256(dst.as_mut_ptr() as *mut __m256i, _mm256_xor_si256(d, s));
    }
    // scalar tail
}
```

**预期收益**: 消除编译器自动向量化不确定性。在 x86_64 上编译器可能不如 aarch64 积极向量化。

### 4.3 方案 C: 内存拷贝优化 (最大瓶颈)

**当前瓶颈**: input_copy 占 43.8%, output_writeback 占 21.7%。

**优化方向**:
1. **零拷贝 FFT**: 直接在输入 shard buffer 上做 FFT, 消除到 FlatWork 的拷贝
2. **SoA 内存布局**: 转置为 `[byte0_all_lanes, byte1_all_lanes, ...]`, 蝶形运算变连续访问
3. **Prefetch 提示**: `_mm_prefetch` 预取下一个 chunk

**风险**: 高。架构级变更，需要全面重写。

### 4.4 方案 D: GFNI 原生 GF 乘法 (仅限 Ice Lake+)

**原理**: `vqgf2p8affineqb` 指令直接在硬件上执行 GF(2^8) 乘法，无需查表。

```rust
#[target_feature(enable = "avx512f,avx512bw,gfni")]
unsafe fn gf8_mul_gfni(dst: &mut [u8], src: &[u8], coeff: u8) {
    let coeff_vec = _mm512_set1_epi8(coeff as i8);
    for (s, d) in src.chunks_exact(64).zip(dst.chunks_exact_mut(64)) {
        let sv = _mm512_loadu_si512(s.as_ptr() as *const __m512i);
        let dv = _mm512_loadu_si512(d.as_ptr() as *const __m512i);
        let product = _mm512_gf2p8affine_epi64_epi8(sv, coeff_vec, 0);
        _mm512_storeu_si512(d.as_mut_ptr() as *mut __m512i, _mm512_xor_si512(dv, product));
    }
}
```

**优势**: 消除 LUT 查表，64 字节/迭代。
**劣势**: 仅 Ice Lake+ (2019+), GFNI 在 galois_8 中从未自动选择。

---

## 五、验证方案

### 5.1 环境准备

#### 5.1.1 硬件要求

| 测试目标 | 最低 CPU | 推荐 CPU | 说明 |
|---------|---------|---------|------|
| 基线验证 | 任意 x86_64 | — | 确认编译 + 测试通过 |
| SSSE3 测试 | Core 2+ | — | `_mm_shuffle_epi8` |
| AVX2 测试 | Haswell+ | Intel 12th+ / AMD Zen 3+ | `_mm256_shuffle_epi8` |
| AVX-512 测试 | Skylake-X+ | Intel Sapphire Rapids+ | 频率节流需注意 |
| GFNI 测试 | Ice Lake+ | Intel 11th+ | 仅覆盖验证 |

#### 5.1.2 CI 环境配置

```yaml
# .github/workflows/x86_64-verify.yml
name: x86_64 Verification
on: [push, pull_request]
jobs:
  verify:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-gnu
          - x86_64-apple-darwin
          - x86_64-pc-windows-msvc
        features:
          - "std"
          - "std,simd-accel"
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --lib --features ${{ matrix.features }}
      - run: cargo test --test benchmark_smoke --features ${{ matrix.features }}
```

#### 5.1.3 本地 x86_64 交叉编译 (从 aarch64)

```bash
# 安装 target
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-apple-darwin

# 交叉编译 (仅检查编译, 无法运行)
cargo check --target x86_64-unknown-linux-gnu --features "std,simd-accel"
cargo check --target x86_64-apple-darwin --features "std,simd-accel"
```

### 5.2 测试矩阵

#### 5.2.1 编译验证

| 测试项 | 命令 | 期望 |
|--------|------|------|
| 默认编译 | `cargo build --features std` | ✅ 通过 |
| simd-accel 编译 | `cargo build --features std,simd-accel` | ✅ 通过 |
| release 编译 | `cargo build --release --features std` | ✅ 通过 |
| release+simd | `cargo build --release --features std,simd-accel` | ✅ 通过 |

#### 5.2.2 功能测试

| 测试项 | 命令 | 期望 |
|--------|------|------|
| 全量单元测试 | `cargo test --lib --features std` | 199 passed |
| 全量+simd | `cargo test --lib --features std,simd-accel` | 199 passed |
| leopard 专项 | `cargo test --lib --features std -- leopard` | 全部通过 |
| 基准冒烟 | `cargo test --test benchmark_smoke --features std` | 27 passed |

#### 5.2.3 性能基准

| 配置 | shard_size | 测试内容 | 期望 |
|------|-----------|---------|------|
| 4x2 | 1K-1M | 小文件 encode | auto 策略选择正确 |
| 10x4 | 1K-1M | 小文件 encode | auto 策略选择正确 |
| 32x16 | 1M, 4M | 大文件 encode | 吞吐量记录 |
| 64x32 | 64K, 1M, 4M | 大文件 encode | 吞吐量记录 |
| 96x48 | 1M, 4M | 大文件 encode | 吞吐量记录 |
| 128x64 | 1M, 4M | 大文件 encode | 吞吐量记录 |

### 5.3 验证脚本

```bash
#!/bin/bash
# scripts/verify-x86_64.sh
# 在 x86_64 机器上运行完整验证

set -euo pipefail

echo "=== Step 1: CPU 信息 ==="
cat /proc/cpuinfo | grep -E "model name|flags" | head -2
echo ""

echo "=== Step 2: 编译验证 ==="
cargo build --features std 2>&1
cargo build --release --features std 2>&1
echo "✅ 编译通过"
echo ""

echo "=== Step 3: 功能测试 ==="
cargo test --lib --features std 2>&1
echo "✅ 功能测试通过"
echo ""

echo "=== Step 4: 基准冒烟测试 ==="
cargo test --test benchmark_smoke --features std -- --nocapture 2>&1
echo "✅ 基准测试通过"
echo ""

echo "=== Step 5: 收集结果 ==="
echo "Leopard encode 结果:"
for f in target/benchmark-smoke/leopard-encode-*.json; do
    case=$(basename "$f" .json)
    throughput=$(python3 -c "import json; print(json.load(open('$f'))['throughput_mb_s'])" 2>/dev/null || echo "N/A")
    echo "  $case: $throughput MB/s"
done
echo ""

echo "=== Step 6: SIMD 特性检测 ==="
python3 -c "
import json, os
# 检查 galois_8 后端选择
print('galois_8 后端: 通过 benchmark 结果中的 backend 字段确认')
for f in ['target/benchmark-smoke/smoke-results.json']:
    if os.path.exists(f):
        data = json.load(open(f))
        if isinstance(data, list) and len(data) > 0:
            print(f'  backend: {data[0].get(\"backend\", \"unknown\")}')
            print(f'  backend_id: {data[0].get(\"backend_id\", \"unknown\")}')
"
echo ""

echo "=== 验证完成 ==="
```

### 5.4 预期结果对比

#### 5.4.1 绝对吞吐量预期

基于 aarch64 Apple M5 Max 数据，x86_64 预期：

| 配置 | aarch64 (M5 Max) | x86_64 (Zen 4) | x86_64 (13th Gen) | 说明 |
|------|-----------------|----------------|-------------------|------|
| 4x2_1M | ~1650 MB/s | ~1200-1500 | ~1000-1400 | 小文件，内存延迟敏感 |
| 32x16_1M | ~410 MB/s | ~300-400 | ~250-350 | 中等，计算 + 内存混合 |
| 96x48_1M | ~125 MB/s | ~90-130 | ~80-120 | 大文件，计算密集 |
| 128x64_1M | ~153 MB/s | ~110-150 | ~90-140 | 大文件，计算密集 |

> 注：预期值为粗略估计，实际取决于 CPU 微架构、内存带宽、缓存大小。

#### 5.4.2 策略选择验证

| 配置 | shard_size | 期望策略 | 验证方式 |
|------|-----------|---------|---------|
| 4x2 | < 64K | decomposed | `RSE_DIT4_STRATEGY=auto` 日志 |
| 4x2 | >= 64K | direct | `RSE_DIT4_STRATEGY=auto` 日志 |
| 10x4 | < 64K | decomposed | `RSE_DIT4_STRATEGY=auto` 日志 |
| 10x4 | >= 64K | direct | `RSE_DIT4_STRATEGY=auto` 日志 |

#### 5.4.3 回归检测阈值

| 指标 | 阈值 | 说明 |
|------|------|------|
| 功能测试 | 0 失败 | 不允许任何测试失败 |
| 吞吐量 vs aarch64 | > 50% | x86_64 不应低于 aarch64 的 50% |
| 策略选择正确率 | 100% | auto 模式必须全部正确 |
| 内存对齐 | 64 字节 | FlatWork 分配必须 64 字节对齐 |

---

## 六、实施步骤

### Phase 1: 基线验证 (1 天)

1. 在 x86_64 机器上克隆仓库
2. 运行 `scripts/verify-x86_64.sh` 收集基线数据
3. 确认所有测试通过
4. 记录 x86_64 绝对吞吐量
5. 验证自适应策略选择

### Phase 2: SIMD slice_xor 显式化 (1-2 天)

1. 在 `ops.rs` 中添加 `#[cfg(target_arch = "x86_64")]` 条件编译
2. 实现 `slice_xor_avx2` (使用 `_mm256_xor_si256`)
3. 实现 `slice_xor_ssse3` (使用 `_mm_xor_si128`)
4. 运行功能测试 + 基准测试
5. 对比编译器自动向量化 vs 显式 SIMD

### Phase 3: SIMD fft_dit2_lut 评估 (2-3 天)

1. 参考 galois_8 `x86/ssse3.rs` 实现 nibble-lookup
2. 实现 `gf8_lut_xor_ssse3` 和 `gf8_lut_xor_avx2`
3. 在 x86_64 上基准测试 (关键：与 aarch64 不同，可能有正收益)
4. 如果回归，回退; 如果有收益，保留

### Phase 4: Profile 驱动优化 (3-5 天)

1. 在 x86_64 上运行 profiler (`perf record -g`)
2. 确认 x86_64 的瓶颈分布是否与 aarch64 相同
3. 针对 x86_64 特定瓶颈优化：
   - 如果内存拷贝仍是瓶颈 → 零拷贝 FFT 探索
   - 如果 LUT 查表是瓶颈 → 方案 A/D
   - 如果 XOR 累加是瓶颈 → 方案 B

---

## 七、风险与注意事项

### 7.1 AVX-512 频率节流

| 场景 | 影响 | 缓解 |
|------|------|------|
| 混合 AVX-512 + 标量代码 | 频率切换延迟 ~10μs | 避免频繁切换，批量处理 |
| 持续 AVX-512 | CPU 降频 10-20% | AVX-512 仅用于大 chunk |
| 热累积 | 长时间运行降频 | 基准测试需冷却间隔 |

**建议**: 默认使用 AVX2, AVX-512 仅作为可选后端 (通过 `RSE_BACKEND_OVERRIDE`)。

### 7.2 内存对齐

| 指令 | 对齐要求 | 当前满足 |
|------|---------|---------|
| `_mm_loadu_si128` | 无 | ✅ |
| `_mm256_loadu_si256` | 无 | ✅ |
| `_mm256_load_si256` | 32 字节 | ⚠️ FlatWork 64 字节对齐，满足 |
| `_mm512_load_si512` | 64 字节 | ⚠️ FlatWork 64 字节对齐，满足 |

当前 FlatWork 已 64 字节对齐，满足所有 AVX/AVX-512 对齐要求。但 `src`/`dst` 切片的起始偏移需确保对齐。

### 7.3 平台差异

| 差异 | aarch64 | x86_64 | 处理 |
|------|---------|--------|------|
| 默认 SIMD 宽度 | 128-bit | 128-bit (SSE) | 需显式选择 AVX2/512 |
| 特性检测 | 编译时 | 运行时 | 使用 `is_x86_feature_detected!` |
| 内存模型 | 弱序 | 强序 (TSO) | 无影响 (无原子操作) |
| 页面大小 | 4K/16K/64K | 4K | FlatWork 对齐无影响 |

---

## 八、后续 x86_64 工作提示词

### 8.1 验证阶段提示词

```
在 x86_64 机器上验证 leopard_gf8 优化:

1. 克隆仓库: git clone <repo>
2. 运行验证脚本: bash scripts/verify-x86_64.sh
3. 收集基准数据: target/benchmark-smoke/ 下的 JSON 文件
4. 对比 aarch64 基线: docs/leopard-gf8-optimization-summary-2026-05-30.md
5. 如果吞吐量低于 aarch64 的 50%, 标记为需要调查
6. 验证自适应策略: 4x2_1K 应选 decomposed, 4x2_1M 应选 direct
```

### 8.2 SIMD 优化阶段提示词

```
为 leopard_gf8 添加 x86_64 SIMD 优化:

参考文件:
- src/galois_8/x86/ssse3.rs — SSSE3 nibble-lookup 模式
- src/galois_8/x86/avx2.rs — AVX2 nibble-lookup 模式
- src/galois_8/x86/avx512.rs — AVX-512 nibble-lookup 模式
- src/galois_8/backend.rs — 运行时特性检测模式

实现步骤:
1. 在 ops.rs 中添加 #[cfg(target_arch = "x86_64")] 分支
2. 实现 slice_xor_avx2 (优先, 风险最低)
3. 实现 gf8_lut_xor_ssse3 / gf8_lut_xor_avx2 (蝶形运算)
4. 使用 #[target_feature(enable = "avx2")] 标注
5. 标量 fallback 处理尾部字节
6. 运行 cargo test --lib --features std 确认功能正确
7. 运行 benchmark_smoke 对比性能

关键注意事项:
- shuffle_epi8 的 index 只看低 4 位, 高位被忽略
- 256-bit shuffle 不跨 128-bit lane, 需要 split+merge
- AVX-512 可能降频, 默认使用 AVX2
- FlatWork 已 64 字节对齐, 满足 AVX 对齐要求
```

### 8.3 CI 集成提示词

```
添加 x86_64 CI 验证:

1. 在 .github/workflows/ 添加 x86_64 测试矩阵
2. 覆盖: linux-gnu, apple-darwin, windows-msvc
3. 测试: std, std+simd-accel
4. 基准: 运行 benchmark_smoke 并记录 JSON
5. 对比: 与历史数据比较, 检测回归
```

### 8.4 Profile 分析提示词

```
在 x86_64 上进行性能分析:

# 采集 profile
perf record -g -- cargo test --test benchmark_smoke --features std -- benchmark_leopard_encode_96x48_1m

# 分析热点
perf report --sort=dso,symbol

# 关注:
# - fft_dit4_full_lut / ifft_dit4_full_lut — 蝶形运算
# - slice_xor — XOR 累加
# - memcpy — 数据拷贝
# - gf8_lut_xor (如果已实现) — SIMD LUT 查找

# 对比 aarch64 profile:
# aarch64 分布: input_copy 43.8%, xor 23.3%, output 21.7%, fft 8%
# 如果 x86_64 分布不同, 优化策略应调整
```

---

## 九、相关文件索引

| 文件 | 内容 |
|------|------|
| `src/core/leopard_gf8/ops.rs` | 蝶形运算，slice_xor — SIMD 优化目标 |
| `src/core/leopard_gf8/encode.rs` | encode 主逻辑，dit4 策略选择 |
| `src/core/leopard_gf8/work.rs` | FlatWork 对齐分配 |
| `src/core/leopard_gf8/mod.rs` | 常量，数据结构，Plan 构建 |
| `src/galois_8/x86/ssse3.rs` | SSSE3 nibble-lookup 参考实现 |
| `src/galois_8/x86/avx2.rs` | AVX2 nibble-lookup 参考实现 |
| `src/galois_8/x86/avx512.rs` | AVX-512 nibble-lookup 参考实现 |
| `src/galois_8/backend.rs` | 运行时特性检测 + 后端选择 |
| `docs/leopard-gf8-optimization-summary-2026-05-30.md` | aarch64 优化汇总 |
| `docs/leopard-gf8-optimization-roadmap-2026-05-30.md` | 优化路线图 |
| `scripts/verify-x86_64.sh` | x86_64 验证脚本 (待创建) |
