# Leopard GF8 x86_64 验证与优化 — 大模型执行提示语

> 本文件包含两份提示语：
> 1. **验证阶段**: 仅验证 + 收集数据，不修改代码
> 2. **优化阶段**: 验证通过后，实施 SIMD 优化

---

## 提示语 1: 验证阶段 (首次使用)

```
你是一个 Rust 性能工程师。请在当前 x86_64 机器上验证 rustfs-erasure-codec 项目的 leopard_gf8 编码器。

## 背景

该项目在 aarch64 (Apple M5 Max) 上已完成 4 轮优化, 现需在 x86_64 上验证。核心文件:
- src/core/leopard_gf8/ — Leopard FFT 编码器 (纯 Rust, 无 SIMD intrinsics)
- src/galois_8/x86/ — galois_8 已有 SSSE3/AVX2/AVX-512 SIMD 后端
- docs/leopard-gf8-optimization-summary-2026-05-30.md — aarch64 优化汇总
- docs/leopard-gf8-x86_64-verification-plan-2026-05-30.md — 详细方案

## 任务

严格按以下步骤执行, 每步完成后输出结果:

### Step 1: 环境信息
输出: CPU 型号、SIMD 特性 (SSSE3/AVX2/AVX-512/GFNI)、Rust 版本、OS

### Step 2: 编译验证
```bash
cargo build --features std 2>&1
cargo build --release --features std 2>&1
cargo build --features std,simd-accel 2>&1  # 可能失败，记录即可
```

### Step 3: 功能测试
```bash
cargo test --lib --features std 2>&1
```
期望: 199 passed, 0 failed

### Step 4: 基准冒烟测试
```bash
cargo test --test benchmark_smoke --features std -- --nocapture 2>&1
```
期望: 27 passed

### Step 5: 收集 leopard encode 吞吐量
从 target/benchmark-smoke/leopard-encode-*.json 提取 throughput_mb_s, 汇总为表格:

| case | throughput_mb_s |
|------|----------------|
| 32x16_1m | ? |
| 32x16_4m | ? |
| 64x32_64k | ? |
| 64x32_1m | ? |
| 64x32_4m | ? |
| 96x48_1m | ? |
| 96x48_4m | ? |
| 128x64_1m | ? |
| 128x64_4m | ? |

### Step 6: 与 aarch64 基线对比
aarch64 基线数据 (Apple M5 Max, commit d242272):

| case | aarch64 MB/s |
|------|-------------|
| 32x16_1m | 420.60 |
| 32x16_4m | 422.28 |
| 64x32_64k | 324.17 |
| 64x32_1m | 341.80 |
| 64x32_4m | 350.58 |
| 96x48_1m | 125.35 |
| 96x48_4m | 134.98 |
| 128x64_1m | 153.48 |
| 128x64_4m | 163.16 |

计算每个 case 的 x86_64/aarch64 比率。如果任何 case < 50%, 标记为异常。

### Step 7: 小文件策略验证
运行小文件基准 (可选, 耗时较长):
```bash
cargo test --test benchmark_smoke --features std -- benchmark_small_file --nocapture 2>&1
```
验证: 4x2_1K 应选 decomposed, 4x2_1M 应选 direct

### Step 8: Profile 分析
如果 Linux, 使用 perf:
```bash
perf record -g -- cargo test --test benchmark_smoke --features std -- benchmark_leopard_encode_96x48_1m
perf report --sort=dso,symbol | head -30
```
如果 macOS, 使用 Instruments 或 `cargo instruments -t time`

输出热点函数排序。

### Step 9: 生成报告
将所有结果写入 docs/leopard-gf8-x86_64-baseline-YYYY-MM-DD.md, 格式:
1. 环境信息
2. 编译/测试结果
3. 吞吐量对比表
4. 异常标记
5. Profile 热点
6. 结论: 是否需要 SIMD 优化

## 约束
- 不修改任何源代码
- 如果测试失败, 记录错误信息但继续执行
- 如果基准测试因热节流导致数据异常低, 等待 60 秒后重试
- 所有数据保存到 docs/ 目录
```

---

## 提示语 2: 优化阶段 (验证通过后使用)

