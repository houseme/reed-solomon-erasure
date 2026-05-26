# 阶段 4：SIMD 架构升级与运行时分发

## 1. 阶段目标

将当前 crate 的 SIMD 能力从“编译期开关 + 静态路径”升级为“运行时多 ISA 分发 + 后端可扩展架构”。

## 2. 为什么这个阶段重要

当前 SIMD 不是没有，而是架构还不够现代：

- 构建期选路
- 通用分发不够灵活
- 缺少高层 dispatch 层
- 缺少 GFNI 的显式优先级控制

相比之下，MinIO 所依赖的底层库在这方面更成熟。

## 3. 改造目标

1. 运行时探测 CPU 能力
2. 运行时选择最优后端
3. 保留 scalar fallback
4. 支持 x86_64 与 aarch64 的多后端
5. 后端与 core 编码逻辑解耦
6. 明确 C 与 Rust 双路线的迁移策略

## 3.1 SIMD 实现语言决策

本阶段明确采用以下技术决策：

1. 现有 `C` SIMD 内核继续保留
2. 现有 `C` SIMD 内核不再被视为长期唯一实现
3. 后续新增 SIMD 后端优先采用 Rust `std::arch`
4. 全量 Rust 重写不在本阶段一次性完成
5. C 与 Rust 后端必须并存一段时间，并通过一致性测试对照

### 为什么不继续长期纯 C

原因：

- 构建与交叉编译更复杂
- 与 Rust 核心逻辑边界较硬
- runtime dispatch 管理不够自然
- 后续接入新 ISA 时维护成本更高

### 为什么不现在立刻全量改写为 Rust

原因：

- 范围过大，风险高
- GFNI / AVX512 / NEON 这些路径重写难度高
- 容易在短期内丢失已有性能优势
- 若没有 benchmark 基线和 backend 抽象，重写价值难证明

### 最终方向

最终技术方向不是“继续 C”也不是“立刻纯 Rust”，而是：

- 短期：C 作为稳定高性能基线
- 中期：Rust backend 逐步接管
- 长期：Rust 成为主实现，C 仅保留为 fallback 或 legacy feature

## 4. 建议架构

### 4.1 backend 层

建议抽象出：

```rust
struct GaloisBackend {
    mul_slice: fn(u8, &[u8], &mut [u8]),
    mul_slice_xor: fn(u8, &[u8], &mut [u8]),
    name: &'static str,
}
```

### 4.2 backend 分类

建议后端至少包含：

- scalar
- sse2
- ssse3
- avx2
- avx512
- gfni
- neon

### 4.3 运行时选择

初始化时探测：

- x86_64:
  - SSE2
  - SSSE3
  - AVX2
  - AVX512
  - GFNI
- aarch64:
  - NEON
  - 后续可扩展 SVE

### 4.4 与现有 C SIMD 的关系

第一步不需要立刻完全放弃 `simd_c/reedsolomon.c`。

建议策略：

1. 先保留现有 C 内核
2. 增加 runtime dispatch 包装层
3. 将 C 路径降级为 backend，而不是默认唯一实现
4. 新增 Rust intrinsic/backend 路径并并存验证
5. 再逐步迁移到 Rust 主导实现

### 4.5 Rust 后端引入策略

建议的引入顺序如下：

1. `scalar`：先整理纯 Rust 标量路径，作为对照基线
2. `ssse3` / `avx2`：优先迁移中等复杂度且收益稳定的 x86 路径
3. `neon`：补齐 ARM64 现代路径
4. `gfni`：在 runtime dispatch 和 benchmark 稳定后再引入
5. `avx512`：最后迁移，避免过早陷入高复杂度实现

原因：

- 先迁易验证、收益稳定的路径
- 先把 backend 架构和测试矩阵打稳
- 最复杂的路径留到后面，降低整体失败概率

## 5. 任务拆解

### 任务 1：抽象 SIMD 后端接口

要求：

- `galois_8` 不再直接耦合单一路径
- 上层只关心统一接口
- C 与 Rust 后端都可挂接到同一接口

### 任务 2：实现 runtime dispatch

要求：

- 只探测一次
- 后续使用缓存的 backend
- 提供调试输出或名称查询接口
- 能区分当前 backend 来自 C 还是 Rust

建议新增调试接口：

```rust
pub fn active_backend_name() -> &'static str
pub fn active_backend_kind() -> BackendKind
```

### 任务 3：补 GFNI 路径

这是本阶段最重要的性能专项之一。

目标：

- 在支持 GFNI 的 CPU 上显著优于 AVX2

### 任务 4：补 ARM64 路径治理

要求：

- 明确 NEON 路径是否已足够稳定
- 评估 SVE 的未来接入点

### 任务 5：细化 build.rs

改造方向：

- 避免默认强绑定单一 `-march`
- 构建期只负责生成可用后端
- 最优选择放到运行时

### 任务 6：Rust SIMD 后端第一批迁移

建议首批迁移：

- `mul_slice`
- `mul_slice_xor`

先从以下路径开始：

- `ssse3`
- `avx2`
- `neon`

这一步的目标不是一次性替掉全部 C，而是建立 Rust SIMD 后端开发模式。

### 任务 7：C/Rust 双实现对照测试

要求：

- 同一输入同时跑 scalar、C backend、Rust backend
- 结果做字节级一致性比较
- 固定 golden vector 与随机数据都覆盖

### 任务 8：退役策略设计

在文档中明确：

- 何时允许默认走 Rust backend
- 何时允许把 C backend 改为 fallback
- 何时允许完全移除 C backend

建议门槛：

1. Rust backend 在主流机器上不低于 C backend
2. 所有 ISA 路径有稳定一致性结果
3. benchmark 至少连续多轮没有明显退化

## 6. 验收标准

1. scalar fallback 正确
2. 每个 ISA 后端通过一致性测试
3. 运行时 dispatch 正确选路
4. GFNI 机器上显著优于 AVX2
5. C backend 与 Rust backend 至少在一组公共 ISA 上完成对照
6. Rust 新后端具备明确性能数据，不低于可接受门槛

## 7. 风险点

- 多 ISA 路径显著增加测试矩阵
- runtime dispatch 若处理不当，可能有 silent corruption 风险
- C/Rust 双实现并存期间维护成本会上升
- 过早迁移高复杂度 ISA 可能导致性能回退

## 8. 风险应对

- 引入 golden vector 和 cross-backend 对照测试
- 每个后端都用同一输入做 hash 对比
- 严格控制迁移顺序，先易后难
- 用 benchmark 数据决定是否推进某条 Rust 后端替换

## 9. 完成后的收益

- crate 可更适合通用二进制分发
- 后续 SIMD 优化能独立演进，不必反复触碰 core 逻辑
- 后续维护会逐步从“依赖外部 C 热路径”转向“Rust 内部统一治理”

## 10. 明确结论

本阶段的最终结论如下：

1. 继续使用现有 C SIMD，但只作为过渡期核心后端之一
2. 后续 SIMD 的长期目标是 Rust 主导实现
3. 迁移方式是“先并存、后对照、再替换”
4. 没有 benchmark 与一致性验证支撑时，不允许贸然移除 C backend

## 11. 当前落地状态（2026-05-25）

已完成：

- [x] 已有编译期开关 `simd-accel`，并保留纯 Rust 标量 fallback
- [x] SIMD-C 路径仍可用，当前与纯 Rust 路径共存（按编译条件）
- [x] 已引入统一 backend 抽象与 `name/kind` 观测接口
- [x] 已实现 runtime dispatch，一次探测并缓存 backend 结果
- [x] `std` / `no_std` 边界已打通：`std` 下可 runtime 探测，`no_std` 下保守回退 scalar
- [x] 已有首条 Rust SIMD backend pilot：
  - aarch64: `rust-neon` 已接通并完成与 scalar / simd-c 的一致性对照
  - x86_64: `rust-avx2` 已接通，并完成 `x86_64-apple-darwin` 目标下的对照测试编译与运行级一致性验证
- [x] benchmark smoke 已记录 runtime 实际 backend，并支持 `RSE_BACKEND_OVERRIDE`
- [x] 已新增 release 内核 benchmark：`galois_backend`

未完成 / 差距（本阶段核心）：

- [x] `rust-neon` 当前 correctness 已通过，并在 release 内核 benchmark / throughput bench 上显示出优于当前 `simd-c` 的趋势
- [ ] 仍需继续观察更广泛 case 与更稳定基准，以确认是否长期保持默认优先级
- [x] `rust-avx2` 已在 native `x86_64` 主机（`AMD EPYC 9V45`）完成 runtime benchmark 与 release smoke 复核，当前结论支持继续作为默认首选
- [x] GFNI 路径代码与 backend override 已引入，当前同时具备 `rust-gfni-avx2` 与 `rust-gfni-avx512`，并保持 override-only；剩余缺口是 native GFNI 主机上的性能验证与默认优先级结论
- [x] ARM64 路径的治理收口与 SVE 预留结构已完成，包括目录边界、feature-detect 插槽、override/metadata 验证与 profiling 契约
- [ ] ARM64 更深度的性能治理与可用 SVE backend 仍属后续实现任务

补充核查（2026-05-26，远程 `AMD EPYC 9V45` `x86_64` 主机）：

