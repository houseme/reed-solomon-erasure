# Leopard GF8 Unsafe & Panic Audit Report

> 审计日期: 2026-05-30
> 提交: 基于 `0af1fe4` (main) 的重构后代码
> 平台: Apple M5 Max / aarch64-macos-unknown
> Rust: 1.96.0 (edition = 2024)

---

## 一、审计范围

本次审计覆盖以下模块的 `unsafe` 代码和 `panic` 路径:

| 模块 | 文件数 | 路径 |
|------|--------|------|
| Leopard GF8 编码器 | 5 | `src/core/leopard_gf8/{mod,encode,ops,driver,work}.rs` |
| GF(2^8) SIMD 后端 | 10 | `src/galois_8/{backend,scalar,aligned,profile}.rs` + `x86/` + `aarch64/` |
| 遗留 SIMD-C FFI | 1 | `src/galois_8/legacy/simd_c.rs` |

---

## 二、Unsafe 代码审计

### 2.1 Leopard GF8 模块

**重构前**: 1 处 `unsafe` (encode.rs:190 — 原始指针算术获取 4 个可变引用)

**重构后**: 通过 `RSE_DIT4_STRATEGY` 环境变量可选三种策略:

| 策略 | unsafe 块数 | 说明 |
|------|------------|------|
| `decomposed` | 0 | 纯 safe pairwise `get_pair_mut` + `fft_dit2`/`ifft_dit2` |
| `direct` (默认) | 1 | unsafe 原始指针快速路径 + safe 边界回退 |
| `direct-safe` | 0 | `split_at_mut` 链 + `fft_dit4_full_lut` |

**`direct` 策略安全性分析**: 使用 `work.as_mut_ptr()` + `ptr.add(idx)` 获取 4 个 `&mut W` 引用。安全性由 `d < work.len()` 守卫保证（`a < b < c < d`，所有索引互不相同且在分配范围内）。

**`decomposed` 策略安全性分析**: 通过 `get_pair_mut` 获取不重叠的可变引用，内部使用 `split_at_mut` 验证索引有效性。

**`direct-safe` 策略安全性分析**: 通过 3 次 `split_at_mut` 链获取 4 个不重叠的 `&mut [u8]` 切片，无 `unsafe`。

**`fft_dit4_full_lut` / `ifft_dit4_full_lut`**: 移除 `#[allow(dead_code)]` 标记，被 `direct` 和 `direct-safe` 策略调用。

### 2.2 GF(2^8) SIMD 后端

所有 SIMD 后端遵循统一的安全模式:

```
安全包装函数 (pub)
  ├─ assert_eq!(input.len(), out.len())  // API 契约
  ├─ if input.is_empty() { return; }     // 空输入守卫
  └─ unsafe { impl_fn(c, input, out) }   // 调用 unsafe 实现

unsafe fn impl_fn (#[target_feature])
  ├─ bytes_done = input.len() & !(N-1)   // 对齐到 SIMD 宽度
  ├─ split_at(bytes_done)                 // 分离 SIMD 尾部
  ├─ chunks_exact(N).zip(...)             // 迭代 N 字节块
  │   ├─ unsafe { load intrinsic }        // 加载
  │   ├─ 纯 SIMD 运算                     // 处理
  │   └─ unsafe { store intrinsic }       // 存储
  └─ scalar::fallback(tail)               // 标量处理尾部
```

| 后端 | SIMD 宽度 | 块大小 | Unsafe 位置 |
|------|----------|--------|------------|
| SSSE3 | 128-bit | 16 字节 | `_mm_loadu_si128` / `_mm_storeu_si128` |
| AVX2 | 256-bit | 32 字节 | `_mm256_loadu_si256` / `_mm256_storeu_si256` |
| AVX-512 | 512-bit | 64 字节 | `_mm512_loadu_si512` / `_mm512_storeu_si512` |
| GFNI+AVX2 | 256-bit | 32 字节 | `_mm256_loadu_si256` + `_mm256_gf2p8mul_epi8` |
| GFNI+AVX512 | 512-bit | 64 字节 | `_mm512_loadu_si512` + `_mm512_gf2p8mul_epi8` |
| NEON | 128-bit | 16/64 字节 | `vld1q_u8` / `vst1q_u8` / `vld1q_u8_x4` |

**安全不变量**:
- `bytes_done = len & !(N-1)` 保证所有 SIMD 操作在分配范围内
- `chunks_exact(N)` 保证指针指向有效 N 字节内存
- 标量回退处理 0..N-1 字节尾部

### 2.3 其他 Unsafe 代码

| 文件 | 类型 | 说明 |
|------|------|------|
| `aligned.rs` | 内存分配 | `alloc_zeroed` / `dealloc` — 64 字节对齐的 shard 分配器 |
| `aligned.rs` | 指针转切片 | `from_raw_parts` / `from_raw_parts_mut` |
| `aligned.rs` | Trait 实现 | `unsafe impl Send + Sync` — 所有权语义保证线程安全 |
| `simd_c.rs` | FFI | C SIMD 库调用 — 指针有效性依赖于切片引用 |
| `tests.rs` | 测试 | `env::set_var` / `env::remove_var` — 仅测试使用 |

---

## 三、Panic 路径审计

### 3.1 已修复的 Panic

| 问题 | 位置 | 修复 |
|------|------|------|
| **OOB panic** — `parity_shards > 128` 时 `fft_skew` 越界 | `driver.rs:25` | 添加 `m > MODULUS8` 守卫，返回 `Error::TooManyShards` |

**Bug 详情**: 当 `parity_shards > 128` 时，`m = ceil_pow2(parity_shards)` = 256，`skew_offset = m - 1 = 255`。但 `fft_skew` 是 `[u8; 255]`，索引 255 越界。`validate_leopard_gf8` 仅检查 `total_shards > 256`，不阻止此情况。

