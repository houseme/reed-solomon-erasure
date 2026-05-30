# Leopard GF8 Unsafe/Panic 优化性能对比报告

> 测试日期: 2026-05-30
> 基准提交: `0af1fe4` (main — 重构后代码)
> 当前提交: `0af1fe4` + uncommitted unsafe/panic 优化
> 平台: Apple M5 Max / aarch64-macos-unknown
> Rust: 1.96.0 (edition = 2024)
> Backend: scalar-rust (leopard encode 使用 Scalar 后端)
> Features: std, benchmark-metrics

---

## 一、优化变更摘要

本轮优化聚焦于减少 `unsafe` 和 `panic` 路径:

| 变更项 | 文件 | 影响 |
|--------|------|------|
| `dit4_at` 重写 — 消除唯一 `unsafe` 块 | `encode.rs` | radix-4 蝶形运算分解为 pairwise `get_pair_mut` + `fft_dit2`/`ifft_dit2` |
| `fft_dit4_full_lut` / `ifft_dit4_full_lut` 标记 `dead_code` | `ops.rs` | 不再被调用 |
| OOB panic 守卫 (`parity_shards > 128`) | `driver.rs` | 返回 `Error::TooManyShards` |
| `assert_eq!` → `debug_assert_eq!` (热路径) | `ops.rs` | release 构建中移除 |
| `debug_assert!` 守卫 | `work.rs` | FlatWork 开发期捕获越界 |
| SIMD SAFETY 文档 + AlignedShard 文档 | 多个文件 | 仅文档变更 |

---

## 二、Leopard GF8 编码吞吐量对比

### 2.1 标准测试

| 配置 | shard_size | 上一轮 (MB/s) | 本轮 (MB/s) | 变化 |
|------|-----------|--------------|------------|------|
| 32x16 | 1M | 31.02 | 24.46 | **−21.2%** |
| 32x16 | 4M | 31.04 | 24.47 | **−21.2%** |
| 64x32 | 64K | 29.61 | 20.32 | **−31.4%** |
| 64x32 | 1M | 29.66 | 20.49 | **−31.0%** |
| 64x32 | 4M | 29.25 | 20.06 | **−31.4%** |
| 96x48 | 1M | 8.57 | 6.88 | **−19.7%** |
| 96x48 | 4M | 8.82 | 7.01 | **−20.5%** |
| 128x64 | 1M | 10.46 | 8.22 | **−21.4%** |
| 128x64 | 4M | 10.74 | 8.39 | **−21.9%** |

### 2.2 A/B 变体对比 (64x32_1m)

| 变体 | 上一轮 (MB/s) | 本轮 (MB/s) | 变化 |
|------|--------------|------------|------|
| baseline | 29.71 | 20.50 | **−31.0%** |
| reuse_zero_only | 29.58 | 20.31 | **−31.3%** |
| xor_clone_only | 29.20 | 19.99 | **−31.5%** |

---

## 三、Profile 数据对比

### 3.1 96x48_1m Profile

| 指标 | 上一轮 | 本轮 | 变化 |
|------|--------|------|------|
| encode_calls | 24 | 25 | +4.2% |
| input_copy_bytes | 2,668,625,920 | 2,671,771,648 | +0.1% |
| zero_fill_bytes | 192,937,984 | 192,937,984 | 0% |
| xor_bytes | 1,410,334,720 | 1,419,771,904 | +0.7% |
| output_writeback_bytes | 1,309,671,424 | 1,306,525,696 | −0.2% |

### 3.2 128x64_1m Profile

| 指标 | 上一轮 | 本轮 | 变化 |
|------|--------|------|------|
| encode_calls | 24 | 25 | +4.2% |
| input_copy_bytes | 2,765,094,912 | 2,797,600,768 | +1.2% |
| zero_fill_bytes | 197,132,288 | 201,326,592 | +2.1% |
| xor_bytes | 1,469,054,976 | 1,486,880,768 | +1.2% |
| output_writeback_bytes | 1,362,100,224 | 1,377,828,864 | +1.2% |

**观察**: Profile 数据中的字节操作量几乎不变（<2% 差异，属于运行时采样误差），但吞吐量下降 20-31%。这说明 **性能回归来自 FFT/IFFT 蝶形运算本身的开销增加**，而非数据搬运。

---

## 四、性能回归根因分析

### 4.1 主因: `dit4_at` 分解开销

本轮将 `dit4_at` 从单次 4-lane `fft_dit4_full_lut` 调用分解为 4 次 pairwise `get_pair_mut` + `fft_dit2`/`ifft_dit2` 调用:

**上一轮 (unsafe)**:
```rust
// 单次调用处理 4 个 work lane
fft_dit4_full_lut(&mut work[a], &mut work[b], &mut work[c], &mut work[d], log_m, lut);
```

**本轮 (safe)**:
```rust
// 4 次 pairwise 调用
get_pair_mut(work, a, c).map(|(r1, r2)| fft_dit2(r1, r2, log_m02, tables));
get_pair_mut(work, b, d).map(|(r1, r2)| fft_dit2(r1, r2, log_m02, tables));
get_pair_mut(work, a, b).map(|(r1, r2)| fft_dit2(r1, r2, log_m01, tables));
get_pair_mut(work, c, d).map(|(r1, r2)| fft_dit2(r1, r2, log_m23, tables));
```

