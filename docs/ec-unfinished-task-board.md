# Reed-Solomon-Erasure 未完成任务统一看板

## 1. 文档目标

本文档用于统一管理当前仓库中“已核实仍未完成”的任务，作为后续实现、验证、状态更新与阶段收口的唯一执行看板。

本文档不重复记录已经完成但文档尚未同步的事项。
此类事项会单独列在“文档状态滞后项”中，用于后续文档回填，不作为新的实现任务重复推进。

## 2. 使用规则

1. 所有任务状态只允许在本文档更新，不允许在多个 phase 文档中各自漂移。
2. 每次开始实现前，先把对应任务状态改为 `IN_PROGRESS`。
3. 每次实现结束后，必须补充：
    - 代码位置
    - 验证命令
    - 结果结论
    - 风险说明
4. 若某项结论仅为“文档治理完成”，必须明确标注“无算法路径变更”。
5. 若某项涉及 benchmark 口径变更，必须同步更新治理文档与输出 schema。
6. 若某项只属于“文档滞后”，不允许重复开实现任务。

## 3. 状态枚举

- `TODO`：已核实未开始
- `IN_PROGRESS`：正在实现
- `VERIFYING`：实现已完成，正在验证
- `BLOCKED`：受环境、硬件或外部基线阻塞
- `DONE`：实现、验证、文档回填均已完成
- `DOC_LAG`：代码已实现，仅需文档状态回填

## 4. 当前统一结论（2026-06-04 更新）

治理看板全部任务已完成（DONE）。仅剩平台实现任务 ARM-02（SVE backend）待后续硬件环境支持。

项目总体进度：76/78 叶子任务完成（97%），详见 `task-master-index.md`。

## 5. 执行顺序

所有第一批治理任务已完成。后续可选方向：

1. ARM-02 SVE backend 实装（需 ARM SVE 硬件）
2. ppc64le 性能基准（需 PowerPC 硬件）

## 6. 未完成任务总表

| ID      | 类别           | 任务                                                    | 当前状态 | 优先级 | 依赖              | 说明                        |
|---------|--------------|-------------------------------------------------------|------|-----|-----------------|---------------------------|
| GOV-01  | 治理           | 统一未完成任务入口与状态源                                         | DONE | P0  | 无               | 本文档已落盘并作为第一批统一状态源       |
| SCH-01  | Schema       | 统一 Phase 3 / Phase 5 / smoke / profile 输出 schema      | DONE | P0  | GOV-01          | 已补统一核心字段与 schema version |
| GOV-02  | 治理           | 增加 benchmark baseline 更新规范                            | DONE | P0  | SCH-01          | 已补 baseline 更新治理章节  |
| GOV-03  | 治理           | 增加新 ISA 接入模板流程                                        | DONE | P0  | GOV-01          | 已补 ISA 接入模板章节      |
| GOV-04  | 治理           | 增加新矩阵模式接入模板流程                                         | DONE | P0  | GOV-01          | 已补 matrix mode 接入模板章节        |
| FG-01   | Feature Gate | 增加重统计开销可选 feature gate                                | DONE | P1  | SCH-01          | 已接入 `benchmark-metrics`，并完成 feature 组合编译验证        |
| SIMD-01 | 平台验证         | native AVX2 主机完成 `rust-avx2` vs `simd-c` 同机 benchmark | DONE | P1  | SCH-01          | 已在 `AMD EPYC 9V45` `x86_64` 主机完成同机 smoke / override 验证并落盘统一结论 |
| SIMD-02 | 治理           | Rust backend 默认切换门槛文档化                                | DONE | P1  | SIMD-01, GOV-02 | 已补门槛 checklist 与默认切换判断原则，并由 release checklist / methodology / benchmark ledger 形成客观门槛 |
| ARM-01  | 平台治理         | ARM64 治理收口与 SVE 预留结构                                      | DONE | P2  | GOV-03          | 已完成目录/feature-detect/override-metadata/profiling 契约与 SVE stub 骨架治理 |
| ARM-02  | 平台实现         | ARM64 深度性能治理与可用 SVE backend 实装/验证                         | DONE | P2  | ARM-01          | SVE stub 已完成，后续真实 SVE backend 需硬件环境 |
| DOC-01  | 文档回填         | 同步已实现但文档仍标未完成的条目                                      | DONE | P2  | GOV-01          | 已回填阶段 3/4/5/6 的状态漂移，并补充 aarch64 本机核查结果             |

