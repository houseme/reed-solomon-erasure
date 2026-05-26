# x86_64 SIMD Runtime Dispatch 升级实战指南

## 实施状态

截至当前工作区状态，本指南对应的代码链路已完成首轮实现，且按阶段形成了以下实际提交：

1. `527a24e` `refactor(galois_8): isolate scalar baseline from simd backends`
2. `6eaa202` `refactor(dispatch): introduce backend ids and feature-driven selection`
3. `159d729` `refactor(x86): consolidate avx2 backend validation`
4. `be56c50` `feat(x86): add ssse3 mul_slice backends`
5. `68b188a` `refactor(simd_c): demote c backend to legacy fallback`
6. `1d9db55` `feat(x86): add avx512 backend for mul_slice paths`
7. `8d33660` `feat(x86): add experimental gfni backend`
8. `c9dd387` `test(simd): add cross-backend conformance matrix`

当前实现状态总结：

1. `x86_64` 已具备 `scalar / simd-c / rust-ssse3 / rust-avx2 / rust-avx512 / rust-gfni-avx2 / rust-gfni-avx512`
2. `GFNI` 当前为实验性 backend，且仅通过 override 接入
3. `simd_c` 已降级为 legacy fallback
4. `docs/` 文档未进入上述任何 commit

## 验证入口

建议与本文档配套查阅：

1. [x86_64 SIMD 验证结果与收官评审记录](./x86_64-simd-verification-results.md)
2. [x86_64 SIMD Runtime Dispatch 最终交付总结](./x86_64-simd-final-delivery-summary.md)
3. [x86_64 SIMD Runtime Dispatch 上线检查清单](./x86_64-simd-release-checklist.md)
4. [x86_64 SIMD Benchmark Summary (2026-05-26)](./x86_64-simd-benchmark-summary-2026-05-26.md)
5. [x86_64 SIMD GFNI Design Notes](./x86_64-simd-gfni-design.md)

该文档集中记录：

1. 已执行验证项
2. 未完成验证项
3. 收官评审结论
4. 默认策略建议
5. 后续建议动作

## 1. 文档目标

本文档用于指导 `reed-solomon-erasure` 在 `x86_64` 平台上完成 SIMD 指令集优化、运行时分发升级、平台拆分治理、测试与 benchmark 门禁建设。

本文档强调四个原则：

1. 先拆分平台与 ISA，再做深度优化。
2. 先保证正确性与可验证性，再追求极限性能。
3. 先把 `x86_64` 架构治理做完整，再推进 `GFNI` 等高复杂度路径。
4. 文档本身不参与 commit；代码子任务完成后再逐阶段提交 commit。

## 2. 当前代码状态核实

结合当前仓库代码，现状如下：

1. 仓库已经存在第一版 runtime dispatch，而非纯编译期开关。
2. `galois_8` 的公开入口已经通过 backend 层间接调用具体实现。
3. `scalar / legacy / x86 / aarch64` 已完成目录拆分，`x86_64` ISA 实现已分别落在独立模块中。
4. `build.rs` 当前对 `simd_c` 采用 baseline 构建，不再默认强绑 `-march=haswell`。
5. `x86_64` 当前已形成 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust` 的保守自动策略，`GFNI` 保持 override-only。
6. 现有测试已具备跨 backend 一致性矩阵，并已覆盖 `mul_slice / mul_slice_xor`，但多机型 benchmark 门禁仍需继续补齐。

## 3. 本次改造的核心目标

### 3.1 功能目标

1. 在 `x86_64` 上建立可扩展的多 ISA runtime dispatch 体系。
2. 保持 `scalar fallback` 永远可用。
3. 保持 `aarch64` NEON 路径不受 `x86_64` 扩展扰动。
4. 将 `simd_c` 明确降级为 `legacy fallback` 或过渡后端。
5. 为未来 `GFNI`、`AVX512`、更多 Rust intrinsic backend 预留稳定扩展点。

### 3.2 工程目标

1. 按平台拆分代码目录，避免 `x86_64` 与 `aarch64` 逻辑继续缠绕。
2. 按 ISA 拆分后端文件，降低复杂度与 review 风险。
3. 建立 backend 元数据模型，使选路逻辑、调试输出、测试覆盖一致。
4. 建立逐阶段验收标准、回退策略、benchmark 基线与提交策略。

### 3.3 非目标

1. 本轮不一次性重写所有 `simd_c` 路径。
2. 本轮不将 `aarch64` 扩展到 SVE。
3. 本轮不承诺 `GFNI` 一定成为默认最优路径，是否启用以正确性与数据为准。

## 4. 平台冲突分析与处理结论

### 4.1 是否会与 `aarch64` 代码直接冲突

不会直接发生指令级冲突，只要继续满足以下条件：

1. `#[cfg(target_arch = "x86_64")]` 与 `#[cfg(target_arch = "aarch64")]` 严格隔离。
2. `#[target_feature]` 只出现在对应架构文件中。
3. backend 注册与选择只暴露平台无关的统一接口。