- [x] `benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45-96-core-processor.json` 与配套 `*.run-meta.json` 已齐备，远程实机结果和运行元数据都已归档。
- [x] machine json 中 `adoption_decision_stub.override_mismatch_count = 0`，说明同机 release smoke override 没有漂移。
- [x] machine json 中 `adoption_decision_stub.status = manual-review-required`，符合“推荐顺序与默认策略有差异时必须人工评审”的治理规则。
- [x] benchmark 推荐顺序高于当前 runtime policy 的候选 backend 仅作为文档结论记录；当前 `src/galois_8/backend.rs` 的 `x86_64` 自动选路顺序仍保持 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust`。
- [x] 因 benchmark 推荐与当前 runtime policy 存在差异，本轮不自动切换默认优先级，只完成远程实机验证收口与文档回填。

补充核查（2026-05-26，aarch64-apple-darwin 本机）：

- [x] `rust-neon` / `scalar` override 在 `RSE_STRICT_BACKEND_OVERRIDE=1` 下均可严格命中；
- [x] `test_active_backend_metadata` 与 `test_backend_override_affects_active_backend` 在 `std + simd-accel` 下通过；
- [x] `benchmark_smoke` 结果文件包含 backend 元数据字段（`backend/backend_id/backend_kind/backend_override/override_honored`）并与 override 行为一致。
- [x] 当前机器为 `arm64` Darwin，Rust host triple 为 `aarch64-apple-darwin`，现有核查结论与本机真实架构一致。

## 12. 执行待办（按优先级）

### P0（阶段核心闭环）

- [x] 定义 backend 抽象：
  - 建议文件：`src/galois_8/backend.rs`
  - 结构：`mul_slice` / `mul_slice_xor` / `name` / `kind`
- [x] 实现 runtime dispatch：
  - 首次探测 CPU feature 并缓存选择结果
  - x86_64：已支持 `scalar` / `simd-c` / `rust-avx2`
  - aarch64：已支持 `scalar` / `simd-c` / `rust-neon`
- [x] 暴露调试能力：
  - `active_backend_name()`
  - `active_backend_kind()`
- [x] 保证 `no_std` 与 `std` 的编译边界清晰（`no_std` 仍可用）

### P1（能力扩展）

- [x] 引入首批 Rust SIMD backend（当前已接入 `neon` pilot，`avx2` pilot 也已接线）
- [x] 建立 C backend 与 Rust backend 对照测试框架（固定长度 + 随机输入基础对照已具备）
- [x] 调整 `build.rs`，已显式暴露 SIMD-C 构建级别，runtime 可据此安全判定
- [x] 在 native AVX2 主机上补齐 `rust-avx2` benchmark，并与 `simd-c` 做同机对照
- [x] 已对 `rust-neon` 做进一步 profiling，当前结论更接近“debug smoke 低估性能，release bench 更能反映真实上限”
- [x] 已为 `encode/verify/reconstruct` 增加矩阵/调度层 profiling 观测（`RuntimeProfileStats` + `throughput_matrix` profile 导出）

### P2（高级优化）

- [x] GFNI 路径已完成实验性引入与本机正确性验证；后续仍需补 native GFNI 主机上的性能验证
- [x] ARM64 路径已补 SVE 预留扩展位（目录与 feature-detect 插槽）
- [x] ARM64 路径治理收口与 SVE 预留结构
- [ ] ARM64 深度性能治理与 SVE backend 实装/验证
- [x] Rust backend 成为默认路径前的退役门槛文档化（见 `docs/benchmark-methodology.md`）

## 13. 建议 PR 拆分

1. `phase4-backend-abstraction`: backend trait/struct + 调试接口 + 基础测试
2. `phase4-runtime-dispatch`: runtime feature 探测 + 分发缓存 + 回退路径
3. `phase4-rust-backend-pilot`: 首条 Rust SIMD backend + C/Rust 对照测试
4. `phase4-gfni-arm64`: GFNI/ARM64 强化与基准验证

## 14.1 Rust Backend 默认切换门槛

Rust backend 允许成为默认优先路径前，至少需要满足以下条件：

1. correctness 全通过：
   - scalar 对照
   - backend consistency sweep
   - smoke regression 不出现 correctness 漂移
2. benchmark 证据充分：
   - 同机、同 feature、同 backend override 口径下完成对照
   - `galois_backend` 与 workload 级 `smoke` / `throughput_matrix` 至少一条主线成立
3. 结果稳定：
   - 不能基于单次 noisy run 决策
   - 需要重复运行并以中位数结论为主
4. 回退路径清晰：
   - `scalar` fallback 保留
   - `simd-c` 或前序稳定 backend 仍可通过 override 强制回退
5. 治理同步：
   - baseline 更新理由已记录
   - 默认优先级变更已同步到相关文档与脚本

在未满足上述条件前，Rust backend 可以继续作为：

- correctness-validated backend
- benchmark candidate backend
- explicit override backend

但不应仅凭局部结果直接替换为默认优先路径。

## 14. 验收命令

```bash
cargo check --tests
cargo test --features std
cargo test --features "std simd-accel"
cargo bench --bench galois_backend --features "std simd-accel"
cargo bench --bench throughput_matrix
```

说明：

- `tests/benchmark_smoke.rs` 适合做功能 smoke 与结果导出，不适合单独作为默认 backend 性能裁决依据
- 默认 backend 的性能判断，优先参考 release 模式下的 `galois_backend` 与 `throughput_matrix`

## 15. 矩阵与调度层 Profiling（2026-05-25）

本轮新增：

- `ReedSolomon::runtime_profile_stats()` / `reset_runtime_profile_stats()`（`std`）
- `ReedSolomon::effective_parallel_policy()`（`std`），支持环境变量覆盖调度参数
- `RuntimeProfileStats` 包含：
  - `code_some_*`（串并行调用次数、字节量、chunk 数）
  - `parallel_policy_*`（policy 调用次数、串并行决策、jobs、chunk_len 聚合）
  - `reconstruct_*`（重建调用、data_only 调用、缺失 data/parity 聚合）
- `benches/throughput_matrix.rs` 支持 profile 导出：
  - `RSE_WRITE_PROFILE_REPORT=1`
  - `RSE_PROFILE_REPORT_PATH=/tmp/throughput-profile-*.json`

并行调度环境变量（`std`）：

- `RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES`
- `RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB`
- `RS_PARALLEL_POLICY_MAX_JOBS`
- `RS_RECONSTRUCT_DATA_MIN_PARALLEL_SHARD_BYTES`
- `RS_RECONSTRUCT_FULL_MIN_PARALLEL_SHARD_BYTES`

关键注意：

- 若 benchmark 走 `encode/verify/reconstruct`（非 `_opt`），不会经过 runtime 调度策略入口，`parallel_policy_*` 统计会接近 0。
- 若要分析“矩阵与调度层”行为，必须走 `encode_opt/verify_opt/reconstruct_opt/reconstruct_data_opt` 路径。

本轮（同机、release、同参数）观察结论：

- 在 `*_opt` 路径下，`10x4_1m` / `32x16_1m` 等中大 case 的 `parallel_policy_parallel` 与 `code_some_parallel_calls` 明显大于 0，说明调度策略生效。
- 同口径下 `rust-neon` 与 `simd-c` 都走到并行路径，但 `rust-neon` 的端到端吞吐仍显著更高，当前没有观察到“调度层导致 rust-neon 吃掉优势”的证据。
- reconstruct 场景中，decode matrix cache 命中持续接近满命中（除首次 miss），当前不是主要瓶颈。

## 16. 调度参数 Sweep 结论（2026-05-25）

在同机、同 benchmark（`throughput_matrix`，`*_opt` 路径，`rust-neon`）下做了以下参数扫描：

1. 默认（无覆盖）
2. `RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB=131072`
3. `RS_PARALLEL_POLICY_MAX_JOBS=4`
4. `RS_PARALLEL_POLICY_MAX_JOBS=8` + `RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB=131072`
5. `RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES=65536` + `RS_PARALLEL_POLICY_MAX_JOBS=8`

结论：

- 默认策略仍然是最稳妥配置；在 `10x4_1m` / `32x16_1m` 上表现稳定且整体最佳。
- 将 `min_bytes_per_job` 从 `256KiB` 降到 `128KiB`，会增加并行切片数量，但未带来稳定收益，部分 reconstruct 场景出现回退。
- 限制 `max_jobs=4` 整体无明显收益，`verify/reconstruct` 场景有回退风险。
- `max_jobs=8 + min_bytes_per_job=128KiB` 对 `verify_10x4_1m` 有局部提升，但整体收益不稳定。
- 将 `min_parallel_shard_bytes` 降到 `64KiB` 会让 `4x2_64k` 被强制并行，吞吐出现显著退化，应避免。

辅助产物：

- `/tmp/policy-default-rust-neon.json`
- `/tmp/policy-minbytes128k-rust-neon.json`
- `/tmp/policy-maxjobs4-rust-neon.json`
- `/tmp/policy-maxjobs8-minbytes128k-rust-neon.json`
- `/tmp/policy-minparallel64k-maxjobs8-rust-neon.json`

### 16.1 `simd-c` 同口径串行 Sweep（2026-05-25）

为避免并发 benchmark 互相干扰，本轮 `simd-c` 参数扫描按同机串行执行（每轮独立命令，固定 Criterion 参数）：

- `--sample-size 12 --warm-up-time 1 --measurement-time 2`
- backend 固定：`RSE_BACKEND_OVERRIDE=simd-c`

结果摘要（12 个 workload）：

- `default` 在 12 个 workload 中有 11 个为最佳吞吐。
- 唯一非 default 最优点：`reconstruct_10x4_1m` 在 `minparallel64k-maxjobs8` 下较 default 提升约 `+0.28%`，幅度接近噪声区间。
- `minbytes128k` 相对 default 平均 `-2.86%`，最差约 `-5.61%`。
- `maxjobs4` 相对 default 平均 `-3.22%`，最差约 `-6.16%`。
- `maxjobs8-minbytes128k` 相对 default 平均 `-4.11%`，最差约 `-8.54%`。
- `minparallel64k-maxjobs8` 相对 default 平均 `-14.67%`；在 `encode_4x2_64k`/`verify_4x2_64k` 上出现约 `-63%` 级别退化（小分片被过早强制并行）。

结论：

- `simd-c` 与前述 `rust-neon` sweep 趋势一致：default 仍是当前最稳妥的默认调度策略。
- 当前不建议把 `RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES` 下调到 `64KiB`。

辅助产物：

- profile JSON:
  - `/tmp/policy-default-simd-c-serial.json`
  - `/tmp/policy-minbytes128k-simd-c-serial.json`
  - `/tmp/policy-maxjobs4-simd-c-serial.json`
  - `/tmp/policy-maxjobs8-minbytes128k-simd-c-serial.json`
  - `/tmp/policy-minparallel64k-maxjobs8-simd-c-serial.json`
- bench 日志:
  - `/tmp/policy-default-simd-c-serial.log`
  - `/tmp/policy-minbytes128k-simd-c-serial.log`
  - `/tmp/policy-maxjobs4-simd-c-serial.log`
  - `/tmp/policy-maxjobs8-minbytes128k-simd-c-serial.log`
  - `/tmp/policy-minparallel64k-maxjobs8-simd-c-serial.log`

## 17. `rust-neon` 内核级 Profiling（2026-05-25）

本轮新增可观测性（`std`）：

- `galois_8::rust_neon_profile_stats()`
- `galois_8::reset_rust_neon_profile_stats()`
- `RustNeonProfileStats` 字段：
  - `mul_calls` / `mul_xor_calls`
  - `total_bytes`
  - `vector_64b_chunks` / `vector_16b_chunks`
  - `tail_bytes` / `tail_calls`
  - `table_lookups`（按 `vqtbl1q_u8` 次数聚合）

并已把这些字段接入 `benches/throughput_matrix.rs` 的 profile JSON 导出。

### 17.1 首轮观测结论

基于：

- `RSE_BACKEND_OVERRIDE=rust-neon`
- `cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
- profile: `/tmp/throughput-neon-kernel-profile.json`

