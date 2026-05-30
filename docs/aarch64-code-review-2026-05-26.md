# Code Review: main vs master 分支差异 (aarch64 视角)

**审查日期**: 2026-05-26
**审查设备**: aarch64 (Apple Silicon)
**审查范围**: main 相对 master 的全部差异，结合 docs/ 文档

## 已验证

- `cargo build --features 'std simd-accel'` 通过
- `cargo test --release --features 'std simd-accel' -- rust_neon` 全部通过
- `cargo test --release --features 'std simd-accel' -- mul_slice` 通过
- `cargo test --release --features 'std simd-accel' -- active_backend` 通过

## 发现的问题

### BUG-1: NEON profile stats 测试存在并行竞争 (严重)

**文件**: `src/galois_8/mod.rs:602-624`
**问题**: `test_rust_neon_profile_stats_track_vector_vs_tail` 依赖全局静态 `RUST_NEON_PROFILE_METRICS`，但不阻止并行执行。当多个测试同时运行时，其他测试（如 `test_rust_neon_matches_scalar_mul_slice`）也会调用 `rust_neon_mul_slice`，污染计数器。
**表现**: 并行 `cargo test` 时偶发 `tail_calls` 断言失败 (期望 2，实际 1 或更大偏差)。
**修复建议**: 使用 `std::sync::Mutex` 或序列化标记保护此测试；或者在测试中使用独立的局部计数器而非全局静态。

### BUG-2: x86 backend 优先级逻辑错误 (严重)

**文件**: `src/galois_8/backend.rs:360-374`
```rust
fn select_x86_backend(features: X86FeatureSet) -> GaloisBackend {
    if supports_rust_avx2(features) {      // 优先级 1: AVX2
        return RUST_AVX2_BACKEND;
    }
    if supports_rust_avx512(features) {    // 优先级 2: AVX512
        return RUST_AVX512_BACKEND;
    }
    ...
}
```
**问题**: AVX2 优先于 AVX512，但 AVX512 是更宽的 SIMD 指令集，通常性能更高。在同时支持 AVX2 和 AVX512 的 CPU 上（如测试用例 `test_select_x86_backend_priority` 所验证的），永远不会选择 AVX512。
**注意**: 测试 `test_select_x86_backend_priority` 确认了这个行为，说明可能是**有意为之**（AVX2 在某些场景下因频率更高而实际更快），但文档未说明原因。
**建议**: 如果是刻意选择，应添加注释说明原因；如果不是，应调换优先级。

### BUG-3: GFNI 后端永远不会被自动选中 (严重)

**文件**: `src/galois_8/backend.rs:360-374`
**问题**: `select_x86_backend` 中没有 `supports_rust_gfni_avx2` 或 `supports_rust_gfni_avx512` 的检查。GFNI 后端只能通过 `RSE_BACKEND_OVERRIDE` 环境变量手动选择，运行时自动分发永远不会选中它。
**影响**: 在支持 GFNI 的 CPU 上，错过了潜在的性能提升。

### ISSUE-4: NEON mul_slice profile metrics 计数可能不准确 (中等)

**文件**: `src/galois_8/aarch64/neon.rs:41-116`
**问题**: `rust_neon_mul_slice_impl` 中 profile 记录的 `vector_64b_chunks` 和 `vector_16b_chunks` 计算逻辑需要验证：当 `bytes_done_unrolled == bytes_done` 时，`vector_16b_chunks` 为 0，但 `remainder` 阶段仍有 16-byte 块处理。
**建议**: 验证 profile 数据的准确性，确保 tail 和 vector chunk 的计数逻辑一致。

### ISSUE-5: SVE stub 中的无用 `let _ = features.sve;` (轻微)

**文件**: `src/galois_8/backend.rs:421`
```rust
fn select_aarch64_backend(features: Aarch64FeatureSet) -> GaloisBackend {
    let _ = features.sve;  // 无操作
```
**问题**: 这行代码只是为了抑制 "unused" 警告，但更好的做法是直接使用 `_` 前缀或重构。

### ISSUE-6: scalar.rs 使用裸指针而非安全抽象 (轻微)

**文件**: `src/galois_8/scalar.rs`
**问题**: `mul_slice_pure_rust` 和 `mul_slice_xor_pure_rust` 大量使用裸指针和 `unsafe`，但这些操作完全可以用安全的迭代器实现，性能差异在现代编译器优化下可以忽略。
**建议**: 考虑用 `chunks_exact` + `zip` 重写，减少 unsafe 代码量。

### ISSUE-7: 环境变量过多且缺乏统一管理 (中等)