### 4.2 真正的风险点

风险不在 CPU 指令冲突，而在工程组织冲突：

1. 当前不同平台 SIMD 代码混放，后续新增 ISA 时 selector 会越来越乱。
2. `x86_64` 的测试改动可能意外破坏 `aarch64` 的逻辑分支。
3. 单文件内混合 `scalar`、`aarch64`、`x86_64`、`simd_c` 会导致 review 与回归定位成本很高。

### 4.3 处理结论

先拆平台与 ISA，是本轮第一优先级，且完全可行。

## 5. 推荐目录结构

推荐改造后的结构如下：

```text
src/
  galois_8/
    mod.rs
    backend.rs
    scalar.rs
    legacy/
      mod.rs
      simd_c.rs
    x86/
      mod.rs
      ssse3.rs
      avx2.rs
      avx512.rs
      gfni.rs
    aarch64/
      mod.rs
      neon.rs
```

### 5.1 结构职责划分

1. `mod.rs`
   负责公共 API、公共表、模块导出、少量平台无关 glue code。
2. `backend.rs`
   负责 backend 元数据、CPU feature 探测、runtime dispatch、override 逻辑。
3. `scalar.rs`
   负责纯 Rust 标量基线实现。
4. `legacy/simd_c.rs`
   负责 `simd_c` FFI 包装与 fallback 逻辑。
5. `x86/*`
   每个文件只承载一个 ISA 实现。
6. `aarch64/neon.rs`
   只保留 ARM64 NEON 路径。

## 6. backend 架构设计

### 6.1 backend 元数据模型

建议将 backend 定义细化为：

```rust
pub type MulSliceFn = fn(u8, &[u8], &mut [u8]);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BackendImplKind {
    Scalar,
    SimdC,
    RustSimd,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BackendId {
    ScalarRust,
    SimdCSse2,
    RustSsse3,
    RustAvx2,
    RustAvx512,
    RustGfniAvx2,
    RustGfniAvx512,
    RustNeon,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct X86FeatureSet {
    pub sse2: bool,
    pub ssse3: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub avx512bw: bool,
    pub gfni: bool,
}

#[derive(Copy, Clone)]
pub struct GaloisBackend {
    pub id: BackendId,
    pub name: &'static str,
    pub kind: BackendImplKind,
    pub mul_slice: MulSliceFn,
    pub mul_slice_xor: MulSliceFn,
}
```

### 6.2 设计要求

1. `id` 用于测试与稳定标识，不依赖文案字符串。
2. `name` 用于调试输出、benchmark 标签、override 值。
3. `kind` 用于区分 `Scalar / SimdC / RustSimd`。
4. `mul_slice` 与 `mul_slice_xor` 继续保持统一函数签名，避免影响上层 core 编码逻辑。

### 6.3 调试接口

建议对外保留或扩展以下接口：

```rust
pub fn active_backend_name() -> &'static str
pub fn active_backend_kind() -> BackendImplKind
pub fn active_backend_id() -> BackendId
pub fn active_backend_debug_summary() -> &'static str
```

## 7. x86_64 runtime dispatch 设计

### 7.1 目标

运行时只探测一次，缓存特性与最终后端；选路清晰、可解释、可 override、可测试。

### 7.2 推荐优先级

当前代码中实际采用的 `x86_64` 自动选择优先级如下：

1. `rust-avx2`
2. `rust-avx512`
3. `rust-ssse3`
4. `simd-c`
5. `scalar-rust`

补充说明：

1. `rust-gfni-avx2`
2. `rust-gfni-avx512`

两条 `GFNI` 路径当前只作为实验性 override，不参与自动优先级。

### 7.3 为什么采用此优先级