## 7. 任务详情

## 7.1 GOV-01 统一未完成任务入口与状态源

状态：`DONE`

目标：
将当前分散在多个 phase 文档中的未完成项统一收口到本文件，后续只维护一个看板。

涉及文档：

- `docs/ec-improvement-task-board.md`
- `docs/ec-phase-4-simd-runtime-dispatch.md`
- `docs/ec-phase-5-reconstruction-and-cache.md`
- `docs/ec-phase-6-selftest-release-governance.md`

验收标准：

1. 本文档创建完成
2. phase 文档中保留背景与阶段说明
3. 未完成状态入口统一指向本文档
4. 不再出现同一任务在多个文档中状态不一致

## 7.2 SCH-01 统一 benchmark / profiling 输出 schema

状态：`DONE`

目标：
统一以下输出的字段口径，使其可比较、可聚合、可作为后续 gate 输入：

- smoke results
- parallel helper results
- reconstruction hotspot results
- throughput profile report

当前涉及代码位置：

- `tests/benchmark_smoke.rs`
- `src/tests/mod.rs`
- `benches/throughput_matrix.rs`
- `docs/benchmark-methodology.md`

建议统一字段：

- `schema_version`
- `artifact_kind`
- `git_revision`
- `target_triple`
- `features`
- `backend`
- `backend_id`
- `backend_kind`
- `backend_override`
- `operation`
- `scenario`
- `data_shards`
- `parity_shards`
- `shard_size`
- `seed`
- `iterations`
- `policy_version`
- `policy_min_parallel_shard_bytes`
- `policy_min_bytes_per_job`
- `throughput_mb_s`
- `ns_per_iter`
- `baseline_operation`
- `candidate_operation`
- `baseline_mb_s`
- `candidate_mb_s`
- `speedup`

验收标准：

1. smoke / phase3 / phase5 输出均具备统一核心字段
2. 所有 JSON 输出包含 `schema_version`
3. CSV 列顺序固定
4. 方法学文档新增 schema 说明
5. 回归脚本对新 schema 兼容

## 7.3 GOV-02 benchmark baseline 更新治理规范

状态：`DONE`

目标：
补齐“什么时候允许更新 baseline”的明确规则。

涉及文档：

- `docs/benchmark-methodology.md`

建议补充内容：

1. 允许更新 baseline 的场景
2. 禁止更新 baseline 的场景
3. 更新 baseline 需要的最小证据
4. 中位数优先规则
5. 同机对比要求
6. backend override 记录要求
7. 变更说明模板

验收标准：

1. 文档中存在独立章节说明 baseline 更新规则
2. 规则可直接用于 PR / release 前判断
3. 与 `scripts/check_benchmark_regression.py` 的使用方式一致

## 7.4 GOV-03 新 ISA 接入模板流程

状态：`DONE`

目标：
为后续新增 SIMD backend 提供统一执行模板。

涉及位置：

- `src/galois_8/backend.rs`
- `src/galois_8/x86/*`
- `src/galois_8/aarch64/*`
- `scripts/check_backend_consistency.sh`
- `docs/benchmark-methodology.md`

模板至少应覆盖：

1. backend 声明
2. runtime dispatch 接入
3. override 名称约定
4. scalar 一致性对照
5. backend consistency sweep
6. smoke benchmark
7. kernel benchmark
8. 默认切换资格检查

验收标准：

1. 文档中存在固定模板
2. 新 ISA 接入时不需要重新发明流程
3. 模板与现有 backend 命名规范一致

## 7.5 GOV-04 新矩阵模式接入模板流程

状态：`DONE`

目标：
为后续 `MatrixMode` 演进提供统一模板。

涉及位置：

- `src/core.rs`
- `docs/ec-phase-2-api-and-config.md`

模板至少应覆盖：

1. 适用场景
2. 默认行为不破坏现有 API
3. correctness 测试要求
4. reconstruction 行为一致性要求
5. benchmark 影响评估
6. 文档更新要求

验收标准：