```
你是一个 Rust SIMD 优化工程师。请在当前 x86_64 机器上为 leopard_gf8 编码器实施 SIMD 优化。

## 前置条件
- 已完成验证阶段, 基线数据在 docs/leopard-gf8-x86_64-baseline-*.md
- 所有 199 个测试通过, 27 个基准测试通过

## 核心文件
- src/core/leopard_gf8/ops.rs — 优化目标 (slice_xor, fft_dit2_lut, ifft_dit2_lut)
- src/galois_8/x86/ssse3.rs — SSSE3 nibble-lookup 参考实现
- src/galois_8/x86/avx2.rs — AVX2 nibble-lookup 参考实现
- src/galois_8/backend.rs — 运行时特性检测模式

## 实施顺序

### Phase A: 显式 SIMD slice_xor (风险最低, 优先实施)

在 ops.rs 中添加 x86_64 分支:

```rust
#[inline]
fn slice_xor(input: &[u8], out: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { slice_xor_avx2(input, out); }
            return;
        }
    }
    // 现有 u64 fallback
    slice_xor_fallback(input, out);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn slice_xor_avx2(input: &[u8], out: &mut [u8]) {
    // 使用 _mm256_xor_si256, 32 字节/迭代
    // 处理尾部用 u64 或标量
}
```

验证:
```bash
cargo test --lib --features std 2>&1
```

### Phase B: SIMD fft_dit2_lut / ifft_dit2_lut (中等风险)

参考 galois_8 x86/ssse3.rs 的 nibble-lookup 模式:

1. 从 256 字节 LUT 预计算 16 字节 low/high nibble 表
2. SSSE3: _mm_shuffle_epi8 处理 16 字节/迭代
3. AVX2: _mm256_shuffle_epi8 处理 32 字节/迭代 (注意跨 lane 问题)
4. 标量 fallback 处理尾部

关键:
- _mm_shuffle_epi8 只看 index 低 4 位
- _mm256_shuffle_epi8 不跨 128-bit lane, 高 128-bit 和低 128-bit 独立 shuffle
- 如果 AVX2 实现复杂, 先只做 SSSE3

验证:
```bash
cargo test --lib --features std 2>&1  # 功能正确
cargo test --test benchmark_smoke --features std -- --nocapture 2>&1  # 性能
```

### Phase C: 评估与决策

对比 Phase A/B 与基线:
- 如果有收益 → 保留
- 如果回归 → 回退, 记录原因
- 将对比数据写入 docs/leopard-gf8-x86_64-simd-results-YYYY-MM-DD.md

## 约束
- 每个 Phase 完成后必须通过全部 199 个测试
- 每个 Phase 完成后运行基准测试对比
- 如果某个 Phase 导致回归, 立即回退该 Phase 的改动
- 使用 #[cfg(target_arch = "x86_64")] 隔离 x86_64 代码
- 使用 #[target_feature(enable = "avx2")] 标注 SIMD 函数
- 始终保留标量 fallback 路径
- 不修改 galois_8 模块的任何代码

## 参考: galois_8 x86 SIMD 实现模式

SSSE3 (src/galois_8/x86/ssse3.rs):
- 16 字节 nibble-lookup: low = src & 0xf, high = (src >> 4) & 0xf
- product = shuffle(low_tbl, low) ^ shuffle(high_tbl, high)
- out ^= product (XOR 变体) 或 out = product (纯变体)

AVX2 (src/galois_8/x86/avx2.rs):
- 广播 16 字节表到 256-bit: _mm256_broadcastsi128_si256
- 32 字节/迭代, 同样 nibble-lookup

运行时检测 (src/galois_8/backend.rs):
- static BACKEND: OnceLock<BackendInfo>
- is_x86_feature_detected!("avx2") 运行时检测
```

---

## 提示语 3: 快速验证 (仅确认编译 + 测试通过)

```
在当前 x86_64 机器上快速验证 rustfs-erasure-codec:

1. cargo build --features std — 确认编译通过
2. cargo test --lib --features std — 确认 199 测试通过
3. cargo test --test benchmark_smoke --features std — 确认 27 基准测试通过
4. 从 target/benchmark-smoke/leopard-encode-*.json 提取吞吐量
5. 输出汇总表格

如果任何步骤失败, 记录错误信息。不修改任何代码。
```

---

## 使用建议

| 场景 | 推荐提示语 | 预计耗时 |
|------|-----------|---------|
| 首次在新 x86_64 机器上验证 | 提示语 1 (验证阶段) | 15-30 分钟 |
| 验证通过，开始 SIMD 优化 | 提示语 2 (优化阶段) | 2-5 天 |
| CI 环境快速冒烟 | 提示语 3 (快速验证) | 5-10 分钟 |
| 仅检查编译是否通过 | `cargo build --features std` | 30 秒 |

## 数据传递

验证阶段产出的基线数据文件路径应传递给优化阶段：
```
基线数据: docs/leopard-gf8-x86_64-baseline-2026-05-30.md
aarch64 对比: docs/leopard-gf8-optimization-summary-2026-05-30.md
优化方案: docs/leopard-gf8-x86_64-verification-plan-2026-05-30.md
```