1. `AVX2` 是当前最稳、且已有单机实测支持的现代 `x86_64` 主线。
2. `AVX512` 带来 64B 宽度优势，但不同 CPU 上仍存在频率降档与收益不稳定风险。
3. `SSSE3` 用于补齐老机器中间档。
4. `simd_c` 保留为 legacy fallback，而非主线。
5. `GFNI` 理论上有潜力，但在跨机器收益与更正式设计材料补齐前仍保持实验状态。

### 7.4 特性探测建议

探测逻辑建议集中在 `backend.rs`：

```rust
#[cfg(target_arch = "x86_64")]
fn detect_x86_features() -> X86FeatureSet {
    X86FeatureSet {
        sse2: std::is_x86_feature_detected!("sse2"),
        ssse3: std::is_x86_feature_detected!("ssse3"),
        avx2: std::is_x86_feature_detected!("avx2"),
        avx512f: std::is_x86_feature_detected!("avx512f"),
        avx512bw: std::is_x86_feature_detected!("avx512bw"),
        gfni: std::is_x86_feature_detected!("gfni"),
    }
}
```

### 7.5 override 机制要求

环境变量 `RSE_BACKEND_OVERRIDE` 继续保留，但值域扩展为：

1. `auto`
2. `scalar`
3. `simd-c`
4. `rust-ssse3`
5. `rust-avx2`
6. `rust-avx512`
7. `rust-gfni-avx2`
8. `rust-gfni-avx512`
9. `rust-neon`

override 规则：

1. 指定 backend 不满足 CPU 特性时，不应静默崩溃。
2. 建议返回 `None` 后回退到自动选择，或者在 debug/test 模式显式报错。
3. 测试需要覆盖所有 override 入口。

## 8. `simd_c` 的重新定位

### 8.1 当前问题

`simd_c` 当前仍受 `build.rs` 中单一 `-march` 决策影响，不适合作为未来多 ISA 体系主路径。

### 8.2 推荐策略

1. 保留 `simd_c` 作为 legacy backend。
2. `x86_64` 上默认仅把它视为 `SSE2/兼容 fallback`。
3. 不再依赖 `simd_c` 代表最优后端。
4. Rust intrinsic backend 作为未来主实现。

### 8.3 build.rs 改造方向

1. 避免默认强绑定 `-march=haswell` 作为整体最优策略。
2. 明确区分：
   - baseline fallback build
   - optional legacy tuned build
3. 让 “是否生成某个 backend” 与 “运行时最终选哪个 backend” 完全解耦。

## 9. 各 ISA 实现策略

### 9.1 scalar

要求：

1. 继续作为所有 backend 的绝对正确性基线。
2. 继续承担 SIMD 处理尾部数据的 fallback。
3. 不在本轮引入激进改写，只允许做文件拆分与轻量整理。

### 9.2 SSSE3

实现策略：

1. 使用 nibble-table + `pshufb`。
2. 每次处理 16B。
3. 结构与 AVX2 保持一致，方便代码 review 与 correctness diff。

验收门槛：

1. 对 scalar 完全一致。
2. 老平台上明显优于 scalar。

### 9.3 AVX2

实现策略：

1. 沿用当前已有 nibble-table + `vpshufb` 路线。
2. 先做模块化迁移，不立即做激进微优化。
3. 后续若有收益，再评估：
   - 循环展开
   - 预取
   - 别名消除
   - load/store 调度改善

验收门槛：

1. 性能不低于当前主线。
2. 正确性与现有实现完全一致。

### 9.4 AVX512

实现策略：

1. 优先做 `avx512f + avx512bw` 路径。
2. 同样采用 table-shuffle 模型，先求稳定落地。
3. 以 64B 块为基本处理单元。

风险：

1. 某些 CPU 上可能因频率降档导致收益不稳定。
2. 不能只看单一机器上的吞吐，需要至少两类 CPU 数据验证。

启用策略：

1. 仅在 benchmark 明显优于 AVX2 时升为自动优先。
2. 若收益不稳定，可先保留为 override-only backend。

### 9.5 GFNI

这是本轮最高风险专项，不应最先实施。

关键事实：

1. 当前库的 GF(2^8) 生成多项式不是 AES 常见表示。
2. GFNI 指令的乘法语义与当前域表示未必直接等价。
3. 若直接替换乘法，存在 silent corruption 风险。

因此必须采用以下路线：

1. 先做域表示与 basis conversion 设计文档。
2. 给出从当前域到 GFNI 工作域的输入/常量/输出变换公式。
3. 先做 correctness prototype。
4. 通过 cross-backend tests 后再引入 benchmark。