观察：

- 各 operation/case 的 `neon_tail_bytes` 全为 `0`，`neon_vector_16b_chunks` 全为 `0`。
- `neon_table_lookups / neon_total_bytes` 稳定为 `0.125`（即每字节固定 `1/8` 次 lookup 聚合，符合当前 64B 向量路径的理论比例）。
- 说明当前 benchmark 主要命中 64B 向量主路径，几乎没有尾部 fallback；`rust-neon` 与 `simd-c` 的性能差异更可能来自主内核实现质量，而非尾部路径或调度参数。

### 17.2 直接结论（针对“`vqtbl1q_u8` 是否天然很难追上 C”）

- 当前证据不支持“慢在 tail/fallback”这一假设。
- 当前数据更接近：热点确实集中在 `vqtbl1q_u8` 主路径，若要继续逼近/超越 C，需要优化主路径的指令组织与数据装载，而不是继续调 `parallel policy`。

辅助产物：

- `/tmp/throughput-neon-kernel-profile.json`
- `/tmp/policy-default-rust-neon-lite.log`

## 18. `rust-neon mul_slice_xor` 微内核 A/B（2026-05-25）

本轮新增：

- `RS_NEON_MUL_SLICE_XOR_UNROLL=2|4`（`std` 下读取一次并缓存，默认 `4`）
- 在 `rust_neon_mul_slice_xor_impl` 中支持：
  - `unroll4`：原 64B 循环
  - `unroll2`：32B 循环（降低寄存器压力的对照组）

同机同参数（`galois_backend`, `sample-size 10`, `warm-up 1s`, `measurement 1s`, backend=`rust-neon`）结果：

- `len_65536`: `unroll2` vs `unroll4` = `50.815` vs `51.362` GiB/s（`-1.06%`）
- `len_1048576`: `48.796` vs `49.000` GiB/s（`-0.42%`）
- `len_4194304`: `47.733` vs `48.249` GiB/s（`-1.07%`）

结论：

- 当前实现下，`unroll2` 没有带来收益，三档长度均回退。
- 默认仍应保持 `unroll4`。
- 开关会保留，便于后续继续尝试更激进路径（例如不同 load/store 组合或指令重排）时快速 A/B。

辅助产物：

- `/tmp/neon-xor-unroll4.log`
- `/tmp/neon-xor-unroll2.log`
- `/tmp/neon-xor-ab.csv`

## 19. 参考 `reed-solomon-simd` 的实现对照（2026-05-25）

本轮对照了本机 registry 中 `reed-solomon-simd-3.1.0`（`engine/engine_neon.rs`）的 Neon 规则，重点观察：

- 以 64B 固定块为核心处理单元；
- 内部以 2x128-bit 子块组合计算（`mul_128` / `muladd_128`）；
- 主路径同样基于 `vqtbl1q_u8`，并尽量避免尾部路径进入热循环。

### 19.1 可迁移尝试

基于上面的规则，我们在 `rust_neon_mul_slice_xor_impl` 上做了对照实验：

- 将 64B 循环重排为“pair-wise 32B + 32B”顺序处理（降低单次寄存器活跃度）；
- 保持 `RS_NEON_MUL_SLICE_XOR_UNROLL=2|4` A/B 开关不变，便于同机复测。

### 19.2 结果与处理

同机同参数（`galois_backend`, `sample-size 10`, `warm-up 1s`, `measurement 1s`）下，相比原 `unroll4`：

- `len_65536`: `-1.99%`
- `len_1048576`: `-0.77%`
- `len_4194304`: `-0.54%`

结论：

- 这版“pair-wise 64B 重排”在当前代码结构和编译产物下为负收益；
- 已回退到原 `unroll4` 主路径，避免默认性能回退；
- 保留 `RS_NEON_MUL_SLICE_XOR_UNROLL` 作为后续 A/B 框架，后续更值得尝试的方向是：
  - 针对 `mul_slice_xor` 的 load/store 次序与 `vqtbl` 指令重排做更细粒度实验；
  - 在 `throughput_matrix` 的 `neon_*` profile 字段下联动验证端到端收益，而不仅看单点 microbench。

辅助产物：

- `/tmp/neon-xor-unroll4-pair.log`
- `/tmp/neon-xor-unroll2-pair.log`
- `/tmp/neon-xor-compare.csv`

## 20. 64B 对齐快速路径尝试（2026-05-25）

为了继续参考 `reed-solomon-simd` 的“固定块优先”思路，本轮额外尝试了：

- 在 `rust_neon_mul_slice` / `rust_neon_mul_slice_xor` 中为 `len % 64 == 0` 引入专用 fast-path（直接 64B 循环，跳过 16B/tail 分支）。

同机同参数（`galois_backend`, `sample-size 10`, `warm-up 1s`, `measurement 1s`）对照结果（`mul_slice_xor`）：

- `unroll4`:
  - `len_65536`: `-0.77%`
  - `len_1048576`: `-0.34%`
  - `len_4194304`: `-1.32%`
- `unroll2`:
  - `len_65536`: `-0.95%`
  - `len_1048576`: `-1.28%`
  - `len_4194304`: `-1.99%`

结论：

- 本轮 fast-path 在当前实现上没有带来收益，反而稳定小幅回退；
- 已回滚该 fast-path，避免默认路径性能回退；
- 后续优先继续在现有 `unroll4` 主循环内部做更细粒度指令级重排实验。

辅助产物：

- `/tmp/neon-xor-unroll4-fast64.log`
- `/tmp/neon-xor-unroll2-fast64.log`
- `/tmp/neon-fast64-compare.csv`

## 21. `mul_slice_xor` 指令调度 A/B（2026-05-25）

本轮新增了一个仅用于微调度对比的开关：

- `RS_NEON_MUL_SLICE_XOR_SCHEDULE=split`
- 默认（未设置）维持原有 `veorq(vqtbl(low), vqtbl(high))` 路径
- `split` 路径改为先批量做 `low_tbl`/`high_tbl` lookup，再做 xor 聚合

同机同参数（`galois_backend`, `sample-size 10`, `warm-up 1s`, `measurement 1s`, backend=`rust-neon`）对比：

- `len_65536`: `split` vs `base` = `50.331` vs `50.687` GiB/s（`-0.70%`）
- `len_1048576`: `48.709` vs `48.669` GiB/s（`+0.08%`）
- `len_4194304`: `47.974` vs `47.845` GiB/s（`+0.27%`）

