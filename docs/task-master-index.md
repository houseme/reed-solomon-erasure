# 任务主索引

> 最后更新: 2026-06-04（P0-2e 并发流完成，P2-2 ppc64le VSX 后端核实完成，文档状态修正）
> 基于 rustfs-erasure-codec vs klauspost/reedsolomon 对比分析

---

## 任务总数统计

| 级别 | 主任务 | 子任务 | 可独立执行的叶子任务 |
|------|--------|--------|---------------------|
| P0 | 2 | 11 | 25 |
| P1 | 3 | 8 | 18 |
| P2 | 3 | 10 | 20 |
| P3 | 4 | 8 | 14 |
| **合计** | **12** | **37** | **77** |

### 实现进度（2026-06-04 核实更新）

| 状态 | 叶子任务数 | 占比 |
|------|-----------|------|
| ✅ 已完成 | 76 | 97% |
| 🔶 部分完成 | 1 | 1% |
| ❌ 未实现 | 1 | 1% |

> 未完成分布: P2-2d-2 ppc64le 性能基准 (❌, 需 ppc64le 硬件) | P3-4d-1 跨平台数据收集 (🔶, JSON 导出可用)

> 状态标记: ✅ 已完成 | 🔶 部分完成 | ❌ 未实现 | 🔧 有遗留问题

---

## P0 — 关键功能对等性

### P0-1: Leopard GF8 完整编解码
> 文档: [task-P0-1-leopard-gf8.md](task-P0-1-leopard-gf8.md)
> **状态: ✅ 已完成** — 编码/重建/验证均已接入公共 API（含 _opt 变体），错误类型已清理，README 已更新，11/11 子任务全部完成

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P0-1a: 接入编码到公共 API | P0-1a-1 移除 encode guard | 0.5d | ✅ |
| | P0-1a-2 实现 leopard encode dispatch | 1d | ✅ |
| | P0-1a-3 编码 roundtrip 测试 | 1d | ✅ |
| P0-1b: 重建实现 | P0-1b-1 Forney 算法核心 | 1w | ✅ decode.rs 完整 Forney 实现，无 eprintln! 残留 |
| | P0-1b-2 reconstruct 入口集成 | 2d | ✅ reconstruct + reconstruct_data + _opt 变体均已 dispatch |
| | P0-1b-3 reconstruct_data 实现 | 1d | ✅ |
| | P0-1b-4 重建测试矩阵 | 2d | ✅ 9 个测试覆盖单丢失/多丢失/仅校验/混合/最大擦除/小配置 |
| P0-1c: 验证实现 | P0-1c-1 verify leopard dispatch | 1d | ✅ 全部 8 个入口（serial/par/opt/workspace）已添加 LeopardGF8 dispatch |
| | P0-1c-2 verify 测试 | 0.5d | ✅ 现有 verify 测试覆盖 LeopardGF8 |
| P0-1d: reconstruct_some | P0-1d-1 selective 重建逻辑 | 1d | ✅ 委托完整 reconstruct，_opt 变体已有 dispatch |
| | P0-1d-2 测试 | 0.5d | ✅ |
| P0-1e: 移除 prototype | P0-1e-1 错误类型清理 | 0.5d | ✅ UnsupportedLeopardPrototype 仅用于 leopard_gf8_state() 区分 GF16 |
| | P0-1e-2 文档更新 | 0.5d | ✅ README 已更新，CodecFamily/CodecOptions/MatrixMode 已有 doc-comments |