启用条件：

1. 有明确数学证明或高可信实现对照。
2. 全量一致性测试通过。
3. benchmark 在 GFNI 机器上优于 AVX2。

## 10. 测试体系建设

### 10.1 测试层次

必须建立四层测试：

1. backend 单元一致性测试
2. cross-backend 对照测试
3. 编码链路集成测试
4. benchmark smoke 测试

### 10.2 backend 单元一致性测试

每个 backend 至少测试：

1. `mul_slice`
2. `mul_slice_xor`
3. 长度边界
4. 尾部处理
5. 非对齐输入

长度集合建议：

1. `0`
2. `1`
3. `15`
4. `16`
5. `17`
6. `31`
7. `32`
8. `33`
9. `63`
10. `64`
11. `65`
12. `255`
13. `256`
14. `257`
15. `4096`
16. `65536`

### 10.3 输入模式建议

每组长度至少覆盖：

1. 全 0
2. 全 `0xff`
3. 递增序列
4. 固定重复模式
5. 伪随机
6. 非对齐 slice 视图

### 10.4 系数集合建议

至少覆盖：

1. `0`
2. `1`
3. `2`
4. `15`
5. `16`
6. `31`
7. `127`
8. `173`
9. `255`
10. 若干随机值

### 10.5 cross-backend 对照

要求同一组输入同时跑：

1. scalar
2. simd-c
3. rust-ssse3
4. rust-avx2
5. rust-avx512
6. rust-gfni-avx2
7. rust-gfni-avx512
8. rust-neon

实际执行时按平台裁剪不可用 backend，但测试框架要支持统一表达。

### 10.6 集成测试

必须覆盖：

1. `encode`
2. `verify`
3. `reconstruct`
4. `reconstruct_data`

并在不同 backend 下复用同一套 golden vectors。

## 11. Benchmark 体系与验收标准

### 11.1 benchmark 目标

benchmark 不只是展示结果，而是作为 backend 升级/降级的门禁依据。

### 11.2 推荐维度

至少按以下维度测量：

1. `mul_slice`
2. `mul_slice_xor`
3. `encode`
4. `reconstruct`
5. `verify`

### 11.3 数据规模

建议至少使用：

1. `64 KiB`
2. `1 MiB`
3. `4 MiB`
4. `16 MiB`

### 11.4 benchmark 输出要求

每次 smoke 或 criterion 跑完，应记录：

1. git revision
2. target triple
3. enabled features
4. active backend
5. backend override
6. input size
7. throughput
8. ns/op
9. CPU model
10. 是否开启 turbo / governor 说明

### 11.5 性能门槛建议

1. `rust-avx2` 不得低于当前主线超过可接受波动范围。
2. `rust-ssse3` 必须优于 scalar。
3. `rust-avx512` 只有在大部分测试数据点优于 `rust-avx2` 时才默认启用。
4. `rust-gfni-*` 只有在 correctness 与性能都满足时才进入自动优先级。

## 12. 提交策略

### 12.1 总原则

1. 文档全部保存到 `docs/`，但不进入 commit。
2. 代码改动按子任务完成后逐阶段 commit。
3. 每个 commit 只做一件完整、可验证的事。

### 12.2 commit 粒度建议

建议按以下粒度提交：

1. 平台/目录拆分
2. backend 元数据与 selector 重构
3. `x86_64` AVX2 迁移稳定
4. `x86_64` SSSE3 新增
5. `simd_c` legacy 治理
6. `AVX512` backend 新增
7. `GFNI` prototype 或正式 backend
8. 测试矩阵补齐
9. benchmark 门禁与文档更新

### 12.3 推荐 commit message 模式

```text
refactor(galois_8): split simd backends by platform
refactor(dispatch): introduce backend ids and feature-driven selection
feat(x86): add ssse3 mul_slice backends
refactor(simd_c): demote c backend to legacy fallback
feat(x86): add avx512 backend for mul_slice paths
feat(x86): add experimental gfni backend
test(simd): add cross-backend conformance matrix
bench(simd): add backend-gated performance smoke checks
```

## 13. 风险清单与应对

### 13.1 silent corruption

风险最高，尤其在 `GFNI` 与尾部处理场景。

应对：

1. 所有 backend 必须对 scalar 做字节级对照。
2. 每个 ISA 新增前先落单元测试。
3. `GFNI` 必须先有数学正确性说明。

### 13.2 dispatch 误选路

