# Leopard GF8 代码审查与改造方案

> 审查日期: 2026-05-30
> 范围: `src/core/leopard_gf8/` 模块 + 相关测试/基准代码
> 状态: 待评审

---

## 一、模块总览

| 文件 | 职责 | 行数 |
|------|------|------|
| `mod.rs` | 常量、类型定义、Plan 构建器、全局表/性能计数器、公共 API | ~410 |
| `encode.rs` | 编码主流程: `encode_skeleton`、`encode_with_tables`、IFFT/FFT 蝶形运算 | ~650 |
| `ops.rs` | GF(2^8) 算术、Walsh-Hadamard 变换、`slice_xor`、蝶形基元 | ~420 |
| `tables.rs` | 一次性 LUT 构建: Cantor 基 log/exp、FFT 偏斜因子、GF 乘法表 | ~130 |
| `driver.rs` | 编码驱动参数计算 (chunk sizing、work slice 数量) | ~37 |
| `work.rs` | `FlatWork` 连续缓冲区抽象 | ~65 |
| `tests.rs` | 单元测试 (表形状、驱动参数) | ~25 |

**编码流程**: `encode_with_tables` -> 构建 driver -> 初始化 tables -> 构建 IFFT/FFT Plan -> 分块循环处理 (First-group IFFT -> Later-group IFFT XOR 累加 -> Remainder IFFT -> FFT -> 输出回写)

---

## 二、问题清单

### P0 - 正确性与可靠性

#### 2.1 缺少编码正确性验证
**位置**: `tests/benchmark_smoke.rs`, `src/core/leopard_gf8/tests.rs`

当前测试仅验证 parity shard 是否**发生变化**，没有验证编码结果的**数学正确性**（如：编码后重建是否能恢复原始数据）。这是最严重的测试缺口。

**改造方案**:
- 新增端到端正确性测试：`encode` -> 模拟丢弃若干 data shard -> `reconstruct` -> 比对原始数据
- 使用已知输入和预期输出的 golden vector 测试
- 引入 proptest/quickcheck 进行属性测试："任意输入编码后重建应恢复原始数据"

#### 2.2 Profile 测试并行安全问题
**位置**: `tests/benchmark_smoke.rs` `run_leopard_encode_profile`

`reset_leopard_gf8_profile_stats()` 和 `leopard_gf8_profile_stats()` 操作全局 `static PROFILE8`。Rust `#[test]` 默认并行执行，多个 profile 测试并发运行会导致计数器互相污染。`Ordering::Relaxed` 加剧了这个问题。

**改造方案**:
- 方案 A: 使用 `#[serial]` 注解 (来自 `serial_test` crate) 串行化 profile 测试
- 方案 B: 将 `PROFILE8` 从全局 static 改为通过 `ReedSolomon` 实例持有，消除全局状态
- 方案 C: 在 profile 测试中加 `std::sync::Mutex` 保护

**推荐方案 B**，从根本上消除共享可变状态。

#### 2.3 `LeopardGF8Codec::setup_matrix()` 调用 `unreachable!()`
**位置**: `src/core/leopard.rs:62`

如果任何代码路径意外调用此方法会导致 panic。虽然当前不会被触发，但作为防御性编程应返回 `Err` 或 `None`。

---

### P1 - 性能

#### 2.4 `slice_xor` 缺少 SIMD 内建函数
**位置**: `src/core/leopard_gf8/ops.rs:62`

手动展开 64 元素的 XOR 循环依赖 LLVM 自动向量化，不同编译器版本行为不一致。对于处理 MB 级数据的 Reed-Solomon 编码器，显式 SIMD (AVX2 `_mm256_xor_si256` / NEON `veorq_u8`) 可带来显著提升。

**改造方案**:
- 短期: 添加 `#[repr(align(32))]` 对齐类型，确保编译器能更好向量化
- 中期: 使用 `std::simd` (nightly) 或 `wide` crate 实现跨平台 SIMD
- 长期: 与 `galois_8` 模块的 SIMD 后端分发机制统一，运行时选择最优实现

```rust
// 方案示例: 运行时分发
pub fn slice_xor(input: &[u8], out: &mut [u8]) {
    match active_backend() {
        Backend::Avx2 => unsafe { slice_xor_avx2(input, out) },
        Backend::Neon => unsafe { slice_xor_neon(input, out) },
        _ => slice_xor_scalar(input, out),
    }
}
```

#### 2.5 `assert_eq!` 存在于热路径
**位置**: `ops.rs` `slice_xor`, `fwht8` 等