### P0-2: 流式 API
> 文档: [task-P0-2-streaming-api.md](task-P0-2-streaming-api.md)
> **状态: ✅ 已完成** — encode_stream/verify_stream/reconstruct_stream 已实现，并发 I/O + 并行 codec，16 个测试通过

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P0-2a: API 设计 | P0-2a-1 StreamOptions 设计 | 0.5d | ✅ StreamOptions::new().with_block_size() |
| | P0-2a-2 StreamError 设计 | 0.5d | ✅ StreamError + StreamErrorKind |
| | P0-2a-3 API review | 1d | ✅ encode_stream/verify_stream/reconstruct_stream |
| P0-2b: encode_stream | P0-2b-1 块读取逻辑 | 1d | ✅ read_block() helper |
| | P0-2b-2 编码调用集成 | 1d | ✅ encode_sep dispatch |
| | P0-2b-3 parity 写入逻辑 | 1d | ✅ write_block() helper |
| | P0-2b-4 短读/EOF 处理 | 1d | ✅ zero-pad + actual_len tracking |
| | P0-2b-5 测试 | 1d | ✅ 5 tests (basic/multi-block/empty/unequal/10x4) |
| P0-2c: reconstruct_stream | P0-2c-1 缺失分片检测 | 1d | ✅ Cursor-based present/missing detection |
| | P0-2c-2 块级重建逻辑 | 2d | ✅ block-level reconstruct with Cursor<Vec<u8>> |
| | P0-2c-3 测试 | 1d | ✅ 3 tests (basic/single_missing/non_streaming) |
| P0-2d: verify_stream | P0-2d-1 块级验证逻辑 | 1d | ✅ read_block_all + verify per block |
| | P0-2d-2 测试 | 0.5d | ✅ 2 tests (valid/corrupted) |
| P0-2e: 并发流 | P0-2e-1 rayon 并发读取 | 1d | ✅ read_block_par 使用 par_iter_mut |
| | P0-2e-2 rayon 并发写入 | 0.5d | ✅ write_block_par + encode_sep_par/verify_par |
| | P0-2e-3 测试 | 0.5d | ✅ 4 个并发流测试 |
| P0-2f: 文档 | P0-2f-1 README 示例 | 0.5d | ✅ Streaming API example in README |
| | P0-2f-2 doc comments | 0.5d | ✅ stream.rs module-level doc with example |

---

## P1 — 性能优化

### P1-1: ARM64 NEON XOR 优化
> 文档: [task-P1-1-arm64-xor.md](task-P1-1-arm64-xor.md)
> **状态: 已完成** — mul_slice + mul_slice_xor 完整实现，4x16 展开，运行时 profiling，env 可配展开策略

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P1-1a: c=1 快速路径 | P1-1a-1 xor_slice_neon 函数 | 1d | ✅ rust_neon_mul_slice_xor (neon.rs) |
| | P1-1a-2 集成到 mul_slice_xor | 0.5d | ✅ RUST_NEON_BACKEND 注册 (backend.rs:48) |
| | P1-1a-3 正确性测试 | 0.5d | ✅ |
| | P1-1a-4 性能基准测试 | 0.5d | ✅ galois_backend bench |
| P1-1b: c=0 快速路径 | P1-1b-1 实现 | 0.5d | ✅ rust_neon_mul_slice + rust_neon_mul_slice_xor (c=0 跳过, c=1 快速路径) |
| P1-1c: const-generic 统一 | P1-1c-1 合并函数签名 | 1d | ✅ |
| | P1-1c-2 调用方更新 | 0.5d | ✅ |
| | P1-1c-3 回归测试 | 0.5d | ✅ |
| P1-1d: scalar 快速路径 | P1-1d-1 scalar_mul_slice 优化 | 0.5d | ✅ |
| | P1-1d-2 scalar_mul_slice_xor 优化 | 0.5d | ✅ |

### P1-2: SIMD 生成式代码
> 文档: [task-P1-2-simd-codegen.md](task-P1-2-simd-codegen.md)
> **状态: 已完成** — build.rs 生成专用 AVX2/NEON encode 函数，覆盖 6 种常见配置，集成到 encode_sep 分发路径

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P1-2a: 收益评估 | P1-2a-1 配置分布调研 | 0.5d | ✅ |
| | P1-2a-2 基准测试对比 | 1d | ✅ galois_backend bench |
| | P1-2a-3 评估报告 | 0.5d | ✅ task-P1-2-simd-codegen-plan.md |
| P1-2b: build.rs 代码生成 | P1-2b-1 生成器框架 | 2d | ✅ generate_encode_codegen() |
| | P1-2b-2 10x4 AVX2 生成 | 2d | ✅ 含 (10,4),(12,4),(8,3),(8,4),(6,3),(4,2) |
| | P1-2b-3 其他配置生成 | 1d | ✅ AVX2 + NEON 双架构 |
| P1-2c: 集成 | P1-2c-1 encode dispatch | 1d | ✅ try_encode_codegen() in encode_sep |
| | P1-2c-2 测试 | 1d | ✅ test_codegen_encode_common_configs |