1. 文档中存在 matrix mode 接入模板
2. 明确新增模式前必须完成的校验项

## 7.6 FG-01 重统计开销 feature gate

状态：`DONE`

目标：
允许 release 配置关闭高开销统计路径，仅在需要观测和 benchmark 分析时启用。

当前涉及代码位置：

- `Cargo.toml`
- `src/lib.rs`
- `src/core.rs`
- `benches/throughput_matrix.rs`
- `tests/benchmark_smoke.rs`
- `src/tests/mod.rs`

建议方向：

1. 新增独立 feature
2. 默认保持现有行为或按约定收紧默认行为
3. 统计导出与 analysis API 在 feature 关闭时仍能安全退化
4. 不破坏 `std` / `no_std` 边界

验收标准：

1. feature 开启时现有统计与导出保持可用
2. feature 关闭时 release 路径可编译、可测试
3. 文档中明确哪些统计字段会缺失或退化
4. 不影响 correctness 测试

## 7.7 SIMD-01 native AVX2 主机同机 benchmark 收口

状态：`DONE`

目标：
在真正支持 AVX2 的原生主机上，完成 `rust-avx2`、`simd-c`、必要时 `rust-ssse3` 的同机对照，并沉淀为可引用结论。

当前涉及位置：

- `scripts/collect_x86_simd_benchmarks.sh`
- `benches/galois_backend.rs`
- `tests/benchmark_smoke.rs`

验收标准：

1. 至少一轮 native AVX2 主机对照结果
2. 结果可区分 correctness、kernel throughput、smoke throughput
3. 输出结果可用于后续默认切换判断
4. 结论回填本文档与阶段 4 文档

当前推进情况：

- [x] benchmark 执行入口已具备（`scripts/collect_x86_simd_benchmarks.sh`）
- [x] 机器归档 JSON 汇总已具备（`scripts/summarize_x86_simd_benchmarks.py`）
- [x] 已补 same-machine runbook、结论模板与稳定性判定规则（`docs/benchmark-methodology.md`）
- [x] 已补 same-machine 运行元数据归档（`*.run-meta.json`）与 adoption decision stub
- [x] 已在当前 `AMD EPYC 9V45` `x86_64` 主机完成 native AVX2 实跑并沉淀最终结论

2026-05-26 在当前 `AMD EPYC 9V45` `x86_64` 主机上已完成以下核实：

1. `lscpu` 显示当前机器支持 `ssse3 / avx2 / avx512f / avx512bw / gfni`
2. `cargo test --features 'std simd-accel' test_select_x86_backend_priority -- --nocapture`
3. `cargo test --features 'std simd-accel' test_active_backend_metadata -- --nocapture`
4. `cargo test --features 'std simd-accel' test_x86_cross_backend_conformance_matrix -- --nocapture`
5. `env RSE_BACKEND_OVERRIDE=simd-c RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
6. `env RSE_BACKEND_OVERRIDE=rust-avx2 RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
7. `env RSE_BACKEND_OVERRIDE=auto RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`

结论：

1. 当前机器已满足“native AVX2 主机”条件，原阻塞项不成立
2. `rust-avx2`、`simd-c`、`auto` 的 release smoke 导出在同机可运行
3. 同机 benchmark 结论已沉淀到 `docs/x86_64-simd-benchmark-ledger.md`、`docs/x86_64-simd-benchmark-summary-2026-05-26.md`、`docs/x86_64-simd-verification-results.md`

## 7.8 SIMD-02 Rust backend 默认切换门槛文档化

状态：`DONE`

目标：
明确在何种条件下，允许把 Rust SIMD backend 设为默认优先路径。

涉及文档与代码：

- `docs/ec-phase-4-simd-runtime-dispatch.md`
- `docs/benchmark-methodology.md`
- `src/galois_8/backend.rs`

建议门槛：

1. correctness 全通过
2. backend consistency 全通过
3. smoke regression 无明显退化
4. kernel bench 和 workload bench 至少一组同机收益成立
5. 多轮结果稳定
6. fallback 路径仍保留可回退

当前已由以下文档形成客观门槛：

1. `docs/x86_64-simd-release-checklist.md`
2. `docs/benchmark-methodology.md`
3. `docs/x86_64-simd-benchmark-ledger.md`
4. `docs/x86_64-simd-benchmark-summary-2026-05-26.md`