`assert_eq!` 在 release 构建中仍然执行（除非关闭 debug assertions）。

**改造方案**: 将 `assert_eq!` 替换为 `debug_assert_eq!`，或在稳定后移除。

#### 2.6 Driver chunk size 魔法数字
**位置**: `src/core/leopard_gf8/driver.rs`

阈值 `192`、`144`、`WORK_SIZE8_HIGH_FANOUT` 缺乏文档说明。极端情况下 `m=256, chunk_size=128KB` 会产生 64 MiB 工作内存。

**改造方案**:
- 添加文档注释解释阈值选择依据
- 考虑添加工作内存预算上限检查
- 将魔法数字提取为命名常量

```rust
/// Total shard count threshold for high-fanout chunk mode.
/// Above this, per-chunk FFT overhead dominates; larger chunks amortize it.
const HIGH_FANOUT_TOTAL_SHARDS: usize = 192;

/// Lower threshold when the last group has a non-zero remainder,
/// which adds an extra IFFT pass per chunk.
const HIGH_FANOUT_TOTAL_SHARDS_WITH_REMAINDER: usize = 144;
```

---

### P2 - 可维护性

#### 2.7 Profile 样板代码膨胀 (~130 行重复)
**位置**: `src/core/leopard_gf8/encode.rs` `ifft_dit_encoder8_with_plan`

同一个 `match phase` 三路分支模式重复 8 次，严重损害可读性。

**改造方案**: 引入辅助宏或方法。

```rust
// 方案 A: 辅助方法
impl LeopardGf8ProfileMetrics {
    fn add_bytes(&self, phase: IfftProfilePhase, counter: &AtomicUsize, bytes: usize) {
        #[cfg(feature = "std")]
        {
            counter.fetch_add(bytes, Ordering::Relaxed);
            match phase {
                IfftProfilePhase::FirstGroup => self.first_group_*.fetch_add(bytes, Ordering::Relaxed),
                IfftProfilePhase::LaterGroup => self.later_group_*.fetch_add(bytes, Ordering::Relaxed),
                IfftProfilePhase::RemainderGroup => self.remainder_group_*.fetch_add(bytes, Ordering::Relaxed),
            }
        }
    }
}

// 方案 B: 宏
macro_rules! profile_phase_bytes {
    ($phase:expr, $global:ident, $($variant:ident => $field:ident),+) => {
        #[cfg(feature = "std")]
        {
            PROFILE8.$global.fetch_add($bytes, Ordering::Relaxed);
            match $phase {
                $(IfftProfilePhase::$variant => PROFILE8.$field.fetch_add($bytes, Ordering::Relaxed),)+
            }
        }
    };
}
```

**推荐方案 A**，更易读、更易调试。

#### 2.8 `fft_dit4_at` / `ifft_dit4_at` 代码重复 (~146 行)
**位置**: `src/core/leopard_gf8/encode.rs:171-317`

两个函数几乎完全相同，仅蝶形运算顺序和 LUT 调用不同。

**改造方案**: 参数化变换方向。

```rust
#[derive(Clone, Copy)]
enum TransformDir { Forward, Inverse }

fn dit4_at<W: AsMut<[u8]>>(
    dir: TransformDir,
    work: &mut [W],
    /* ... */
) {
    // 统一实现，根据 dir 选择 LUT 和蝶形顺序
}
```

#### 2.9 `fft_dit2` / `ifft_dit2` 及其 `_lut` 变体重复
**位置**: `src/core/leopard_gf8/ops.rs`

4 个近乎相同的函数。`_non_lut` 变体仅在 FFT 最后阶段使用。

**改造方案**: 保留 `_lut` 变体作为核心实现，`_non_lut` 变体改为 wrapper 或内联消除。

#### 2.10 Unsafe 代码维护风险
**位置**: `encode.rs` `fft_dit4_at` / `ifft_dit4_at` (lines 185-199, 258-274)

使用 `unsafe` 裸指针算术绕过借用检查器。安全性注释正确，但任何索引逻辑变更都可能无声引入 UB。

**改造方案**:
- 使用 `get_unchecked_mut` 替代裸指针（同样 unsafe 但更清晰）
- 添加 `debug_assert!` 验证索引不重叠
- 考虑使用 `slice::split_at_mut` 安全地获取多个可变引用

```rust
// 安全替代方案
let (a, rest) = work.split_at_mut(r + 1);
let (b, rest) = rest.split_at_mut(dist);
let (c, d) = rest.split_at_mut(dist);
// a[r], b[0], c[0], d[0] 四个独立可变引用
```