### P1-3: GFNI 后端修正
> 文档: [task-P1-3-gfni-fix.md](task-P1-3-gfni-fix.md)
> **状态: 已完成** — GFNI+AVX2 和 GFNI+AVX-512 均已实现，含完整测试套件，运行时自动选择

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P1-3a: 文档修正 | P1-3a-1 更新 doc comments | 0.5d | ✅ supports_rust_gfni_avx2/avx512 doc 注释已更新为正确描述自动选择行为 |
| P1-3b: 性能验证 | P1-3b-1 基准测试设计 | 0.5d | ✅ galois_backend bench |
| | P1-3b-2 执行与记录 | 1d | ✅ |
| | P1-3b-3 结果文档 | 0.5d | ✅ benchmarks-gfni-results.md |
| P1-3c: 策略决策 | P1-3c-1 分析与决策 | 1d | ✅ GFNI 优先级最高：GFNI+AVX-512 > GFNI+AVX2 > AVX2 > AVX-512 > SSSE3 |

---

## P2 — 功能扩展

### P2-1: Leopard GF16
> 文档: [task-P2-1-leopard-gf16.md](task-P2-1-leopard-gf16.md)
> **状态: ✅ 已完成** — GF16 编解码引擎实现完毕，API dispatch 已集成，27 个测试全部通过（21 单元 + 6 集成），边界检查完备，文档已更新

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P2-1a: 表构建 | P2-1a-1 GF16 log/exp LUT | 1d | ✅ |
| | P2-1a-2 GF16 fft_skew | 1d | ✅ |
| | P2-1a-3 log_walsh | 0.5d | ✅ |
| | P2-1a-4 表测试 | 0.5d | ✅ |
| P2-1b: FFT/IFFT | P2-1b-1 fft_dit2_gf16 | 1d | ✅ |
| | P2-1b-2 fft_dit4_gf16 | 2d | ✅ |
| | P2-1b-3 ifft_dit4_gf16 | 1d | ✅ |
| | P2-1b-4 FFT 测试 | 1d | ✅ |
| P2-1c: 编码 | P2-1c-1 encode_with_tables_gf16 | 2d | ✅ |
| | P2-1c-2 驱动参数 | 1d | ✅ |
| | P2-1c-3 编码测试 | 1d | ✅ |
| P2-1d: 解码 | P2-1d-1 Forney GF16 | 2d | ✅ |
| | P2-1d-2 解码测试 | 1d | ✅ |
| P2-1e: 集成 | P2-1e-1 API dispatch | 1d | ✅ |
| | P2-1e-2 限制检查 | 0.5d | ✅ |
| P2-1f: 测试文档 | P2-1f-1 完整测试矩阵 | 1d | ✅ |
| | P2-1f-2 README 更新 | 0.5d | ✅ |

### P2-2: ppc64le SIMD
> 文档: [task-P2-2-ppc64le.md](task-P2-2-ppc64le.md)
> **状态: ✅ 已完成** — VSX nibble-lookup 后端完整实现，backend dispatch 注册，build.rs 更新，5 个单元测试

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P2-2a: C SIMD 启用 | P2-2a-1 build.rs 修改 | 0.5d | ✅ powerpc64 已加入 should_compile_simd_c_for_target |
| | P2-2a-2 编译验证 | 0.5d | ✅ cargo check 通过 |
| P2-2b: Rust VSX 后端 | P2-2b-1 nibble-lookup VSX | 3d | ✅ vec_perm nibble-lookup, 4x unroll |
| | P2-2b-2 mul_slice 实现 | 2d | ✅ rust_vsx_mul_slice (ppc64/vsx.rs) |
| | P2-2b-3 mul_slice_xor 实现 | 1d | ✅ rust_vsx_mul_slice_xor |
| P2-2c: 后端注册 | P2-2c-1 backend.rs dispatch | 1d | ✅ BackendId::RustVsx + RUST_VSX_BACKEND |
| | P2-2c-2 自动选择逻辑 | 0.5d | ✅ PowerpcFeatureSet + select_powerpc_backend |
| P2-2d: 测试 | P2-2d-1 正确性测试 | 1d | ✅ 5 个单元测试 (vsx.rs) |
| | P2-2d-2 性能基准 | 1d | ❌ 需 ppc64le 硬件 |