结论：

- `split` 在大块上有轻微正向，但在 64K 回退，整体属于噪声级混合收益；
- 默认策略保持不变（不启用 `split`），以稳定性优先；
- 开关保留，便于后续与更细粒度重排策略组合验证。

辅助产物：

- `/tmp/neon-xor-sched-base2.log`
- `/tmp/neon-xor-sched-split.log`
- `/tmp/neon-sched-compare.csv`

## 22. `UNROLL × SCHEDULE` 小型 Sweep（2026-05-25）

本轮做了 4 组组合：

1. `base`
2. `schedule_split`
3. `unroll2`
4. `unroll2 + schedule_split`

同机同参数（`galois_backend`, `sample-size 10`, `warm-up 1s`, `measurement 1s`, backend=`rust-neon`）三档长度结果汇总（`mul_slice_xor`）：

- `base`: `50.505 / 48.015 / 47.429` GiB/s
- `schedule_split`: `49.991 / 48.020 / 46.513` GiB/s
- `unroll2`: `49.389 / 47.911 / 46.894` GiB/s
- `unroll2 + schedule_split`: `49.956 / 47.964 / 47.078` GiB/s

结论：

- 4 组里没有任何一组超过默认 `base`；
- `schedule_split` 和 `unroll2` 都是轻微回退；
- `unroll2 + schedule_split` 虽然比单独 `unroll2` 好一点，但仍未超过默认；
- 当前应保留默认 `base`，继续向更细粒度的指令级/寄存器级优化推进。

辅助产物：

- `/tmp/neon-sweep-base.log`
- `/tmp/neon-sweep-schedule_split.log`
- `/tmp/neon-sweep-unroll2.log`
- `/tmp/neon-sweep-unroll2_schedule_split.log`
- `/tmp/neon-sweep-matrix.csv`

## 23. 汇编审视与 `veor3` 试验（2026-05-25）

本轮先用 `cargo rustc --release --lib --features "std simd-accel" -- --emit=asm` 生成 release 汇编，定位到：

- `rust_neon_mul_slice_xor` 的热循环主体；
- 结果显示当前循环已经由 LLVM 合成出紧凑的 `tbl` / `eor3` 形态，没有明显的灾难性 spill；
- 因此直接把 `veor3` 作为源码级优化目标并不稳妥，优先级低于更细的 load/store 级调整。

尝试结果：

- `veor3` / `split` 组合没有形成稳定优势；
- 经过修正与回退后，默认路径性能保持正常；
- 这条路线不建议继续深挖，后续应转向更局部的寄存器活跃区间压缩与 load/store 互相错峰。

辅助产物：

- `target/release/deps/reed_solomon_erasure-ec9534e3728d9d83.s`

## 24. 压测卫生规则（2026-05-25）

本阶段后续所有 benchmark / 压测验证，建议统一先清空 `target/` 后再跑，以避免增量编译产物影响结论：

```bash
cargo clean
```

说明：

- 这样可以避免旧编译缓存、残留目标文件、或切换实验分支后的产物污染；
- 尤其适合我们现在这种“多开关 A/B、每次只看 1% 级差异”的场景；
- 后续跑基线或对照时，优先采用 clean-build 再压测。

## 25. clean-build 并排对比：`rust-neon` vs `simd-c`（2026-05-25）

为避免增量编译污染，本轮所有对照均采用：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=<backend> cargo bench --bench <bench> --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

并将完整日志保存到：

- `/tmp/galois-clean-rust-neon.log`
- `/tmp/galois-clean-simd-c.log`
- `/tmp/throughput-clean-rust-neon.log`
- `/tmp/throughput-clean-simd-c.log`

### 25.1 `galois_backend`（中位吞吐，GiB/s）

`mul_slice`：

- `len_65536`: `rust-neon=51.264` vs `simd-c=25.390`（`2.02x`）
- `len_1048576`: `rust-neon=51.010` vs `simd-c=25.117`（`2.03x`）
- `len_4194304`: `rust-neon=51.054` vs `simd-c=25.230`（`2.02x`）

`mul_slice_xor`：

- `len_65536`: `rust-neon=50.662` vs `simd-c=21.132`（`2.40x`）
- `len_1048576`: `rust-neon=48.636` vs `simd-c=21.081`（`2.31x`）
- `len_4194304`: `rust-neon=47.718` vs `simd-c=20.633`（`2.31x`）

### 25.2 `throughput_matrix`（中位吞吐，GiB/s）

`4x2_64k`：

- `encode`: `23.145` vs `10.608`（`2.18x`）
- `verify`: `17.014` vs `9.118`（`1.87x`）
- `reconstruct`: `14.109` vs `8.140`（`1.73x`）
- `reconstruct_data`: `14.215` vs `8.143`（`1.75x`）

`10x4_1m`：

- `encode`: `32.942` vs `16.472`（`2.00x`）
- `verify`: `24.048` vs `14.343`（`1.68x`）
- `reconstruct`: `15.197` vs `8.459`（`1.80x`）
- `reconstruct_data`: `20.774` vs `13.613`（`1.53x`）

`32x16_1m`：

- `encode`: `23.986` vs `12.187`（`1.97x`）
- `verify`: `16.062` vs `9.889`（`1.62x`）
- `reconstruct`: `14.278` vs `8.197`（`1.74x`）
- `reconstruct_data`: `20.827` vs `12.934`（`1.61x`）

### 25.3 结论（当前机器）

- `rust-neon` 在 `galois_backend` 与 `throughput_matrix` 两条线上均显著领先 `simd-c`。
- 这台 `aarch64` 机器上，`simd-c` 不适合继续作为默认性能路径候选。
- 下一步应继续深挖 `rust-neon` 主内核（`vqtbl1q_u8` 路径）与矩阵调度层，而不是继续投入 `simd-c` 的默认路径争夺。

## 26. `reconstruct` 分段 profile（2026-05-25）

本轮新增了 `RuntimeProfileStats` / `throughput_matrix` profile 导出字段：

- `reconstruct_data_stage_calls`
- `reconstruct_data_stage_bytes`
- `reconstruct_parity_stage_calls`
- `reconstruct_parity_stage_bytes`

用于把 `reconstruct` 热路径拆成两段看：

1. 补缺失 data shard
2. 在 data 完整后补缺失 parity shard

profile 基于：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
RSE_WRITE_PROFILE_REPORT=1 \
RSE_PROFILE_REPORT_PATH=/tmp/throughput-clean-rust-neon-profile-v2.json \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

### 26.1 观察结果

`reconstruct`（当前 smoke 缺失模式：丢 1 data + 1 parity）：

- `4x2_64k`: data/parity = `50% / 50%`
- `10x4_1m`: data/parity = `50% / 50%`
- `32x16_1m`: data/parity = `50% / 50%`

`reconstruct_data`：

- `4x2_64k`: data/parity = `100% / 0%`
- `10x4_1m`: data/parity = `100% / 0%`
- `32x16_1m`: data/parity = `100% / 0%`

### 26.2 直接结论

- 当前 benchmark 形态下，`reconstruct` 不是单独慢在 parity-stage，而是 data-stage / parity-stage 平分矩阵乘工作量。
- `reconstruct_data` 的全部热点都落在 data-stage。
- 因此下一步最优先的矩阵/调度层优化目标，应是 data-stage 的矩阵乘与调度策略；它会同时改善 `reconstruct` 和 `reconstruct_data`。

辅助产物：

- `/tmp/throughput-clean-rust-neon-profile-v2.json`
- `/tmp/throughput-clean-rust-neon-profile-v2.log`

## 27. data-stage 调度 sweep：`min_bytes_per_job`（2026-05-25）

为验证 data-stage 是否受更细切块策略影响，本轮在 clean-build 下对 `throughput_matrix` 做了两类 sweep：

1. 全局调 `RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB`
2. 仅对 reconstruct 路径调 `RS_RECONSTRUCT_MIN_BYTES_PER_JOB`

### 27.1 全局 `min_bytes_per_job`

对比基线（默认 `256 KiB`）：

- `RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB=128 KiB`
  - `reconstruct_10x4_1m`: `15.225 -> 15.564` GiB/s
  - `reconstruct_data_10x4_1m`: `20.830 -> 21.294` GiB/s
  - `reconstruct_32x16_1m`: `14.575 -> 14.184` GiB/s
  - `encode_32x16_1m`: `24.252 -> 23.286` GiB/s

- `RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB=64 KiB`
  - `reconstruct_10x4_1m`: `15.225 -> 15.927` GiB/s
  - `reconstruct_data_10x4_1m`: `20.830 -> 21.084` GiB/s
  - `reconstruct_32x16_1m`: `14.575 -> 14.831` GiB/s
  - `reconstruct_data_32x16_1m`: `20.995 -> 21.016` GiB/s
  - 但 `encode_32x16_1m`: `24.252 -> 21.438` GiB/s
  - 且 `verify_32x16_1m`: `16.296 -> 15.111` GiB/s

结论：

- 全局降到 `64 KiB` 确实能抬 `reconstruct` / `reconstruct_data`；
- 但会明显伤到大矩阵的 `encode` / `verify`；
- 因此不适合把更细 chunk 直接作为全局默认。