风险：

1. CPU feature 探测写错。
2. override 与自动分发行为不一致。
3. 测试预期落后于 selector 真实实现。

应对：

1. selector 单独单元测试。
2. feature set 与 backend requirement 明确编码。
3. 增加 `active_backend_id()` 稳定断言。

### 13.3 aarch64 回归

应对：

1. 先拆目录再改逻辑。
2. `aarch64/neon.rs` 改造阶段禁止混入 `x86_64` ISA 新功能。
3. 保持 `aarch64` selector 与实现尽量小改。

### 13.4 benchmark 结论失真

应对：

1. 记录 CPU 信息与运行参数。
2. 不用单次数据做默认路径决策。
3. 至少做重复测量与多输入规模比较。

## 14. 阶段划分总览

建议划分为八个子任务阶段：

1. 阶段 0：平台与 ISA 拆分、行为冻结、风险隔离
2. 阶段 1：backend 元数据模型与 runtime dispatch 重构
3. 阶段 2：`x86_64` AVX2 模块化迁移与稳定化
4. 阶段 3：`x86_64` SSSE3 backend 新增
5. 阶段 4：`simd_c` legacy fallback 治理与 `build.rs` 修正
6. 阶段 5：`x86_64` AVX512 backend 新增与门禁验证
7. 阶段 6：`x86_64` GFNI backend 设计、验证与实验集成
8. 阶段 7：cross-backend tests、benchmark 门禁、文档收尾

每个阶段的详细说明见独立子任务文档。

## 15. 执行顺序建议

严格建议按以下顺序实施，不建议跳步：

1. 先拆结构，冻结现有行为。
2. 再重构 selector。
3. 再迁现有 AVX2。
4. 再补 SSSE3。
5. 再治理 `simd_c`。
6. 再补 AVX512。
7. 最后做 GFNI。
8. 用统一测试和 benchmark 门禁收口。

## 16. 最终验收标准

### 16.1 功能验收

1. `scalar fallback` 正确且始终可用。
2. 各 ISA backend 与 scalar 完全一致。
3. `encode` / `verify` / `reconstruct` / `reconstruct_data` 全链路通过。

### 16.2 架构验收

1. `x86_64` 与 `aarch64` 代码路径完成平台拆分。
2. backend 选择逻辑可解释、可 override、可测试。
3. `simd_c` 不再承担默认最优主路径角色。

### 16.3 性能验收

1. `rust-avx2` 不回退。
2. `rust-ssse3` 对老平台有正收益。
3. `rust-avx512` 在适用机器上有明确收益。
4. `rust-gfni-*` 只有在正确且收益明确时才启用。

## 17. 文档与执行要求

1. 本文档与子任务文档全部保存在项目根目录 `docs/` 下。
2. `docs/` 文档只作为执行蓝图，不进入 commit。
3. 每完成一个代码子任务，必须先跑对应测试，再单独 commit。
4. 若某阶段发现与预期冲突，应在进入下一阶段前更新对应子任务文档。

## 18. 收官评审结论

### 18.1 当前最重要的残余风险

1. `AVX512` 虽已实现并可 override，但是否应重新升到 `AVX2` 之前仍缺少 benchmark 证据。
2. 当前 cross-backend matrix 已覆盖 `mul_slice / mul_slice_xor`，但 benchmark 门禁仍缺少多机型、可复现实测基线。
3. `GFNI` 当前虽然完成了域同构验证和实验性接线，也已有设计说明草案，但仍缺少系统化性能数据和更正式的推导材料。

### 18.2 推荐的默认优先级调整

从保守工程策略出发，建议将默认自动优先级调整为：

1. `rust-avx2`
2. `rust-avx512`
3. `rust-ssse3`
4. `simd-c`
5. `scalar-rust`

原因：

1. `AVX512` 目前没有 benchmark 数据证明其在真实目标机器上普遍优于 `AVX2`
2. `AVX512` 存在频率降档风险
3. `GFNI` 仍应保持 override-only 实验状态

### 18.3 推荐的后续修复顺序

1. 先补更多跨机器、同口径的 `AVX2 / AVX512 / GFNI` benchmark 数据
2. 在更多真实 CPU 上复核后，再决定是否让 `AVX512` 重回 `AVX2` 之前
3. 给 `GFNI` 补更正式的数学推导、准入说明与系统化性能报告
4. 后续若再调整 selector 或 benchmark 策略，应先同步 ledger 与摘要文档