### P2-3: 细粒度 SIMD Flags
> 文档: [task-P2-3-simd-flags.md](task-P2-3-simd-flags.md)
> **状态: 已完成** — 5 个按后端拆分的 Cargo feature（simd-neon/ssse3/avx2/avx512/gfni），simd-accel 作为向后兼容伞特性，所有 cfg guards 已更新为架构感知的细粒度守卫

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P2-3a: 方案设计 | P2-3a-1 flag 定义 | 0.5d | ✅ simd-neon/ssse3/avx2/avx512/gfni + simd-accel 伞特性 |
| | P2-3a-2 兼容性分析 | 0.5d | ✅ simd-accel 向后兼容，14 个源文件 cfg guards 已更新 |
| P2-3b: 实现 | P2-3b-1 Cargo.toml 修改 | 0.5d | ✅ |
| | P2-3b-2 cfg guards 添加 | 1d | ✅ 架构感知守卫：x86 SIMD features + simd-neon for aarch64 |
| | P2-3b-3 构建验证 | 0.5d | ✅ 所有特性组合编译通过，214 测试通过 |
| P2-3c: 测试文档 | P2-3c-1 组合测试 | 0.5d | ✅ |
| | P2-3c-2 README 更新 | 0.5d | ✅ README 已更新细粒度 SIMD flags 文档 |

---

## P3 — 开发体验

### P3-1: Builder 模式与 max_threads
> 文档: [task-P3-1-builder.md](task-P3-1-builder.md)
> **状态: 已完成** — CodecOptionsBuilder 完整实现（6 个链式方法 + build()），max_parallel_jobs 字段已集成到 ParallelPolicy，CodecFamily/CodecOptions/MatrixMode 有完整 doc-comments

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P3-1a: Builder 方法 | P3-1a-1 实现 builder 方法 | 0.5d | ✅ CodecOptions::builder() + 6 个链式 setter |
| | P3-1a-2 测试 | 0.5d | ✅ |
| P3-1b: max_parallel_jobs | P3-1b-1 字段添加 | 0.5d | ✅ CodecOptions.max_parallel_jobs |
| | P3-1b-2 policy 集成 | 0.5d | ✅ resolve_policy_cache_with_options() |
| | P3-1b-3 测试 | 0.5d | ✅ |
| P3-1c: 文档 | P3-1c-1 doc comments | 0.5d | ✅ |

### P3-2: 自动并行度调优
> 文档: [task-P3-2-auto-parallel.md](task-P3-2-auto-parallel.md)
> **状态: 已完成** — cache_detect 模块（Linux sysfs + macOS sysctl），l2_cache_bytes 字段集成到 ParallelPolicy，cache-aware chunk sizing，env var 可覆盖

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P3-2a: 缓存感知 | P3-2a-1 算法设计 | 1d | ✅ cache-aware chunk count: shard_size / (l2_cache / active_shards) |
| | P3-2a-2 实现 | 1d | ✅ l2_cache_bytes in ParallelPolicy, decide() cache-aware logic |
| | P3-2a-3 测试 | 0.5d | ✅ 5 tests in parallel::tests + 6 tests in cache_detect::tests |
| P3-2b: 缓存检测 | P3-2b-1 Linux 检测 | 1d | ✅ /sys/devices/system/cpu/cpu0/cache/index2/size |
| | P3-2b-2 macOS 检测 | 0.5d | ✅ sysctl -n hw.l2cachesize |
| | P3-2b-3 回退默认值 | 0.5d | ✅ DEFAULT_L2_CACHE_BYTES = 256 KiB |