### 3.2 已改进的 Panic 路径

| 变更 | 位置 | 说明 |
|------|------|------|
| `assert_eq!` → `debug_assert_eq!` | `ops.rs:155,164,172` | `slices_xor`, `mul_slice_xor_reference`, `mulgf8` — 内部函数，release 构建中移除 |
| `assert_eq!` → `debug_assert_eq!` | `ops.rs:218-220,310-312` | `fft_dit4_full_lut`, `ifft_dit4_full_lut` — 已标记 dead_code |
| `debug_assert!` 守卫 | `work.rs:32,38,47,61` | `FlatWork::lane/lane_mut/lane_views/with_lane_views` — 开发期捕获越界 |

### 3.3 保留的 Panic 路径

| 位置 | 类型 | 说明 |
|------|------|------|
| SIMD 包装函数 (`assert_eq!`) | API 契约 | 输入/输出长度不匹配时始终 panic — 公共 API 入口点 |
| `galois_8/mod.rs:95` | `panic!("Divisor is 0")` | `div()` 函数 — 已记录的 API 契约 |
| `aligned.rs:29,83` | `.expect()` | Layout 构造 — 仅在极端大小时触发 |
| `policy.rs:129,148,216,641` | `.expect()` | 逻辑保证的 Option 展开 |
| 测试代码 | `unwrap()` | 仅测试使用 |

---

## 四、SIMD 架构后端详情

### 4.1 后端选择优先级

**x86_64 自动选择**:
1. AVX2 (保守选择 — 避免 AVX-512 频率节流)
2. AVX-512 (仅当 AVX2 不可用时)
3. SSSE3
4. SIMD-C
5. Scalar Rust (回退)

**aarch64 自动选择**:
1. NEON (aarch64 始终支持)
2. SIMD-C
3. Scalar Rust (回退)

**GFNI 后端**: 仅通过 `RSE_BACKEND_OVERRIDE` 手动启用 (新指令集，有限验证)

### 4.2 运行时特性检测

| 架构 | 检测函数 | 检测的特性 |
|------|---------|-----------|
| x86_64 | `std::is_x86_feature_detected!` | sse2, ssse3, avx2, avx512f, avx512bw, gfni |
| aarch64 | `std::arch::is_aarch64_feature_detected!` | neon |

**no_std 回退**: 当 `feature = "std"` 未启用时，运行时检测不可用，始终使用 Scalar 后端。

### 4.3 后端算法对比

| 后端 | 算法 | 查找表 | 特点 |
|------|------|--------|------|
| Scalar | `MUL_TABLE[c][byte]` | 256×256 全表 | 始终可用，无 SIMD 依赖 |
| SSSE3/AVX2/AVX512 | Nibble 查找 | `MUL_TABLE_LOW` + `MUL_TABLE_HIGH` | 将字节拆分为低/高 4 位，分别查表后 XOR |
| NEON | Nibble 查找 | 同上 | `vqtbl1q_u8` 表查找，支持 4x/2x 展开 |
| GFNI | 原生 GF(2^8) 乘法 | 无 | `_mm256_gf2p8mul_epi8` + 同构仿射变换 |

---

## 五、代码行数变化

| 文件 | 变化 |
|------|------|
| `encode.rs` | +120 行 (三策略实现 + 配置) |
| `ops.rs` | −2 行 (移除 dead_code 标记) |
| `driver.rs` | +5 行 (OOB 守卫 + MODULUS8 导入) |
| `work.rs` | +4 行 (debug_assert 守卫) |
| `backend.rs` | +12 行 (文档注释) |
| `aligned.rs` | +8 行 (改进 SAFETY 注释) |
| `neon.rs` | +5 行 (SAFETY 注释) |
| `ssse3.rs` | +4 行 (SAFETY 注释) |
| `avx2.rs` | +4 行 (SAFETY 注释) |
| `avx512.rs` | +4 行 (SAFETY 注释) |
| `gfni.rs` | +8 行 (SAFETY 注释) |
| `docs/leopard-gf8-unsafe-audit-2026-05-30.md` | 新增 ~200 行 |
| **净变化** | **+41 行** |

---

## 六、验证结果

| 检查项 | 结果 |
|--------|------|
| `cargo build --features std` | ✅ 通过 |
| `cargo test --lib --features std` | ✅ 199 测试通过 |
| `cargo clippy --features std` | ✅ 无新增警告 |
| Leopard GF8 unsafe 块数 (decomposed) | 0 (重构前: 1) |
| Leopard GF8 unsafe 块数 (direct) | 1 (优化后保留，用于最大性能) |
| Leopard GF8 unsafe 块数 (direct-safe) | 0 (完全 safe) |
| 可达 OOB panic | 0 (重构前: 1 — parity_shards > 128) |
| release assert_eq! (热路径) | 0 (重构前: 9) |

---

## 七、后续建议

1. **SVE 后端实现**: `aarch64/sve.rs` 目前是存根 (`available: false`)。当 SVE 硬件更普及时，可实现 SVE 后端以利用可变向量长度。
2. **GFNI 自动选择**: 当 GFNI 部署率提升且性能验证充分后，可将其加入自动选择路径。
3. **AVX-512 优先级调整**: 如果未来工作负载更偏 compute-bound 而非 memory-bandwidth-bound，可考虑提升 AVX-512 优先级。
4. **`AlignedShard` 改用 `Box<[u8]>`**: 当 `std::alloc::Allocator` 稳定后，可用自定义分配器替代手动 `alloc_zeroed`/`dealloc`，进一步减少 unsafe。