#### 2.11 `leopard_env_enabled` 死代码
**位置**: `encode.rs:629-645`

标记 `#[allow(dead_code)]`，检查 `RSE_LEOPARD_GF8_XOR_CLONE` 环境变量但从未被调用。

**改造方案**: 要么集成到热路径中使用，要么删除。

---

### P3 - 架构

#### 2.12 LeopardGF8 是"原型骨架"——公共 API 全面拒绝
**位置**: `src/core/encode.rs`, `src/core/verify.rs`, `src/core/reconstruct.rs`

所有 `encode`、`verify`、`reconstruct` 方法对 LeopardGF8 返回 `Error::UnsupportedLeopardPrototype`。实际 FFT 编码路径仅通过 `pub(crate)` 函数可达。

**改造方案 (分阶段)**:

| 阶段 | 目标 | 工作量 |
|------|------|--------|
| Phase 1 | 通过 `encode_opt` 暴露 LeopardGF8 编码 | 小 |
| Phase 2 | 实现 LeopardGF8 的 `verify` | 中 |
| Phase 3 | 实现 LeopardGF8 的 `reconstruct` | 大 |
| Phase 4 | 支持 `ShardByShard` 增量编码 | 大 |

#### 2.13 Codec 分发机制不统一
**位置**: `src/core/encode.rs`

三种守卫模式并存:
- `self.ensure_classic_family_execution()?` — 拒绝 GF8 和 GF16
- `self.is_leopard_gf8_family()` — 仅检查 GF8
- `leopard_gf8_state().is_ok()` — 仅检查 GF8

**改造方案**: 统一为 trait-based dispatch。

```rust
trait CodecStrategy {
    fn encode(&self, shards: &mut [impl AsRef<[u8]> + AsMut<[u8]>]) -> Result<()>;
    fn verify(&self, shards: &[impl AsRef<[u8]>]) -> Result<()>;
    fn reconstruct(&self, shards: &mut [Option<impl AsRef<[u8]> + AsMut<[u8]>>]) -> Result<()>;
}
```

#### 2.14 `build_family_state` 创建零矩阵
**位置**: `src/core/leopard.rs:95`

`LeopardGF8Codec::new()` 接收一个零填充矩阵，而调用方已计算好完整的 Vandermonde 矩阵。这是死代码/未完成代码。

**改造方案**: 如果 LeopardGF8 不需要 Vandermonde 矩阵（使用 FFT），则清理此路径避免误导；如果需要，则传入实际矩阵。

#### 2.15 `LeopardGF16` 纯占位符
**位置**: `CodecFamily::LeopardGF16`, `FamilyState::LeopardGF16`

所有路径返回 `UnsupportedLeopardPrototype`。

**改造方案**: 明确标记为 `#[doc(hidden)]` 或在构造时直接返回错误，避免用户困惑。

---

### P4 - 测试与基准

#### 2.16 测试覆盖缺口

| 缺失项 | 影响 |
|--------|------|
| 编码正确性验证 | 无法确认 FFT 编码结果正确 |
| Small-file Leopard 覆盖 | 1K-512K 区间无 leopard 吞吐数据 |
| Profile 测试仅 2 个配置 | 无法观察小规模配置的内部结构 |
| A/B 测试仅 1 个配置 | `forward_tables` 变体从未被测试 |
| `decode_idx` / `reconstruct_some` 无 Criterion 基准 | 性能敏感操作缺少精确基准 |

#### 2.17 测试代码大量重复

以下函数在 `benchmark_smoke.rs` 和 `benchmark_small_files.rs` 间完全重复:
- `git_revision()`, `features()`, `backend()`, `backend_id()`, `backend_kind()`, `target_triple()` (~60 行)
- `ARTIFACT_SCHEMA_VERSION` 常量 (3 个文件)
- `run_operation()` 核心分发逻辑
- `write_results()` JSON/CSV 格式化样板

**改造方案**: 提取到 `tests/common/mod.rs` 或 `benches/common/mod.rs`。

#### 2.18 手写 JSON 序列化脆弱
**位置**: 所有 `write_*_results` 函数

使用 `format!()` 字符串插值构建 JSON，无转义处理。若字符串字段含双引号或反斜杠会产生非法 JSON。

**改造方案**: 引入 `serde_json` (dev-dependency) 安全序列化。

#### 2.19 `bandwidth.rs` 是遗留异类
**位置**: `benches/bandwidth.rs`

使用自己的 `create_shards()` (非确定性 `thread_rng()`)，不共享 `benches/common`，配置与主基准不一致。