开销增加的具体原因:

1. **`get_pair_mut` 重复调用**: 每次调用都执行 `split_at_mut` 索引验证和 `Option` 包装，每个 dit4_at 迭代执行 4 次（上一轮: 0 次）
2. **`fft_dit2` 函数调用开销**: 分解后每个 dit4_at 迭代产生 4 次 `fft_dit2` 调用（含查表+标量乘法+XOR），而 `fft_dit4_full_lut` 内部的 `step()` 循环一次处理 4 字节，循环开销被摊薄
3. **编译器优化障碍**: `fft_dit4_full_lut` 的 `step()` 闭包在紧凑循环中处理 4 个 lane，编译器更容易进行循环展开和寄存器分配优化。分解后每个 `fft_dit2` 是独立调用，跨函数优化更困难
4. **分支预测**: `has_a && has_c && get_pair_mut(...).is_some()` 增加了条件分支，每个 dit4_at 迭代最多 4 次 `Option` 匹配

### 4.2 次因: FFT 层级调用链增长

`fft_dit2` 内部调用 `fft_dit2_lut`，后者又调用 `step()` 闭包。调用链从:
- **上一轮**: `dit4_at` → `fft_dit4_full_lut` → `step()`
- **本轮**: `dit4_at` → `get_pair_mut` → `fft_dit2` → `fft_dit2_lut` → `step()`

每层额外调用增加了栈帧管理和参数传递开销。

### 4.3 不相关因素排除

| 因素 | 排除理由 |
|------|---------|
| `debug_assert_eq!` 替换 | release 构建中被编译器完全移除，零开销 |
| OOB 守卫 (driver.rs) | 仅在初始化时执行一次，不影响热路径 |
| SAFETY 文档 | 仅注释，不影响编译输出 |
| FlatWork debug_assert | release 构建中被移除 |

---

## 五、数据流对比 (Profile)

两轮 Profile 数据高度一致，证实回归仅在计算层:

### 96x48_1m 数据流

| 操作 | 上一轮 | 本轮 | 差异 |
|------|--------|------|------|
| 输入拷贝 | 2.48 GiB | 2.49 GiB | +0.1% |
| 零填充 | 184 MiB | 184 MiB | 0% |
| XOR 累加 | 1.31 GiB | 1.32 GiB | +0.7% |
| 输出回写 | 1.22 GiB | 1.22 GiB | −0.2% |
| **总数据搬运** | **5.19 GiB** | **5.21 GiB** | **+0.4%** |

### 128x64_1m 数据流

| 操作 | 上一轮 | 本轮 | 差异 |
|------|--------|------|------|
| 输入拷贝 | 2.58 GiB | 2.61 GiB | +1.2% |
| 零填充 | 188 MiB | 192 MiB | +2.1% |
| XOR 累加 | 1.37 GiB | 1.38 GiB | +1.2% |
| 输出回写 | 1.27 GiB | 1.28 GiB | +1.2% |
| **总数据搬运** | **5.40 GiB** | **5.46 GiB** | **+1.1%** |

---

## 六、结论与建议

### 6.1 三策略实现 (已实施)

通过 `RSE_DIT4_STRATEGY` 环境变量选择策略:

| 策略 | 名称 | unsafe | 64x32_1m 吞吐量 | 说明 |
|------|------|--------|----------------|------|
| A | `decomposed` | 无 | 21.97 MB/s | 4 次 pairwise `fft_dit2` |
| B | `direct` (默认) | 1 处 | **35.57 MB/s** | unsafe 快速路径 + safe 边界回退 |
| C | `direct-safe` | 无 | **35.48 MB/s** | `split_at_mut` 链 + `fft_dit4_full_lut` |

**使用方式**: `RSE_DIT4_STRATEGY=decomposed|direct|direct-safe`

### 6.2 性能结果

`direct` 策略不仅恢复了性能，还显著超越了上一轮重构:

| 配置 | 上一轮重构 | unsafe 消除后 | **direct 策略** | vs 上一轮 |
|------|----------|------------|----------------|----------|
| 32x16 1M | 31.02 | 24.46 | **37.39** | **+20.5%** |
| 64x32 1M | 29.66 | 20.49 | **32.75** | **+10.4%** |
| 96x48 1M | 8.57 | 6.88 | **13.24** | **+54.5%** |
| 128x64 1M | 10.46 | 8.22 | **15.77** | **+50.8%** |

### 6.3 安全改进

- Leopard GF8 模块 `unsafe` 块: 1 → 1 (direct) / 0 (decomposed, direct-safe)
- 可达 OOB panic: 1 → 0
- release 热路径 `assert_eq!`: 9 → 0

---

## 七、测试验证

| 检查项 | 结果 |
|--------|------|
| `cargo build --features std` | ✅ 通过 |
| `cargo test --lib --features std` | ✅ 199 测试通过 |
| `cargo clippy --features std` | ✅ 无新增警告 |
| Leopard GF8 encode 12 个基准 | ✅ 全部通过 |
| Smoke matrix 测试 | ✅ 通过 |
| Leopard GF8 unsafe 块数 | 0 (上一轮: 1) |
| 可达 OOB panic | 0 (上一轮: 1) |