### 27.2 reconstruct 专用 `min_bytes_per_job`

新增环境覆盖：

```bash
RS_RECONSTRUCT_MIN_BYTES_PER_JOB=65536
```

目标是只影响 reconstruct 并行路径，不影响 encode / verify。

结果：

- `encode_32x16_1m`: `24.252 -> 23.193` GiB/s（仍回退）
- `verify_32x16_1m`: `16.296 -> 16.514` GiB/s（轻微波动）
- `reconstruct_10x4_1m`: `15.225 -> 15.381` GiB/s（小幅正向）
- `reconstruct_data_10x4_1m`: `20.830 -> 20.541` GiB/s（回退）
- `reconstruct_32x16_1m`: `14.575 -> 14.189` GiB/s（回退）
- `reconstruct_data_32x16_1m`: `20.995 -> 19.236` GiB/s（明显回退）

结论：

- 把 `64 KiB` 只下放到 reconstruct 路径，并没有稳定保住收益；
- 尤其 `reconstruct_data_32x16_1m` 明显回退，说明 data-stage 并不是“越细 chunk 越好”；
- 下一步不应继续围绕单一 `min_bytes_per_job` 常量做默认值调整，而应转向：
  - 按 `missing_data` / `missing_total` 分层决策；
  - 或直接优化 data-stage 的矩阵乘组织，而不是继续单参 sweep。

辅助产物：

- `/tmp/throughput-neon-minjob-128k.log`
- `/tmp/throughput-neon-minjob-64k.log`
- `/tmp/throughput-neon-reconstruct-minjob-64k.log`

## 28. `missing_data / missing_total` 分层决策实验（2026-05-25）

在完成单参数 `min_bytes_per_job` sweep 后，本轮继续尝试把 reconstruct 调度从“单一常量”升级到“按缺失模式分层决策”：

1. 先尝试较宽的启发式：
   - 让部分 small-data-set / mixed-missing 场景自动使用更细 chunk；
2. 再收紧为只命中：
   - `!data_only`
   - `missing_data == 1`
   - `missing_total <= 2`
   - 小数据分片规模

验证仍统一采用 clean-build：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

### 28.1 第一版分层启发式

产物：

- `/tmp/throughput-neon-tiered-reconstruct.log`

关键结果（对比默认基线）：

- `reconstruct_10x4_1m`: `15.225 -> 15.386` GiB/s
- `reconstruct_data_10x4_1m`: `20.830 -> 20.800` GiB/s
- `reconstruct_32x16_1m`: `14.575 -> 14.817` GiB/s
- `reconstruct_data_32x16_1m`: `20.995 -> 20.413` GiB/s

结论：

- 相比“全局 64 KiB”，这版分层启发式更稳一些；
- 但它仍然会明显伤到 `reconstruct_data_32x16_1m`；
- 因此还不适合直接默认启用。

### 28.2 第二版收紧启发式

将策略进一步收紧为：只在 small-data-set 的 full reconstruct mixed-missing 场景下调细 chunk。

产物：

- `/tmp/throughput-neon-tiered-reconstruct-v2.log`

关键结果（对比默认基线）：

- `reconstruct_10x4_1m`: `15.225 -> 14.671` GiB/s
- `reconstruct_data_10x4_1m`: `20.830 -> 20.767` GiB/s
- `reconstruct_32x16_1m`: `14.575 -> 15.207` GiB/s
- `reconstruct_data_32x16_1m`: `20.995 -> 21.888` GiB/s

结论：

- 第二版虽然改善了 `32x16` 的 `reconstruct / reconstruct_data`；
- 但却把 `10x4` 的 `reconstruct` 拉低了；
- 说明“仅靠一层简单 `missing_data / missing_total` 启发式”仍不足以形成稳定净收益。

### 28.3 最终处理

- 这两版分层启发式都已做过 clean-build 验证；
- 结论已记录，但没有保留为默认逻辑；
- 当前代码已回到稳定默认路径，只保留：
  - reconstruct 专用 env 覆盖能力；
  - reconstruct 分段 profiling 能力；
  - 后续继续做更细粒度实验所需的基线设施。

### 28.4 下一步建议

- 不再继续扩大默认启发式范围；
- 更值得推进的方向是：
  - 直接针对 `10x4` 与 `32x16` 分开建模；
  - 或下探 data-stage 的矩阵乘组织本身，而不是继续只调调度常量或粗粒度 heuristic。

### 28.5 回退到稳定默认后的 clean 校验

在撤回实验性默认启发式后，重新做了一次 stable clean-build 验证：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

产物：

- `/tmp/throughput-neon-post-revert-stable.log`

关键结果：

- `reconstruct_10x4_1m`: `15.245` GiB/s
- `reconstruct_data_10x4_1m`: `20.945` GiB/s
- `reconstruct_32x16_1m`: `14.771` GiB/s
- `reconstruct_data_32x16_1m`: `21.236` GiB/s

说明：

- `reconstruct / reconstruct_data` 已回到此前基线区间，没有因为撤回实验性启发式而出现新的结构性退化；
- `encode / verify` 会有一定 run-to-run 波动，后续若要做默认路径裁决，仍应以多轮 clean-build 对照为准，而不是单次结果。

## 29. 与 `x86_64` ISA 路径的冲突核实与拆分（2026-05-25）

### 29.1 核实结论

当前 `reconstruct` 调度逻辑位于 `src/galois_8.rs` 的高层 `ReedSolomon` 包装层，而不是 Neon / AVX2 内核本身。

这意味着：

- 它不会直接修改 `rust-neon` / `rust-avx2` / `simd-c` 的 SIMD 指令实现；
- 但如果把某个 reconstruct heuristic 直接默认化，它会同时影响 `aarch64` 与 `x86_64` 的上层调度行为；
- 因而从“默认策略”层面看，确实存在跨平台互相污染的风险。

### 29.2 本轮处理

为避免后续 `aarch64` 实验误伤 `x86_64`，已经先把 reconstruct policy 入口拆分为：

- `reconstruct_parallel_policy_default(...)`
- `reconstruct_parallel_policy_aarch64(...)`
- `reconstruct_parallel_policy(...)` 作为架构分发入口

当前默认行为保持不变：

- `aarch64` 先经过独立入口，但暂时仍回落到默认 policy；
- 非 `aarch64` 平台继续走默认 policy；
- 因此这次拆分本身不引入行为变化，只建立后续按架构继续优化的隔离边界。

### 29.3 验证

已跑的验证包括：

- `cargo test --features "std simd-accel" test_reconstruct_parallel_policy_respects_min_bytes_per_job_env`
- `cargo test --features "std simd-accel" test_reconstruct_parallel_policy_has_data_only_and_full_tiers`

另外还新增了一个仅在非 `aarch64` 下编译的保护测试：

- `test_reconstruct_parallel_policy_default_arch_stays_on_default_chunk`

说明：

- 在当前这台 `aarch64` 机器上，该测试会被 `cfg(not(target_arch = "aarch64"))` 过滤掉；
- 这正是预期行为，它的作用是保证将来在 `x86_64` 上不会被 `aarch64` 专用 heuristic 静默污染。

### 29.4 结论

- “先按平台和 ISA 拆分，再继续做定向优化”是完全可行的；
- 而且这一步已经先做了入口拆分；
- 后续若继续做 `aarch64/neon` 专用 reconstruct heuristic，可以只改 `reconstruct_parallel_policy_aarch64(...)`，不必再碰 `x86_64` 默认路径。

### 29.5 `aarch64` 专属 reconstruct policy 入口验证

在完成入口拆分后，本轮进一步把 `reconstruct_parallel_policy_aarch64(...)` 变成真正独立可调入口：

- 新增 `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB`
- 仅在 `target_arch = "aarch64"` 下生效
- 不影响 `x86_64` / 非 `aarch64` 的默认 reconstruct policy

已验证：

- `cargo test --features "std simd-accel" test_reconstruct_parallel_policy_respects_min_bytes_per_job_env`
- `cargo test --features "std simd-accel" test_aarch64_reconstruct_parallel_policy_has_arch_specific_override`

在当前 `aarch64` 机器上做的 clean-build bench：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB=131072 \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

产物：

- `/tmp/throughput-aarch64-reconstruct-128k.log`

关键结果（对比 stable 基线）：

- `reconstruct_10x4_1m`: `15.245 -> 14.968` GiB/s
- `reconstruct_data_10x4_1m`: `20.945 -> 20.200` GiB/s
- `reconstruct_32x16_1m`: `14.771 -> 14.383` GiB/s
- `reconstruct_data_32x16_1m`: `21.236 -> 19.428` GiB/s

结论：

- 这次 `aarch64` 专属 `128 KiB` override 没有形成稳定收益；
- 因此不应默认启用；
- 但这条专属入口本身是有价值的，因为后续继续做 `aarch64` reconstruct 调参时，已经可以完全和 `x86_64` 主线隔离。

### 29.6 `aarch64` 专属调参入口补齐

在初版 `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB` 之上，进一步补齐了 `aarch64` reconstruct policy 的三参数入口：

- `RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES`
- `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB`
- `RS_AARCH64_RECONSTRUCT_MAX_JOBS`

作用：