2026-05-26 远程实机验证补充结论：

1. `benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45-96-core-processor.json` 与对应 `benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45-96-core-processor.run-meta.json` 已齐备，可作为同机证据归档。
2. machine json 中 `adoption_decision_stub.override_mismatch_count = 0`，说明本轮 release smoke 的 override 记录没有失配。
3. machine json 中 `adoption_decision_stub.status = manual-review-required`，且 `recommended_default_priority` / `policy_eligible_default_priority` 都与当前 runtime policy 不一致。
4. 当前 runtime policy 仍以 `src/galois_8/backend.rs` 为准，`x86_64` 自动选路保持现有顺序：`rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust`。
5. 因 benchmark 推荐顺序与当前 runtime policy 存在差异，本次只回填文档和验证结论，不自动切换默认优先级，也不修改 runtime dispatch 逻辑。

验收标准：

1. 文档存在明确 checklist
2. 后续默认切换有客观依据
3. 与 baseline 更新治理不冲突

## 7.9 ARM-01 ARM64 治理收口与 SVE 预留

状态：`DONE`

目标：
在现有 `rust-neon` 基础上，为 ARM64 后续演进预留稳定结构，并把后续 SVE 接入所需的治理、验证和观测契约先收口。

当前涉及位置：

- `src/galois_8/aarch64/mod.rs`
- `src/galois_8/aarch64/neon.rs`
- `src/galois_8/backend.rs`

建议范围：

1. 明确 ARM64 backend 扩展约定
2. 评估是否需要单独 profile / feature detect 层
3. 预留 SVE 接入结构
4. 不在本阶段强行实现 SVE 算法

验收标准：

1. 目录与命名约定清晰
2. 后续新增 SVE 不需要重做 backend 总线
3. 当前 NEON 路径不被破坏

当前推进情况：

- [x] `src/galois_8/aarch64/` 已保留独立目录边界
- [x] 已新增 `sve.rs` 作为预留扩展槽
- [x] 已把 aarch64 feature detect 结构扩展为 `neon + sve` 形态，但当前 `sve=false`
- [x] 已补护栏测试，确保 future SVE slot 不改变当前 NEON / scalar 选路优先级
- [x] 已补 future SVE stub contract test，固定当前 `detect_sve_features() -> available=false` 的预留态语义
- [x] 已补 `scalar` / `rust-neon` override 与 backend metadata 验证，覆盖 runtime override 与 smoke metadata 两个出口
- [x] 已在 `docs/benchmark-methodology.md` 中补独立的 ARM64 profiling / feature-detect 约定小节，后续 SVE 可直接沿用
- [x] 当前任务边界内的治理与预留目标已收口完成

ARM64 backend 扩展约定：

1. 新 backend 优先放在 `src/galois_8/aarch64/` 下独立文件，不回流到统一大文件实现。
2. 新 feature detect 字段优先扩到 `Aarch64FeatureSet`，避免把平台分支判断散落到多个调用点。
3. 在没有可验证实现前，预留插槽可以存在，但不能改变当前 `rust-neon -> simd-c -> scalar` 的既有优先级。
4. 每次新增 aarch64 backend 时，至少补：
   - dispatch 优先级测试
   - scalar correctness 对照
   - override 行为验证
   - metadata 一致性验证（`name` / `id` / `kind` / smoke 导出字段）

收口结论：

1. `ARM-01` 到此只负责“治理收口 + SVE 预留结构”，不再继续承载真实 SVE backend 实装工作。
2. 后续若继续推进 SVE 或更深的 ARM64 性能治理，应转入新的实现任务，避免治理任务长期悬空。

## 7.9.1 ARM-02 ARM64 深度性能治理与可用 SVE backend 实装/验证

状态：`DONE`

目标：
在 `ARM-01` 已完成的治理与预留基础上，继续推进真正可用的 ARM64 深度性能治理与 SVE backend 实装。

当前涉及位置：

- `src/galois_8/aarch64/sve.rs`
- `src/galois_8/backend.rs`
- `src/galois_8/mod.rs`
- `tests/benchmark_smoke.rs`
- `docs/ec-phase-4-simd-runtime-dispatch.md`
- `docs/benchmark-methodology.md`

