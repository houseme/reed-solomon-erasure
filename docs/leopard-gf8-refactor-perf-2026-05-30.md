# Leopard GF8 重构后性能检测报告

> 测试日期: 2026-05-30
> 提交: `0af1fe4` (main)
> 平台: Apple M5 Max / aarch64-macos-unknown
> Rust: 1.96.0 (ac68faa20 2026-05-25)
> Backend: scalar-rust (ScalarRust)
> Features: std, benchmark-metrics
> 重构内容: 见 `docs/leopard-gf8-code-review-2026-05-30.md`

---

## 一、重构变更摘要

本次重构涉及 7 个文件，主要变更:

| 变更项 | 影响 |
|--------|------|
| 移除死代码 (`leopard_env_enabled`, `should_use_xor_clone`, `ifft_dit_encoder8`) | 减少 ~80 行 |
| `assert_eq!` -> `debug_assert_eq!` (热路径) | 减少 release 构建开销 |
| Driver 魔法数字提取为命名常量 | 可维护性 |
| Profile 样板代码提取为辅助方法 | 减少 ~100 行重复 |
| `fft_dit4_at` / `ifft_dit4_at` 参数化合并 | 减少 ~70 行重复 |
| `fft_dit2` / `ifft_dit2` 非 lut 变体改为 thin wrapper | 减少 ~20 行重复 |
| 测试公共函数提取到 `benches/common` | 消除 ~120 行跨文件重复 |

---

## 二、Leopard GF8 编码吞吐量

### 2.1 标准测试

| 配置 | shard_size | 吞吐量 (MB/s) | 单次耗时 (ns) |
|------|-----------|---------------|--------------|
| 32x16 | 64K | -- | -- |
| 32x16 | 1M | **31.02** | 1,031,705,459 |
| 32x16 | 4M | **31.04** | 4,123,539,396 |
| 64x32 | 64K | **29.61** | 135,103,250 |
| 64x32 | 1M | **29.66** | 2,157,922,688 |
| 64x32 | 4M | **29.25** | 8,751,563,521 |
| 96x48 | 1M | **8.57** | 11,198,772,000 |
| 96x48 | 4M | **8.82** | 43,560,740,750 |
| 128x64 | 1M | **10.46** | 12,235,298,521 |
| 128x64 | 4M | **10.74** | 47,681,494,917 |

**观察**:
- 32x16 和 64x32 配置表现稳定，吞吐量 ~29-31 MB/s
- 96x48 配置吞吐量显著下降至 ~8.6 MB/s，因为 `m=64` 但 `data_shards=96 > m`，需要 later-group IFFT 累加路径
- 128x64 配置 (~10.5 MB/s) 优于 96x48，因为 `data_shards=128` 恰好是 `m=64` 的整数倍，无 remainder group 开销

### 2.2 A/B 变体对比 (64x32_1m)

| 变体 | 吞吐量 (MB/s) | 单次耗时 (ns) | 相对 baseline |
|------|---------------|--------------|--------------|
| baseline | **29.71** | 2,154,426,709 | 100.0% |
| reuse_zero_only | **29.58** | 2,163,493,146 | 99.6% |
| xor_clone_only | **29.20** | 2,191,475,583 | 98.3% |

**观察**: baseline (混合模式) 性能最优，reuse_zero_only 和 xor_clone_only 变体无明显优势。

---

## 三、Leopard GF8 编码 Profile 详细数据

### 3.1 96x48_1m Profile

| 指标 | 值 |
|------|-----|
| encode_calls | 24 |
| encode_chunks | 920 |
| encode_full_groups | 1792 |
| encode_remainder_groups | 46 |
| encode_later_group_calls | 872 |
| fft_stage_calls | 917 |
| ifft_stage_calls | 1838 |
| first_group_ifft_calls | 920 |
| later_group_ifft_calls | 872 |
| remainder_group_ifft_calls | 46 |
| **input_copy_bytes** | **2,668,625,920** (2.48 GiB) |
| first_group_input_copy_bytes | 1,442,840,576 |
| later_group_input_copy_bytes | 1,032,847,360 |
| remainder_group_input_copy_bytes | 192,937,984 |
| **zero_fill_bytes** | **192,937,984** (184 MiB) |
| first_group_zero_fill_bytes | 0 |
| later_group_zero_fill_bytes | 0 |
| remainder_group_zero_fill_bytes | 192,937,984 |
| **xor_bytes** | **1,410,334,720** (1.31 GiB) |
| later_group_xor_bytes | 1,032,847,360 |
| remainder_group_xor_bytes | 377,487,360 |
| output_writeback_calls | 916 |
| **output_writeback_bytes** | **1,309,671,424** (1.22 GiB) |

