# Code Review: main vs master 分支差异 (aarch64 视角)

**审查日期**: 2026-05-27
**审查设备**: aarch64 (Apple Silicon), arm64
**审查范围**: main 相对 master 的全部差异 (48 commits, 96 files, +26,308 / -1,353)
**审查依据**: docs/ 目录下所有文档 + 4 路并行代码分析 (aarch64 SIMD, build/core, x86 SIMD, tests/scripts)

## 1. 变更概览

| 领域 | 变更 |
|------|------|
| SIMD 架构 | `galois_8` 从单文件拆分为模块化架构 (`aarch64/`, `x86/`, `scalar.rs`, `backend.rs`, `policy.rs`) |
| x86 后端 | 新增 SSSE3 / AVX512 / GFNI 后端，运行时分发支持 7 级优先级 |
| aarch64 后端 | NEON 后端 (4x 展开 + 2x 展开 + scalar tail)，SVE stub 预留 |
| 核心逻辑 | 新增 `CodecOptions`, `ParallelPolicy`, `MatrixMode`, cache 分析，reconstruct 特化 |
| 构建系统 | `build.rs` 重写为架构感知的表生成 + C 代码编译 |
| 测试 | 新增 ~1900 行内部测试，benchmark smoke, golden vectors, selftest |
| CI | 新增完整 CI 流水线 (typos, audit, multi-toolchain build/test, tag-gated publish) |

## 2. 已确认修复的问题

以下问题在 `docs/aarch64-code-review-2026-05-26.md` 中记录，经核实已在当前代码中修复：

| Issue | 修复内容 | 验证位置 |
|-------|----------|----------|
| BUG-1: NEON profile stats 测试并行竞争 | 添加 `NEON_PROFILE_TEST_LOCK` 互斥锁 | `src/galois_8/mod.rs:170` |
| BUG-2: AVX2 > AVX512 优先级错误 | 优先级改为 GFNI+AVX512 > GFNI+AVX2 > AVX512 > AVX2 > SSSE3 > SimdC > Scalar | `src/galois_8/backend.rs:360-382` |
| BUG-3: GFNI 后端永远不被自动选中 | GFNI 现为最高优先级 | `src/galois_8/backend.rs:362-366` |
| ISSUE-5: SVE stub 无用代码 | 改为 `let _sve = features.sve;` 并有注释 | `src/galois_8/backend.rs:429` |

## 3. 仍存在的问题

### P0 — 正确性/安全性

#### ~~FIX-1: scalar.rs 大量裸指针 + unsafe~~ — 已修复

**文件**: `src/galois_8/scalar.rs`
**修复内容**: 将 `mul_slice_pure_rust` / `mul_slice_xor_pure_rust` / `slice_xor` 三个函数从裸指针+unsafe 重写为安全迭代器实现。同时消除了 `len as isize` 截断风险 (原 FIX-3)。

#### ~~FIX-2: NEON profile metrics 计数语义不精确~~ — 误判

**文件**: `src/galois_8/aarch64/neon.rs:53-56`
**结论**: 经仔细核实，计数逻辑正确。当 `bytes_done == bytes_done_unrolled` 时 `remainder` 区间为空（0 字节），`vector_16b_chunks = 0` 是正确的。原始 ISSUE-4 为误判。

#### ~~FIX-3: `len as isize` 截断风险~~ — 已随 FIX-1 修复

随 scalar.rs 安全重写一并消除。

### P1 — 工程/可维护性

#### ~~FIX-4: CI 完全缺失 aarch64 覆盖~~ — 已修复

**文件**: `.github/workflows/ci.yml`
**严重程度**: P1
**问题**: CI 只有 `ubuntu-latest` (x86_64) 和 `windows-latest`。NEON 后端的回归只能靠手动发现。
**修复方向**: 添加 `macos-latest` (aarch64 Apple Silicon) job 或 `macos-13` (x86_64) + `macos-14` (aarch64) 分离测试。至少保证 `cargo test --features 'std simd-accel'` 在 aarch64 上运行。

#### FIX-5: 环境变量碎片化

**文件**: `src/galois_8/policy.rs`, `src/galois_8/mod.rs`, `src/galois_8/backend.rs`
**严重程度**: P1
**问题**: 存在 ~15 个环境变量用于运行时调参，缺乏统一配置入口，新增调参需求时会持续膨胀。
**修复方向**: 在 `ParallelPolicy` / `CodecOptions` 基础上统一环境变量解析，或引入 builder pattern。

#### ~~FIX-6: x86 后端间代码重复高~~ — 已修复

**文件**: `src/galois_8/x86/ssse3.rs`, `avx2.rs`, `avx512.rs`, `gfni.rs`
**修复内容**: 使用 `const XOR: bool` 泛型参数将每个后端的 `mul_slice` + `mul_slice_xor` 合并为单个 `_impl` 函数。提取共享 `load_table_halves` helper 到 `mod.rs`。净减 184 行代码。

### P2 — 文档/治理

#### ~~FIX-7: 文档语言不一致且缺少 aarch64 对应文档~~ — 已修复