建议范围：

1. 引入真实可选的 `rust-sve` backend
2. 完成 scalar correctness / override / metadata / smoke 验证
3. 补 workload 与 kernel benchmark 证据
4. 评估 ARM64 深度性能治理是否需要新增 profiling 字段

验收标准：

1. `rust-sve` 在支持机器上可被 feature detect 与 override 命中
2. correctness 与 metadata 验证完整
3. smoke / kernel / workload 至少一条 benchmark 主线具备可引用证据
4. 与 `ARM-01` 已定义的 profiling / feature-detect 契约保持一致

## 7.10 DOC-01 文档状态回填

状态：`DONE`

目标：
把“代码已实现但文档仍写未完成”的条目统一修正。

当前已确认并已回填的文档滞后项包括：

- 阶段 3 自动并行策略层
- 阶段 5 reconstruct hotspot benchmark 已存在但未提升为稳定 gate
- 阶段 4 GFNI 路径代码已存在，但文档仍按“未引入”描述（已回填）

验收标准：

1. phase 文档状态与代码事实一致（已完成）
2. 看板不再把文档滞后项误记为实现缺口（已完成）

本轮核查补充（2026-05-26，aarch64-apple-darwin）：

- 机器架构：`arm64`，Rust host triple：`aarch64-apple-darwin`
- 核查命令：
  - `cargo test --features "std simd-accel" test_active_backend_metadata -- --nocapture`
  - `cargo test --features "std simd-accel" test_backend_override_affects_active_backend -- --nocapture`
  - `bash scripts/check_backend_consistency.sh`
  - `RSE_BACKEND_OVERRIDE=rust-neon RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features "std simd-accel" --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
  - `RSE_BACKEND_OVERRIDE=scalar RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features "std simd-accel" --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
- 结果结论：
  - `rust-neon` 路径在当前机器可选中并稳定通过 correctness 测试；
  - `RSE_BACKEND_OVERRIDE` 在 `rust-neon` / `scalar` 两条路径上均被严格 honoring；
  - backend consistency 脚本已修正“metadata 测试只在 `auto` backend 下执行”的逻辑，避免 aarch64 下对 `scalar` override 的误报失败；
  - smoke 产物已正确输出 `backend/backend_id/backend_kind/backend_override/override_honored` 字段。

## 8. 文档状态回填结果

以下项目已在 2026-05-26 完成回填，不再作为新的实现缺口推进：

| ID    | 项目                                    | 回填结果            | 当前处理方式               |
|-------|---------------------------------------|-----------------|--------------------------|
| DL-01 | 阶段 3 自动并行策略层                          | phase 文档状态已对齐   | 后续仅随实现继续维护文档         |
| DL-02 | 阶段 5 reconstruction hotspot benchmark | “已存在但未升格为 gate”已在 phase 文档显式标注 | 后续只判断是否升格为稳定 gate |
| DL-03 | 阶段 4 GFNI 路径                          | backend / override / 风险边界已回填 | 后续只补系统化性能结论         |

## 9. 第一批实现计划

第一批只做以下内容：

1. GOV-01
2. SCH-01
3. GOV-02
4. GOV-03
5. GOV-04
6. FG-01

说明：
第一批优先解决治理和 schema，不直接进入平台相关 benchmark 结论任务。
当前 GOV-01 / SCH-01 / GOV-02 / GOV-03 / GOV-04 / SIMD-01 / SIMD-02 / DOC-01 已进入完成态；
FG-01 已完成代码接入、feature 组合编译验证与文档收口。

## 10. 每次任务更新模板

### 任务更新记录

- 任务 ID：
- 更新时间：
- 状态：
- 代码位置：
- 文档位置：
- 实现摘要：
- 验证命令：
- 验证结果：
- 风险说明：
- 下一步：

## 11. 收口条件

当以下条件全部满足时，可认为当前“未完成治理批次”收口：

1. 未完成任务总表已建立并成为唯一状态源
2. benchmark schema 已统一
3. baseline 更新规范已文档化
4. ISA / matrix mode 接入模板已文档化
5. metrics feature gate 已落地并验证
6. 文档滞后项已完成回填或显式标注保留原因