- 只在 `target_arch = "aarch64"` 下生效；
- 不影响 `x86_64` / 非 `aarch64` 默认 reconstruct policy；
- 后续可以只靠 env 做 `aarch64` 定向试验，不必反复改代码。

已验证：

- `cargo check --tests --benches --features "std simd-accel"`
- `cargo test --features "std simd-accel" test_reconstruct_parallel_policy_respects_min_bytes_per_job_env`
- `cargo test --features "std simd-accel" test_aarch64_reconstruct_parallel_policy_has_arch_specific_override`

说明：

- 当前还没有把这三参数中的任何一组固化成默认值；
- 但从这一刻起，`aarch64` reconstruct 调参与 `x86_64` 已经在“入口能力”层彻底解耦。

### 29.7 `aarch64` 专属三参数首轮 sweep

在入口能力补齐后，先做了两组 clean-build `aarch64` 定向实验：

1. `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB=65536`
2. `RS_AARCH64_RECONSTRUCT_MIN_PARALLEL_SHARD_BYTES=131072` +
   `RS_AARCH64_RECONSTRUCT_MIN_BYTES_PER_JOB=131072`

统一命令：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
... aarch64-only reconstruct env overrides ... \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

产物：

- `/tmp/throughput-aarch64-reconstruct-64k.log`
- `/tmp/throughput-aarch64-reconstruct-128k-threshold128k.log`

对比 stable 基线：

`aarch64-64k`

- `reconstruct_10x4_1m`: `15.245 -> 15.525`
- `reconstruct_data_10x4_1m`: `20.945 -> 20.751`
- `reconstruct_32x16_1m`: `14.771 -> 14.453`
- `reconstruct_data_32x16_1m`: `21.236 -> 20.699`

`aarch64-128k-threshold128k`

- `reconstruct_10x4_1m`: `15.245 -> 15.288`
- `reconstruct_data_10x4_1m`: `20.945 -> 20.951`
- `reconstruct_32x16_1m`: `14.771 -> 14.382`
- `reconstruct_data_32x16_1m`: `21.236 -> 21.288`

结论：

- `64 KiB` 细切块会小幅改善 `10x4` 的 `reconstruct`，但同时会伤到 `32x16`；
- `128 KiB + 128 KiB` 联动则整体更接近 stable 默认，没有形成明确超额收益；
- 说明在当前 smoke 矩阵下，还没有出现一个“明显优于 stable 默认”的 `aarch64` 单组参数。

下一步建议：

- 暂不把任何 `aarch64` 专属参数组固化成默认值；
- 更值得继续推进的是 data-stage 的实现组织优化，而不是继续扩大常量 sweep 规模。

### 29.8 data-stage / parity-stage 分离试验

为验证“是否可以只调 data-stage 而不牵连 parity-stage”，本轮进一步把 `aarch64` reconstruct policy 拆成了 stage-level 入口：

- `RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB`
- `RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB`

并补了对应测试：

- `test_aarch64_reconstruct_stage_policies_allow_data_parity_split`

首轮 clean-build 试验参数：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
RS_AARCH64_RECONSTRUCT_DATA_MIN_BYTES_PER_JOB=65536 \
RS_AARCH64_RECONSTRUCT_PARITY_MIN_BYTES_PER_JOB=262144 \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

产物：

- `/tmp/throughput-aarch64-stage-split.log`

对比 stable 基线：

- `reconstruct_10x4_1m`: `15.245 -> 14.827`
- `reconstruct_data_10x4_1m`: `20.945 -> 20.346`
- `reconstruct_32x16_1m`: `14.771 -> 14.110`
- `reconstruct_data_32x16_1m`: `21.236 -> 19.978`

结论：

- 仅把 data-stage 调细、parity-stage 保持默认，在当前 smoke 矩阵下仍然是负收益；
- 因此可以阶段性收束这条路线：
  - 不是 parity-stage 拖累了调度效果；
  - 而是当前 data-stage 的实现组织本身，并不适合继续靠 chunk 常量来逼收益。

直接结论：

- 继续扩大 reconstruct policy 常量 sweep 的性价比已经很低；
- 下一步应直接转向 `aarch64/neon` data-stage 的实现组织优化，而不是继续在 policy 层打转。

### 29.9 direct write-back 实现试验

在 policy 层基本收束后，本轮尝试了一个实现层思路：

- 将 `reconstruct_internal_option_vec_par_with_stage_policies(...)` 的 `Option<Vec<u8>>` 并行路径，从
  “先分配临时 `Vec<Vec<_>>`，计算完成后再写回 `shards`”
  改为
  “直接在 `shards` 内部 buffer 上写回”

目标：

- 减少 data-stage / parity-stage 的额外分配与拷贝；
- 看实现层减法是否比 policy 常量调参更有效。

验证：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

产物：

- `/tmp/throughput-direct-writeback.log`

对比 stable 基线：

- `reconstruct_10x4_1m`: `15.245 -> 14.824`
- `reconstruct_data_10x4_1m`: `20.945 -> 20.742`
- `reconstruct_32x16_1m`: `14.771 -> 14.703`
- `reconstruct_data_32x16_1m`: `21.236 -> 20.825`

结论：

- 这版 direct write-back 在当前实现下同样是负收益；
- 因此没有保留为默认实现，代码已回退到稳定版本；
- 说明当前瓶颈并不只是“临时 buffer 分配/拷贝”本身，而更可能还在 data-stage 的矩阵乘组织与访存形态。

## 31. 小输出 shard 的 chunk 内并行修正（2026-05-25）

在前面的 profile 与调度实验中，我们发现一个关键不一致：

- `ParallelPolicy` 会根据 `chunk_len` 和 `jobs` 计算出“应该有多少并行工作”；
- 但原先的 `code_some_slices_par_chunked(...)` 只按“输出 shard 数”做 `par_iter`；
- 对于 `reconstruct_data` / `reconstruct` 这类只缺 1-2 个 shard 的场景，哪怕 policy 计算出很多 `jobs`，实际运行时也吃不到，因为单个输出内部的 chunk 仍然是串行处理的。

### 31.1 修正方式

本轮对 `code_some_slices_par_chunked(...)` 做了一个很小但关键的实现修正：

- 当 `outputs.len() <= 2` 且 `chunk_count > 1` 时，
- 不再只按输出 shard 维度并行，
- 而是进入单个输出内部，对 `par_chunks_mut(chunk_len)` 做并行处理。

也就是说：

- 大输出 shard 数场景，仍保持原有“按输出并行”；
- 小输出 shard 数场景（尤其 reconstruct），改为“输出内 chunk 也并行”。

### 31.2 clean-build 结果

两轮 clean-build 复测结果：

首轮：

- `/tmp/throughput-small-output-chunk-par.log`

复测：

- `/tmp/throughput-small-output-chunk-par-repeat.log`

相对 stable 基线：

`run1`

- `reconstruct_10x4_1m`: `15.245 -> 18.610`
- `reconstruct_data_10x4_1m`: `20.945 -> 21.105`
- `reconstruct_32x16_1m`: `14.771 -> 24.109`
- `reconstruct_data_32x16_1m`: `21.236 -> 24.393`

`run2`

- `reconstruct_10x4_1m`: `15.245 -> 18.538`
- `reconstruct_data_10x4_1m`: `20.945 -> 21.016`
- `reconstruct_32x16_1m`: `14.771 -> 24.106`
- `reconstruct_data_32x16_1m`: `21.236 -> 24.076`

### 31.3 直接结论

- 这不是 policy 常量的噪声级收益，而是实现层真正把 `jobs` 用起来后的明显正收益；
- 说明之前的瓶颈之一，确实是“小输出 shard 场景下并行度没有落到单输出内部”；
- 这条修正比前面的 `min_bytes_per_job` sweep、stage-split 调参、direct write-back 都更接近问题本质。

### 31.4 下一步

- 这批实现应优先保留并继续验证；
- 后续可以继续补更细的 runtime 观测字段，确认“小输出 shard + chunk 内并行”在真实 reconstruct 场景下的命中频率；
- 但从当前两轮 clean-build 结果看，这已经是一个值得保留并继续推进的方向。

### 31.5 命中统计补齐

为确认这条优化不是“偶然变快”，本轮补了三项 runtime 观测字段：

- `code_some_small_output_chunk_parallel_calls`
- `code_some_small_output_chunk_parallel_outputs`
- `code_some_small_output_chunk_parallel_chunks`

并在 profile 导出里接入。

基于：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
RSE_WRITE_PROFILE_REPORT=1 \
RSE_PROFILE_REPORT_PATH=/tmp/throughput-small-output-chunk-par-profile-v2.json \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

产物：

- `/tmp/throughput-small-output-chunk-par-profile-v2.json`
- `/tmp/throughput-small-output-chunk-par-profile-v2.log`

关键命中数据：

- `reconstruct_10x4_1m`
  - `small_output_calls=7724`
  - `small_output_outputs=7724`
  - `small_output_chunks=30896`
  - `parallel_policy_jobs=30896`

- `reconstruct_data_10x4_1m`
  - `small_output_calls=6295`
  - `small_output_outputs=12590`
  - `small_output_chunks=25180`
  - `parallel_policy_jobs=50360`

- `reconstruct_32x16_1m`
  - `small_output_calls=3586`
  - `small_output_outputs=3586`
  - `small_output_chunks=14344`
  - `parallel_policy_jobs=14344`