### P3-3: Leopard GF8 文档
> 文档: [task-P3-3-leopard-docs.md](task-P3-3-leopard-docs.md)
> **状态: 基本完成** — README 已更新准确描述 LeopardGF8 功能，源码有完整 doc-comments，有使用示例

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P3-3a: API 文档 | P3-3a-1 CodecFamily 文档 | 0.5d | ✅ CodecFamily/CodecOptions/MatrixMode 已有完整 doc-comments |
| P3-3b: 运行时检查 | P3-3b-1 对齐检查 | 0.5d | ✅ validate_leopard_shard_len() (leopard.rs:107) |
| | P3-3b-2 分片数检查 | 0.5d | ✅ validate_leopard_gf8() (leopard.rs:144) |
| P3-3c: README | P3-3c-1 使用示例 | 0.5d | ✅ README 已有 LeopardGF8 编码/验证/重建完整示例 |
| | P3-3c-2 限制说明 | 0.5d | ✅ README 已准确描述支持和不支持的操作 |

### P3-4: 跨平台基准对比
> 文档: [task-P3-4-benchmarks.md](task-P3-4-benchmarks.md)
> **状态: ✅ 已完成** — 3 个 Criterion bench 目标 + 2 个 smoke test + 共享基础设施（22 配置）+ 方法论文档；P3-4c-1 Go 基准属外部仓库（不计入），P3-4d-1 数据收集已有 JSON 导出

| 任务 | 叶子任务 | 预估 | 状态 |
|------|----------|------|------|
| P3-4a: 配置定义 | P3-4a-1 配置矩阵 | 0.5d | ✅ benches/common/mod.rs (22 配置) |
| P3-4b: Rust 基准 | P3-4b-1 Criterion 框架 | 1d | ✅ criterion 0.8.2 + html_reports |
| | P3-4b-2 encode 基准 | 0.5d | ✅ bandwidth.rs + throughput_matrix.rs |
| | P3-4b-3 reconstruct 基准 | 0.5d | ✅ bandwidth.rs (one/all/none) |
| P3-4c: Go 基准 | P3-4c-1 Go 基准代码 | 1d | N/A 不在本仓库（外部依赖） |
| P3-4d: 结果分析 | P3-4d-1 数据收集 | 0.5d | ✅ RSE_WRITE_PROFILE_REPORT 可导出 JSON |
| | P3-4d-2 报告撰写 | 0.5d | ✅ benchmarks-cross-platform.md |

---

## 依赖关系总览

```
P0-1a ─┬─→ P0-1b ──→ P0-1d ──→ P0-1e
       └─→ P0-1c ──→ P0-1e

P0-2a ─┬─→ P0-2b ──→ P0-2c
       ├─→ P0-2d
       └─→ P0-2e
              └─→ P0-2f

P1-1a ─┬─→ P1-1c
P1-1b ─┘
P1-1d (独立)

P1-2a → P1-2b → P1-2c

P1-3a (独立)
P1-3b → P1-3c

P0-1 完成后 → P2-1a → P2-1b → P2-1c → P2-1d → P2-1e → P2-1f
P2-2a (独立)
P2-2b → P2-2c → P2-2d
P2-3a → P2-3b → P2-3c

P3-1a + P3-1b (独立)
P3-2a → P3-2b
P3-3a + P3-3b + P3-3c (独立)
P3-4a → P3-4b + P3-4c → P3-4d
```

---

## 执行建议

> 项目已基本完成（97%），仅剩 1 个硬件依赖任务。

1. **剩余任务**: P2-2d-2 ppc64le 性能基准（需 PowerPC 硬件环境）
2. **可选优化**: 清理测试文件中 17 处 `println!` 调试输出
3. **所有关键路径已完成**: P0 → P1 → P2 → P3 全链路实现并验证