**改造方案**: 标记为 legacy 或合并到 `throughput_matrix.rs`。

---

## 三、改造优先级路线图

### Phase 1: 基础加固 (1-2 周)

| 编号 | 任务 | 文件 | 优先级 |
|------|------|------|--------|
| R-01 | 新增编码正确性端到端测试 | `tests/`, `src/core/leopard_gf8/tests.rs` | P0 |
| R-02 | Profile 测试串行化或消除全局状态 | `tests/benchmark_smoke.rs`, `src/core/leopard_gf8/mod.rs` | P0 |
| R-03 | `slice_xor` 热路径 `assert_eq!` -> `debug_assert_eq!` | `src/core/leopard_gf8/ops.rs` | P1 |
| R-04 | Driver 魔法数字提取为命名常量 + 文档 | `src/core/leopard_gf8/driver.rs` | P1 |
| R-05 | 删除 `leopard_env_enabled` 死代码 | `src/core/leopard_gf8/encode.rs` | P2 |

### Phase 2: 可维护性提升 (2-3 周)

| 编号 | 任务 | 文件 | 优先级 |
|------|------|------|--------|
| R-06 | Profile 样板代码提取为辅助方法/宏 | `src/core/leopard_gf8/encode.rs`, `mod.rs` | P2 |
| R-07 | `fft_dit4_at` / `ifft_dit4_at` 参数化合并 | `src/core/leopard_gf8/encode.rs` | P2 |
| R-08 | `fft_dit2` / `ifft_dit2` 变体去重 | `src/core/leopard_gf8/ops.rs` | P2 |
| R-09 | Unsafe 代码改用 `split_at_mut` 或加强安全注释 | `src/core/leopard_gf8/encode.rs` | P2 |
| R-10 | 提取测试公共函数到 `tests/common/` | `tests/benchmark_smoke.rs`, `tests/benchmark_small_files.rs` | P4 |
| R-11 | `ARTIFACT_SCHEMA_VERSION` 统一到 `benches/common` | 3 个文件 | P4 |

### Phase 3: 性能优化 (3-4 周)

| 编号 | 任务 | 文件 | 优先级 |
|------|------|------|--------|
| R-12 | `slice_xor` SIMD 实现 (AVX2/NEON) | `src/core/leopard_gf8/ops.rs` | P1 |
| R-13 | 手写 JSON -> serde_json | `tests/benchmark_smoke.rs`, `tests/benchmark_small_files.rs` | P4 |
| R-14 | 增加 small-file leopard 覆盖 | `tests/benchmark_small_files.rs` | P4 |
| R-15 | 增加 profile 测试配置覆盖 | `tests/benchmark_smoke.rs` | P4 |

### Phase 4: 架构演进 (4-6 周)

| 编号 | 任务 | 文件 | 优先级 |
|------|------|------|--------|
| R-16 | 通过公共 API 暴露 LeopardGF8 编码 | `src/core/encode.rs`, `src/core/leopard_gf8/mod.rs` | P3 |
| R-17 | Codec 分发统一为 trait-based | `src/core/mod.rs`, `src/core/encode.rs` | P3 |
| R-18 | 清理 `build_family_state` 零矩阵问题 | `src/core/leopard.rs` | P3 |
| R-19 | `LeopardGF16` 标记 `#[doc(hidden)]` 或构造时拒绝 | `src/core/options.rs` | P3 |

---

## 四、风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| SIMD `slice_xor` 引入平台回归 | 中 | 高 | 多平台 CI + golden vector 测试 |
| 消除全局 PROFILE8 状态破坏 API 兼容 | 低 | 中 | 保留自由函数包装，内部改为实例持有 |
| `split_at_mut` 重构引入逻辑错误 | 低 | 高 | 保留 unsafe 版本作为参考，diff 对比验证 |
| trait-based dispatch 增加调用开销 | 低 | 低 | 单态化 + `#[inline]` 消除虚调用 |

---

## 五、关键指标

| 指标 | 当前 | 目标 |
|------|------|------|
| `encode.rs` 行数 | ~650 | ~450 (去除样板) |
| Profile 匹配分支重复次数 | 8 | 0 (统一为辅助方法) |
| `slice_xor` 吞吐 (AVX2) | 依赖自动向量化 | 显式 SIMD, 2-4x 提升 |
| 端到端正确性测试 | 0 | 3+ (含属性测试) |
| 公共 API 编码方法对 LeopardGF8 支持 | 0/10 | 1/10 (Phase 4 R-16) |