- `reconstruct_data_32x16_1m`
  - `small_output_calls=1848`
  - `small_output_outputs=3696`
  - `small_output_chunks=7392`
  - `parallel_policy_jobs=14784`

说明：

- 对 `reconstruct` 而言，这条新路径的 `small_output_chunks` 与 `parallel_policy_jobs` 基本一一对应；
- 说明 policy 里之前“算出来但没被执行”的并行度，现在已经真正落到执行层；
- 对 `reconstruct_data` 来说，由于缺 2 个 data shard，`parallel_policy_jobs` 仍高于 `small_output_chunks`，这说明后续仍有继续榨取并行度的空间，但当前版本已经带来了稳定收益。

### 31.6 本轮收益复核

在 profile 版 clean-build 里，再次观测到：

- `reconstruct_10x4_1m`: `15.245 -> 18.226`
- `reconstruct_data_10x4_1m`: `20.945 -> 21.072`
- `reconstruct_32x16_1m`: `14.771 -> 23.074`
- `reconstruct_data_32x16_1m`: `21.236 -> 24.586`

结论：

- “小输出 shard + chunk 内并行”不是一次性的偶然结果；
- 它已经具备：实现修正、clean-build 收益、profile 命中率 三条证据；
- 这批改动现在已经具备单独提交的价值。

### 31.7 触发条件与行为说明

当前实现里，“小输出 shard + chunk 内并行”只在以下条件同时满足时触发：

- `outputs.len() <= 2`
- `chunk_count > 1`

也就是：

- 对 `encode` / `verify` 这种通常有更多输出 shard 的路径不生效；
- 对 `reconstruct` / `reconstruct_data` 这类常见的“只缺 1-2 个 shard”场景生效；
- 并且只有在 `chunk_len` 切分后真的有多个 chunk 时才进入该路径。

补充测试：

- `test_parallel_policy_creates_multiple_chunks_for_small_output_reconstruct_case`

这条测试锁住了一个典型场景：

- `shard_len = 1 MiB`
- `output_shards = 2`
- `available_parallelism = 8`

会得到：

- `jobs = 4`
- `chunk_len = 256 KiB`

说明这类 reconstruct 场景确实具备“输出内 chunk 并行”的前提条件，而不只是理论上的 policy 结果。

## 30. `galois_8` 目录结构第一步重构（2026-05-25）

本轮先按“只做结构迁移、不改行为”的原则，把 `galois_8` 从单文件大模块开始拆成目录模块：

- 原 `src/galois_8.rs` 已迁为 `src/galois_8/mod.rs`
- 新增：
  - `src/galois_8/aarch64/mod.rs`
  - `src/galois_8/aarch64/neon.rs`
  - `src/galois_8/x86/mod.rs`
  - `src/galois_8/x86/avx2.rs`
  - `src/galois_8/legacy/mod.rs`
  - `src/galois_8/legacy/simd_c.rs`

当前这一刀的目标不是一次性补齐 `ssse3 / avx512 / gfni`，而是先把现有实现按平台边界拆开，保证：

- 公共 API 继续集中在 `galois_8/mod.rs`
- backend 注册与 runtime 选路继续集中在 `galois_8/backend.rs`
- `aarch64/neon`、`x86/avx2`、`legacy/simd_c` 已从公共入口层分离

### 30.1 已验证

已通过：

- `cargo check --tests --benches --features "std simd-accel"`
- `cargo test --features "std simd-accel" test_active_backend_metadata`
- `cargo test --features "std simd-accel" test_backend_override_affects_active_backend`

说明：

- 这次目录重构在当前阶段没有破坏对外 `galois_8::*` API；
- runtime dispatch 与 backend override 在新结构下仍工作正常。

### 30.2 当前状态

- `aarch64` 逻辑已经可以在 `src/galois_8/aarch64/*` 下独立演进；
- `x86_64` 当前先拆出了 `avx2` 子模块；
- `simd_c` 已明确落到 `legacy/`；
- 后续仍可继续补齐：
  - `src/galois_8/x86/ssse3.rs`
  - `src/galois_8/x86/avx512.rs`
  - `src/galois_8/x86/gfni.rs`

### 30.3 直接结论

- 目录级拆分是可行的，并且已经完成第一步；
- 后续继续做 `aarch64` 定向调度/内核优化时，不再需要在同一个大文件里和 `x86_64` / `simd_c` 主线互相干扰。

### 30.4 `x86` 骨架补齐

在第一步目录迁移之后，继续补齐了 `x86` 子模块骨架：

- `src/galois_8/x86/ssse3.rs`
- `src/galois_8/x86/avx2.rs`
- `src/galois_8/x86/avx512.rs`
- `src/galois_8/x86/gfni.rs`

当前状态：

- `avx2.rs` 已承接现有 `rust-avx2` 实现；
- `ssse3.rs` / `avx512.rs` / `gfni.rs` 先作为 placeholder 模块存在；
- 这样后续继续补 ISA 时，不需要再回到 `mod.rs` 里做大规模剪切。

验证：

- `cargo check --tests --benches --features "std simd-accel"`
- `cargo test --features "std simd-accel" test_active_backend_metadata`

说明：

- 这一步仍然是“结构演进，不改行为”；
- 当前 runtime dispatch 仍保持原有 `scalar / simd-c / rust-neon / rust-avx2` 选路语义不变。

### 30.5 `mod.rs` 进一步瘦身

在补齐目录骨架后，继续完成了第二轮拆分：

- 新增：
  - `src/galois_8/profile.rs`
  - `src/galois_8/policy.rs`

拆分后职责更清晰：

- `src/galois_8/mod.rs`
  - 公共 API
  - GF(2^8) 标量表驱动逻辑
  - shared scalar baseline
  - 现有单元测试入口
- `src/galois_8/backend.rs`
  - backend 注册
  - runtime override
  - CPU feature 探测
  - runtime 选路
- `src/galois_8/profile.rs`
  - `RustNeonProfileStats`
  - Neon profiling metrics / env parsing
- `src/galois_8/policy.rs`
  - `encode_opt / verify_opt / reconstruct_opt / reconstruct_data_opt / reconstruct_some_opt`
  - reconstruct parallel policy helper

验证：

- `cargo check --tests --benches --features "std simd-accel"`
- `cargo test --features "std simd-accel" test_active_backend_metadata`
- `cargo test --features "std simd-accel" test_backend_override_affects_active_backend`

结果：

- `galois_8` 已不再是单文件混合承载 backend、ISA、profiling、policy、scalar 的“大杂烩”结构；
- 后续继续做 `aarch64/neon` 或 `x86/*` 定向优化时，改动边界会更清晰，也更不容易互相污染。

## 31. 小输出 shard 的 chunk 内并行修正（2026-05-25）

在前面的 profile 与调度实验中，我们发现一个关键不一致：

- `ParallelPolicy` 会根据 `chunk_len` 和 `jobs` 计算出“应该有多少并行工作”；
- 但原先的 `code_some_slices_par_chunked(...)` 只按“输出 shard 数”做 `par_iter`；
- 对于 `reconstruct_data` / `reconstruct` 这类只缺 1-2 个 shard 的场景，哪怕 policy 计算出很多 `jobs`，实际运行时也吃不到，因为单个输出内部的 chunk 仍然是串行处理的。

### 31.1 修正方式

对 `code_some_slices_par_chunked(...)` 做了一个很小但关键的实现修正：

- 当 `outputs.len() <= 2` 且 `chunk_count > 1` 时，
- 不再只按输出 shard 维度并行，
- 而是进入单个输出内部，对 `par_chunks_mut(chunk_len)` 做并行处理。

也就是说：

- 大输出 shard 数场景，仍保持原有“按输出并行”；
- 小输出 shard 数场景（尤其 reconstruct），改为“输出内 chunk 也并行”。

### 31.2 clean-build 结果

两轮 clean-build 复测结果：

首轮：

- `/tmp/throughput-small-output-chunk-par.log`

复测：

- `/tmp/throughput-small-output-chunk-par-repeat.log`

相对 stable 基线：

`run1`

- `reconstruct_10x4_1m`: `15.245 -> 18.610`
- `reconstruct_data_10x4_1m`: `20.945 -> 21.105`
- `reconstruct_32x16_1m`: `14.771 -> 24.109`
- `reconstruct_data_32x16_1m`: `21.236 -> 24.393`

`run2`

- `reconstruct_10x4_1m`: `15.245 -> 18.538`
- `reconstruct_data_10x4_1m`: `20.945 -> 21.016`
- `reconstruct_32x16_1m`: `14.771 -> 24.106`
- `reconstruct_data_32x16_1m`: `21.236 -> 24.076`

### 31.3 直接结论

- 这不是 policy 常量的噪声级收益，而是实现层真正把 `jobs` 用起来后的明显正收益；
- 说明之前的瓶颈之一，确实是“小输出 shard 场景下并行度没有落到单输出内部”；
- 这条修正比前面的 `min_bytes_per_job` sweep、stage-split 调参、direct write-back 都更接近问题本质。

### 31.4 命中统计补齐

为确认这条优化不是“偶然变快”，补了三项 runtime 观测字段：