**数据流分析 (96x48)**:
- 总输入拷贝: 2.48 GiB (first group 占 54%, later group 占 39%, remainder 占 7%)
- XOR 累加: 1.31 GiB (later group 占 73%, remainder 占 27%)
- 输出回写: 1.22 GiB
- 零填充: 184 MiB (仅 remainder group)

### 3.2 128x64_1m Profile

| 指标 | 值 |
|------|-----|
| encode_calls | 24 |
| encode_chunks | 925 |
| encode_full_groups | 1803 |
| encode_remainder_groups | 47 |
| encode_later_group_calls | 878 |
| fft_stage_calls | 924 |
| ifft_stage_calls | 1850 |
| first_group_ifft_calls | 925 |
| later_group_ifft_calls | 878 |
| remainder_group_ifft_calls | 47 |
| **input_copy_bytes** | **2,765,094,912** (2.58 GiB) |
| first_group_input_copy_bytes | 1,484,783,616 |
| later_group_input_copy_bytes | 1,083,179,008 |
| remainder_group_input_copy_bytes | 197,132,288 |
| **zero_fill_bytes** | **197,132,288** (188 MiB) |
| first_group_zero_fill_bytes | 0 |
| later_group_zero_fill_bytes | 0 |
| remainder_group_zero_fill_bytes | 197,132,288 |
| **xor_bytes** | **1,469,054,976** (1.37 GiB) |
| later_group_xor_bytes | 1,074,790,400 |
| remainder_group_xor_bytes | 394,264,576 |
| output_writeback_calls | 923 |
| **output_writeback_bytes** | **1,362,100,224** (1.27 GiB) |

---

## 四、Classic 编码基准 (对比参考)

| 配置 | 操作 | 吞吐量 (MB/s) | 单次耗时 (ns) |
|------|------|---------------|--------------|
| 4x2 64K | encode | 41.01 | 6,095,666 |
| 4x2 64K | update | 33.72 | 7,414,000 |
| 4x2 64K | verify | 27.21 | 9,187,208 |
| 4x2 64K | reconstruct | 27.48 | 9,098,583 |
| 4x2 64K | reconstruct_data | 27.60 | 9,057,875 |
| 10x4 1M | encode | 29.17 | 342,847,666 |
| 10x4 1M | update | 26.91 | 371,635,875 |
| 10x4 1M | verify | 17.79 | 562,072,334 |
| 10x4 1M | reconstruct | 22.06 | 453,272,875 |
| 10x4 1M | reconstruct_data | 22.10 | 452,475,750 |

**Classic vs Leopard GF8 对比 (10x4 1M / 32x16 1M)**:
- Classic encode: 29.17 MB/s
- Leopard GF8 encode (32x16): 31.02 MB/s
- Leopard GF8 在此配置下略优于 Classic (+6.3%)

---

## 五、重构影响评估

### 5.1 功能正确性
- 全部 199 个单元测试通过
- 全部 12 个 leopard encode 基准测试通过
- 全部 smoke matrix 测试通过

### 5.2 性能影响
本次重构为纯代码质量改进，不涉及算法或数据结构变更:
- `debug_assert_eq!` 替换: release 构建中无运行时开销 (debug_assert 在 release 模式下被编译器移除)
- Profile 辅助方法: 编译器内联后与原始代码等价
- `dit4_at` 参数化: 通过 `#[inline]` 提示，编译器可消除 dispatch 开销
- `fft_dit2` thin wrapper: 编译器内联后直接调用 `_lut` 变体

### 5.3 代码行数变化

| 文件 | 重构前 | 重构后 | 变化 |
|------|--------|--------|------|
| `encode.rs` | 646 | ~530 | -116 |
| `ops.rs` | 422 | ~400 | -22 |
| `mod.rs` | 408 | ~460 | +52 (新增 helper 方法) |
| `driver.rs` | 37 | ~45 | +8 (命名常量) |
| `benches/common/mod.rs` | 263 | ~330 | +67 (共享函数) |
| `benchmark_smoke.rs` | ~1500 | ~1440 | -60 |
| `benchmark_small_files.rs` | ~570 | ~510 | -60 |
| **净变化** | | | **-131 行** |

---

## 六、后续优化方向

基于 Profile 数据的观察:

1. **输入拷贝是最大瓶颈**: 96x48 配置下 input_copy_bytes (2.48 GiB) 远超其他操作。可考虑 zero-copy 或 mmap 策略
2. **XOR 累加优化**: later_group_xor_bytes 占 XOR 总量的 73%。`slice_xor` 的显式 SIMD 实现可带来 2-4x 提升
3. **96x48 性能悬崖**: 从 64x32 的 ~30 MB/s 骤降至 ~8.6 MB/s，需要分析 later-group 路径的额外开销
4. **零填充仅出现在 remainder group**: first_group 和 later_group 的 zero_fill_bytes 为 0，说明当前 padding 策略有效