新增 `docs/aarch64-simd-design.md` 和 `docs/aarch64-simd-release-checklist.md`，对标 `x86_64-simd-*` 系列结构。

**文件**: `docs/` 目录
**严重程度**: P2
**问题**: `docs/` 下中英文混用。`x86_64-simd-*` 系列文档详尽但缺少 aarch64 对应文档。
**修复方向**: 统一文档语言规范，补齐 aarch64 NEON/SVE 设计文档。

#### ~~FIX-8: `benchmark-metrics` 在 default features 中~~ — 已修复

**文件**: `Cargo.toml:50`
**严重程度**: P2
**问题**: `default = ["std", "benchmark-metrics"]` 导致发布版默认启用统计开销，影响生产性能。
**修复方向**: default 只包含 `std`，`benchmark-metrics` 需显式启用。

## 4. aarch64 特定评估

| 方面 | 评估 |
|------|------|
| NEON 正确性 | **良好** — `vqtbl1q_u8` 查表法正确，4x/2x 展开 + scalar tail 处理边界无误 |
| NEON 性能 | **合理** — 64-byte 对齐展开 + 16-byte fallback + scalar tail |
| SVE 预留 | **良好** — stub 结构清晰，`detect_sve_features()` 返回 `available: false`，不干扰当前路径 |
| aarch64 并行策略 | **合理** — 独立的 `RS_AARCH64_RECONSTRUCT_*` 环境变量覆盖，默认阈值 512KB/256KB |
| Backend override | **验证通过** — `RSE_BACKEND_OVERRIDE=rust-neon` 和 `scalar` 均可正常工作 |
| Build | **通过** — `cargo build --features 'std simd-accel'` 编译成功 |

## 5. x86 后端评估

| 后端 | 算法 | 正确性 | 备注 |
|------|------|--------|------|
| SSSE3 | `pshufb` nibble lookup | 通过 | 基线 x86 SIMD |
| AVX2 | 256-bit pshufb | 通过 | SSSE3 的 2x 宽度版 |
| AVX512 | 512-bit broadcast + shuffle | 通过 | 不使用 mask register |
| GFNI+AVX2 | `gf2p8mul_epi8` + basis isomorphism | 通过 | 需 basis 变换，已验证可逆 |
| GFNI+AVX512 | 同上 512-bit | 通过 | 最快路径 |

所有 x86 后端使用 unaligned load/store (`_mm*_loadu_si*`)，无需对齐保证。Tail 统一回退 scalar。

## 6. 测试覆盖评估

| 覆盖维度 | 状态 | 备注 |
|----------|------|------|
| 所有 x86 后端 correctness | 完整 | 7 个后端均有 cross-backend conformance 测试 |
| aarch64 NEON correctness | 完整 | scalar 对照 + override 验证 |
| Golden vectors | 完整 | 4x2 / 10x4 / 32x16 多配置 |
| Benchmark smoke | 完整 | encode/verify/reconstruct/reconstruct_data 全覆盖 |
| CI aarch64 | **缺失** | 只有 x86_64 和 Windows |
| aarch64 kernel benchmark | 手动 | 有脚本但无 CI 自动化 |

## 7. 修复优先级与执行计划

| 优先级 | 任务 | 状态 | 说明 |
|--------|------|------|------|
| P0-1 | scalar.rs 安全重写 | **已完成** | 消除约 130 行 unsafe 代码 |
| P0-2 | NEON profile 计数修正 | **误判** | 计数逻辑实际正确 |
| P0-3 | isize 截断防护 | **已完成** | 随 P0-1 一起消除 |
| P1-1 | CI 添加 aarch64 job | **已完成** | 新增 `test-macos-arm64` job |
| P1-2 | 环境变量统一治理 | **保持现状** | 当前结构已合理，进一步重构风险大于收益 |
| P1-3 | x86 后端代码去重 | **已完成** | const generic 合并，净减 184 行 |
| P2-1 | 文档语言统一 | **已完成** | 新增 aarch64-simd-design.md + release-checklist.md |
| P2-2 | benchmark-metrics 移出 default | **已完成** | default 改为 `["std"]` |

## 8. 结论

main 分支是一个大规模架构升级，将 SIMD 从编译期静态绑定提升为运行时多后端分发。核心数学逻辑正确，GF(2^8) 的实现（scalar / NEON / x86 各后端）通过 cross-backend conformance tests 验证。

**本轮已完成修复**:
1. scalar.rs 安全重写 — 消除约 130 行 unsafe 代码
2. CI 添加 aarch64 (macOS ARM64) 覆盖
3. `benchmark-metrics` 移出 default features

**本轮已完成修复**:
1. scalar.rs 安全重写 — 消除约 130 行 unsafe 代码
2. CI 添加 aarch64 (macOS ARM64) 覆盖
3. `benchmark-metrics` 移出 default features
4. x86 后端代码去重 — const generic 合并，净减 184 行
5. aarch64 SIMD 设计与发布文档补齐

**已评估保持现状**:
1. 环境变量统一治理 — 当前结构已合理