- `code_some_small_output_chunk_parallel_calls`
- `code_some_small_output_chunk_parallel_outputs`
- `code_some_small_output_chunk_parallel_chunks`

并在 profile 导出里接入。

基于：

```bash
cargo clean
RSE_BACKEND_OVERRIDE=rust-neon \
RSE_WRITE_PROFILE_REPORT=1 \
RSE_PROFILE_REPORT_PATH=/tmp/throughput-small-output-chunk-par-profile-v2.json \
cargo bench --bench throughput_matrix --features "std simd-accel" -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

产物：

- `/tmp/throughput-small-output-chunk-par-profile-v2.json`
- `/tmp/throughput-small-output-chunk-par-profile-v2.log`

关键命中数据：

- `reconstruct_10x4_1m`
  - `small_output_calls=7724`
  - `small_output_outputs=7724`
  - `small_output_chunks=30896`
  - `parallel_policy_jobs=30896`

- `reconstruct_data_10x4_1m`
  - `small_output_calls=6295`
  - `small_output_outputs=12590`
  - `small_output_chunks=25180`
  - `parallel_policy_jobs=50360`

- `reconstruct_32x16_1m`
  - `small_output_calls=3586`
  - `small_output_outputs=3586`
  - `small_output_chunks=14344`
  - `parallel_policy_jobs=14344`

- `reconstruct_data_32x16_1m`
  - `small_output_calls=1848`
  - `small_output_outputs=3696`
  - `small_output_chunks=7392`
  - `parallel_policy_jobs=14784`

说明：

- 对 `reconstruct` 而言，这条新路径的 `small_output_chunks` 与 `parallel_policy_jobs` 基本一一对应；
- 说明 policy 里之前“算出来但没被执行”的并行度，现在已经真正落到执行层；
- 对 `reconstruct_data` 来说，由于缺 2 个 data shard，`parallel_policy_jobs` 仍高于 `small_output_chunks`，这说明后续仍有继续榨取并行度的空间，但当前版本已经带来了稳定收益。

### 31.5 触发条件与行为说明

当前实现里，“小输出 shard + chunk 内并行”只在以下条件同时满足时触发：

- `outputs.len() <= 2`
- `chunk_count > 1`

也就是：

- 对 `encode` / `verify` 这种通常有更多输出 shard 的路径不生效；
- 对 `reconstruct` / `reconstruct_data` 这类常见的“只缺 1-2 个 shard”场景生效；
- 并且只有在 `chunk_len` 切分后真的有多个 chunk 时才进入该路径。

补充测试：

- `test_parallel_policy_creates_multiple_chunks_for_small_output_reconstruct_case`

这条测试锁住了一个典型场景：

- `shard_len = 1 MiB`
- `output_shards = 2`
- `available_parallelism = 8`

会得到：

- `jobs = 4`
- `chunk_len = 256 KiB`

说明这类 reconstruct 场景确实具备“输出内 chunk 并行”的前提条件，而不只是理论上的 policy 结果。

### 31.6 结论

- “小输出 shard + chunk 内并行”不是一次性的偶然结果；
- 它已经具备：实现修正、clean-build 收益、profile 命中率 三条证据；
- 这批改动现在已经具备单独提交的价值。

### 31.7 `1-output / 2-output` 专用 fast path

在上一版“小输出 shard + chunk 内并行”基础上，又进一步做了一个更贴近 data-stage 的实现层优化：

- 当 `outputs.len() == 1` 时，走专用单输出 fast path；
- 当 `outputs.len() == 2` 时，走专用双输出 fast path；
- 在 `2-output` 场景下，同一段输入 chunk 由同一个任务顺手服务两个输出，减少重复的输入切片访问，并更好利用输入局部性。

相对上一版 `small-output-par`：

- `reconstruct_10x4_1m`: `18.538 -> 19.029`
- `reconstruct_data_10x4_1m`: `21.016 -> 25.144`
- `reconstruct_32x16_1m`: `24.106 -> 24.936`
- `reconstruct_data_32x16_1m`: `24.076 -> 26.911`

产物：

- `/tmp/throughput-small-output-fastpath.log`

结论：

- 这条 `1-output / 2-output` 专用 fast path 比泛化版“小输出并行”还要更强；
- 尤其 `reconstruct_data` 的提升最明显，说明 data-stage 在“少输出 shard”场景下确实非常受益于这种更专用的执行形态；
- 当前这批实现已经不仅是“可提交”，而是值得优先保留并继续在此基础上深挖的主方向。

### 31.8 `aarch64 + missing_data==2` 更窄专用路径试验

在 `1-output / 2-output` 专用 fast path 基础上，又尝试了一条更窄的实现：

- 仅在 `aarch64`
- 且 `active_backend_id() == BackendId::RustNeon`
- 且 `reconstruct_data_opt` 命中 `missing_data == 2 && missing == 2`

时，直接走一个更窄的双输出 data-stage 专用路径。

clean-build 结果相对上一版 `small-output-fastpath`：

- `reconstruct_10x4_1m`: `19.029 -> 18.396`
- `reconstruct_data_10x4_1m`: `25.144 -> 24.839`
- `reconstruct_32x16_1m`: `24.936 -> 23.849`
- `reconstruct_data_32x16_1m`: `26.911 -> 26.179`
- 且 `reconstruct_data_4x2_64k`: `14.040 -> 2.6318`

结论：

- 这条更窄的 `aarch64 + missing_data==2` 专用路径没有超过上一版通用 `1/2-output` fast path；
- 并且在小 case（`4x2_64k`）上出现了明显回退；
- 因此没有保留为默认实现，代码已回退到上一版 `1/2-output` fast path。

### 31.9 neon 双输出共享输入 helper 试验

进一步尝试过把“双输出共享输入加载”的专用 helper 直接下沉到 `aarch64/neon` 内核层，
并只在 `reconstruct_data_opt` 的 `missing_data == 2 && missing == 2` 场景下命中。

clean-build 结果相对上一版 `small-output-fastpath`：

- `reconstruct_10x4_1m`: `19.029 -> 18.394`
- `reconstruct_data_10x4_1m`: `25.144 -> 24.839`
- `reconstruct_32x16_1m`: `24.936 -> 23.910`
- `reconstruct_data_32x16_1m`: `26.911 -> 26.179`

结论：

- 直接把“双输出共享输入加载”下沉到当前 neon helper 形态，并没有超过上一版 `1/2-output` fast path；
- 因此这条试验性实现没有保留，代码已回退到更强、更通用的上一版实现；
- 说明下一步若要继续深挖 `aarch64/neon`，应更聚焦在 `vqtbl1q_u8` 主路径的 load/store 与寄存器组织，而不是仅仅把现有逻辑下沉一层。

### 31.10 真正下沉到 neon 双输出 helper 的再次验证

随后又尝试了一版更“正统”的下沉方案：

- 在 `src/galois_8/aarch64/neon.rs` 中新增
  - `rust_neon_mul_slice_two_outputs(...)`
  - `rust_neon_mul_slice_xor_two_outputs(...)`
- 并在 `reconstruct_data_opt` 的 `missing_data == 2 && missing == 2` 场景下直接调用它们

目标是：

- 不再只是高层路径专用分支；
- 而是让双输出共享输入加载真正发生在 neon helper 本身。

clean-build 结果相对上一版 `small-output-fastpath`：

- `reconstruct_10x4_1m`: `19.029 -> 17.711`
- `reconstruct_data_10x4_1m`: `25.144 -> 17.140`
- `reconstruct_32x16_1m`: `24.936 -> 24.017`
- `reconstruct_data_32x16_1m`: `26.911 -> 16.455`

结论：

- 这版“真正下沉到 neon helper”的实现比上一版通用 `1/2-output` fast path 明显更差；
- 因此同样没有保留，代码已回退；
- 进一步说明：当前最优结果并不是“越下沉越好”，而是“在 `core` 的 `1/2-output` fast path 形态下，让 policy 计算出的 chunk 并行度真正落地”。

### 31.11 `vqtbl1q_u8` 主循环表达式收紧试验

为进一步压缩寄存器活跃区间，本轮尝试把 `rust_neon_mul_slice_xor_impl` 的 `unroll4` 主循环从：

- 先生成 `product0..3`
- 再统一做 `veorq(outs, product)`

改成更紧的表达式组织：

- 先装载 `outs`
- 再直接在每个 lane 上构造 `veorq(outs.N, veorq(vqtbl(low), vqtbl(high)))`

目标是：

- 缩短 `product0..3` 的活跃区间；
- 看 LLVM 是否会生成更紧凑的寄存器分配与调度。

clean-build 结果相对当前最强的 `small-output-fastpath`：

- `reconstruct_10x4_1m`: `19.029 -> 18.695`
- `reconstruct_data_10x4_1m`: `25.144 -> 25.162`
- `reconstruct_32x16_1m`: `24.936 -> 24.893`
- `reconstruct_data_32x16_1m`: `26.911 -> 27.033`

结论：

- 这次主循环表达式收紧只表现为混合收益，没有形成足够稳定的正向结果；
- 因此没有保留，代码已回退到上一版更稳的实现；
- 说明当前 `vqtbl1q_u8` 主路径若继续深挖，更值得试的不是“单纯缩短表达式”，而是更成体系的 load/store 组织变化。