**问题**: 存在大量环境变量用于运行时调参：
- `RSE_BACKEND_OVERRIDE`
- `RS_NEON_MUL_SLICE_XOR_UNROLL`
- `RS_NEON_MUL_SLICE_XOR_SCHEDULE`
- `RS_RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES`
- `RS_RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES`
- `RS_RECONSTRUCT_MIN_BYTES_PER_JOB`
- `RS_AARCH64_RECONSTRUCT_*` (5 个)
- `RS_PARALLEL_POLICY_*` (3 个)
- `RUST_REED_SOLOMON_ERASURE_ARCH`
**建议**: 考虑统一为结构化配置（如 TOML 配置文件或 builder pattern），减少环境变量碎片化。

### ISSUE-8: CI 缺少 aarch64 测试矩阵

**文件**: `.github/workflows/ci.yml`
**问题**: CI 只在 `ubuntu-latest` (x86_64) 和 `windows-latest` 上运行，没有 aarch64 测试。虽然 aarch64 NEON 代码有条件编译保护，但缺乏 CI 覆盖意味着 aarch64 回归只能靠本地发现。
**建议**: 添加 `ubuntu-latest-arm64` 或 QEMU 交叉编译检查。

## 文档质量评估

docs/ 目录文档详尽，覆盖了：
- 实施 playbook (`ec-implementation-playbook.md`)
- 各阶段设计 (`ec-phase-1` 到 `ec-phase-6`)
- x86 SIMD 详细设计和验证 (`x86_64-simd-*`)
- Benchmark 方法论 (`benchmark-methodology.md`)
- 任务拆分 (`task-00` 到 `task-07`)

**问题**:
1. 文档中有大量中文内容，但也有英文文档，语言不一致
2. `x86_64-simd-*` 系列文档详尽但缺少 aarch64 对应文档
3. 文档中的 "已完成" 标记与实际代码状态需交叉验证

## aarch64 特定评估

**NEON 后端**:
- 实现正确，使用 `vqtbl1q_u8` 查表法，与 x86 的 SSSE3 `pshufb` 等价
- 4x 展开 (64-byte) 与 2x 展开 (32-byte) 策略合理
- Tail 回退到 scalar 正确
- Profile metrics 设计良好，但存在 ISSUE-4 的计数准确性问题

**SVE 后端**:
- 仅有 stub，`available: false`，设计合理
- 检测代码 (`detect_sve_features`) 当前总是返回 false
- 模块布局为未来扩展预留了空间

**并行策略**:
- `policy.rs` 为 aarch64 提供了独立的环境变量覆盖
- `RS_AARCH64_RECONSTRUCT_*` 系列允许精细调参
- 默认阈值合理 (512KB data-only, 256KB full reconstruct)

## 建议的修复优先级

1. **BUG-1**: 修复 profile stats 测试的竞争条件 ✅
2. **BUG-3**: 将 GFNI 纳入 `select_x86_backend` 自动选择逻辑 ✅
3. **BUG-2**: 修正 AVX512 > AVX2 优先级 ✅
4. **ISSUE-8**: CI 添加 aarch64 覆盖
5. **ISSUE-7**: 统一环境变量配置方案
6. **ISSUE-4**: 验证 profile 数据准确性
7. **ISSUE-5**: 清理 SVE stub 代码 ✅
8. **ISSUE-6**: scalar.rs 代码清理

## 已完成的修复

### BUG-1: NEON profile stats 测试并行竞争 ✅
- 在涉及 NEON profile metrics 的 4 个测试中添加 `NEON_PROFILE_TEST_LOCK` 互斥锁
- 文件: `src/galois_8/mod.rs`

### BUG-2/3: x86 backend 优先级修正 ✅
- 新优先级: GFNI+AVX512 > GFNI+AVX2 > AVX512 > AVX2 > SSSE3 > SimdC > Scalar
- GFNI 后端现可被运行时自动选中
- 文件: `src/galois_8/backend.rs`, `src/galois_8/mod.rs`

### ISSUE-5: SVE stub 清理 ✅
- `let _ = features.sve;` → `let _sve = features.sve;` 添加说明注释
- 文件: `src/galois_8/backend.rs`

### 预存测试修复 ✅
- `test_aarch64_reconstruct_stage_policies_allow_data_parity_split`: 修正 env var 名称（`RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB` → `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB`）
- `test_reconstruct_parallel_policy_has_data_only_and_full_tiers`: 移除 aarch64 上不适用的 `!data_only.use_parallel` 断言
- 文件: `src/tests/mod.rs`
