# Stream Path Performance Optimization Plan v2

> 日期: 2026-06-28
> 范围: `src/core/stream.rs` 的 encode / verify / reconstruct 流式路径
> 目标: 用可复现基准先锁定 stream 层固定成本，再以小批次改动降低分配、清零、调度和 reconstruct 搬运开销。

## 1. Executive Summary

v1 方案判断方向正确: stream 路径不能直接从 SIMD 内核入手，而要先量化 stream 层自身成本。v2 在五个专家视角 review 后，收敛为一个更可执行的路线:

1. 先新增 stream 专项 benchmark 和 artifact 输出，不先改热路径。
2. 第一批代码优化只处理低风险固定成本: buffer resize/clear、引用向量复用、重复 helper 收敛。
3. 第二批才调整并行 I/O 策略: 将当前“总是 rayon 并行”改为可观测的 `Auto / Serial / Parallel`。
4. 第三批专攻 `reconstruct_stream`: 借用已有 reconstruct workspace / preplanned 能力，减少每块 `Vec<Option<Vec<u8>>>` 和 `mem::take` 搬运。
5. 每一步都需要独立 benchmark 证据和 rollback 边界。

核心原则: 不用单次大重构换不可解释的吞吐数字。stream 路径涉及 I/O、调度、codec、内存写入四类成本，必须用分层基准拆开。

## 2. Current Code-Backed Facts

当前实现关键事实:

- `StreamOptions` 只有 `block_size`，默认 `4 MiB`，并 clamp 到 `1 KiB..16 MiB`。
- `encode_stream` 总是执行 `read_block_par -> encode_sep_par -> write_block_par`。
- `verify_stream` 总是执行 `read_block_all_par -> verify_par`。
- `reconstruct_stream` 只支持 `Cursor<Vec<u8>>` 形态，并在每个 block 构造 present shard 索引、`Vec<Option<Vec<u8>>>`，再调用 `self.reconstruct(...)`。
- 已有 `ParallelPolicy`、`effective_parallel_policy()`、`RuntimeProfileStats`、`verify_with_buffer`、`prepare_reconstruct_opt_workspace`、`reconstruct_opt_with_workspace` 等能力可以复用。
- 已有 benchmark 规范要求 artifact 包含 `schema_version`、`artifact_kind`、`git_revision`、`target_triple`、`features`、backend 信息和 operation 维度。

v1 中“新增 stream benchmark 先行”的判断保留。v2 修正两点:

- 不能只测 `Cursor`，否则会把真实文件 I/O 调度问题隐藏掉。
- 不能把并行开关直接做成默认行为变化，必须先有 `Auto` 策略和串行/并行对照数据。

## 3. Five-Expert Review

### 3.1 Performance Kernel Expert

结论: 首批不应碰 GF 内核或 Leopard 路由。

理由:

- stream hot path 当前可疑成本在 block 级别固定开销，例如 `resize(max_len, 0)`、尾部清零、引用向量重建、rayon 调度。
- 内核已有 `encode_sep_par`、`verify_par` 和 policy 统计。先改内核会把 stream overhead 和 codec throughput 混在一起。
- 对 `10+4`、`16+4`、`64+20` 这类常见 EC shape，stream 层小 block 固定成本可能比 codec 优化更直接。

建议:

- 用 `ns_per_block`、`ns_per_iter` 和 `throughput_mb_s` 同时看。
- 小 block 用 latency 优先，大 block 用 throughput 优先。
- 任何 “1 MiB 以上变快但 64 KiB 以下变慢” 的结果都不能算完整胜利。

### 3.2 Rust API / Ownership Expert

结论: API 要兼容当前调用者，但应补足调度表达能力。

问题:

- 旧设计文档提过 `concurrent_streams`，当前 public `StreamOptions` 没有这个字段，实际实现却总是并行。
- 当前 `encode_stream` / `verify_stream` 要求 `Read + Send` / `Write + Send`，即使未来串行模式也保留了 Send 约束。
- `reconstruct_stream` 的 `Cursor<Vec<u8>>` API 不是通用流式 reconstruct，它更像内存 cursor 分块恢复。

建议:

- 新增 `StreamIoMode`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamIoMode {
    Auto,
    Serial,
    Parallel,
}
```

- `StreamOptions` 新增 `io_mode: StreamIoMode`，默认 `Auto`。
- 保留现有方法签名作为兼容起点；如果后续需要降低 Send bound，再另开 `*_serial_stream` 或 trait 分层，不在本轮混做。
- 为 `reconstruct_stream` 文档明确限制: 当前接口是 cursor-backed block reconstruct，不是任意 `Read + Write` 输出 API。

### 3.3 I/O Systems Expert

结论: 强制 rayon 并行 I/O 是最大不确定因素。

问题:

- 对内存 slice / cursor，rayon 调度可能比串行读写更贵。
- 对单盘文件，多 shard 并发读写可能放大随机 I/O 和 page cache 抖动。
- 对多文件、多设备或对象存储 reader，parallel I/O 可能有明显收益。

建议:

- benchmark 必须拆三种 I/O backend:
  - `Memory`: `Cursor<Vec<u8>>` / `&[u8]`，隔离 stream + codec CPU 成本。
  - `TempFile`: `BufReader<File>` / `BufWriter<File>`，模拟本地文件。
  - `SlowReader`: 人工小 chunk reader，模拟短读和 reader overhead。
- `Auto` 策略初始阈值保守:
  - `block_size < 256 KiB`: 默认串行 I/O。
  - `total_shards <= 6 && block_size <= 1 MiB`: 默认串行 I/O。
  - `block_size >= 4 MiB && total_shards >= 10`: 可尝试并行 I/O。
  - 环境变量或 builder 强制模式用于 benchmark 和故障回退。

### 3.4 Benchmark / Statistics Expert

结论: v2 必须先补 artifact，而不是只跑 criterion。

现有 benchmark 规范已经很好，stream benchmark 应继承它:

- `schema_version = 1`
- `artifact_kind = "stream-path-results"`
- 记录 `git_revision`、`target_triple`、`features`、backend 选择、operation、shard shape、block size、payload size、I/O backend、io mode。

新增指标:

- `throughput_mb_s`
- `ns_per_iter`
- `ns_per_block`
- `blocks_per_iter`
- `logical_data_bytes`
- `stream_block_size`
- `stream_io_mode`
- `stream_io_backend`
- `runtime_profile_stats` 快照

测试矩阵分三档:

`quick`

- shape: `4+2`
- shard_size: `64 KiB`
- block_size: `64 KiB`, `256 KiB`
- operation: encode, verify
- backend: Memory

`fast`

- shape: `4+2`, `10+4`
- shard_size: `64 KiB`, `1 MiB`, `16 MiB`
- block_size: `64 KiB`, `256 KiB`, `1 MiB`, `4 MiB`
- operation: encode, verify, reconstruct
- backend: Memory
- io_mode: Auto, Serial, Parallel

`extended`

- shape: `4+2`, `10+4`, `16+4`, `32+16`, `64+20`
- shard_size: `64 KiB`, `1 MiB`, `16 MiB`, `64 MiB`
- block_size: `64 KiB`, `256 KiB`, `1 MiB`, `4 MiB`, `8 MiB`, `16 MiB`
- operation: encode, verify, reconstruct
- backend: Memory, TempFile, SlowReader
- io_mode: Auto, Serial, Parallel

判定规则:

- 小 block (`<= 256 KiB`) 看 `ns_per_block` 和 `ns_per_iter`。
- 大 block (`>= 1 MiB`) 看 `throughput_mb_s`。
- 至少重复 3 轮，使用 median。
- 单个孤立点异常必须 filtered rerun，不直接改代码。

### 3.5 Reliability / Release Expert

结论: 每批改动必须具备 correctness gate、performance gate 和 rollback gate。

风险:

- 最后一块短读的 zero-padding 语义不能变。
- unequal shard length 的输出长度语义不能变。
- 并行错误传播要保留 first error 的 shard index。
- reconstruct 缺失 shard 数超过 parity 时必须早返回。
- 引入 `StreamIoMode` 后不能破坏默认用户行为，尤其是 `StreamOptions::default()`。

必须保留/新增测试:

- empty input
- unequal length input
- single-byte requested block size clamp to 1 KiB 后仍正确
- multi-block encode/verify/reconstruct
- read error shard index
- write zero shard index
- Auto/Serial/Parallel 输出一致
- reconstruct missing data + missing parity 混合场景

## 4. v2 Architecture

### 4.1 StreamOptions

目标 API:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamIoMode {
    Auto,
    Serial,
    Parallel,
}

#[derive(Debug, Clone)]
pub struct StreamOptions {
    pub block_size: usize,
    pub io_mode: StreamIoMode,
}
```

Builder:

```rust
impl StreamOptions {
    pub fn with_io_mode(mut self, mode: StreamIoMode) -> Self {
        self.io_mode = mode;
        self
    }
}
```

兼容性:

- `StreamOptions::default()` 使用 `StreamIoMode::Auto`。
- 不移除 `with_block_size`。
- 不恢复旧名 `concurrent_streams`，避免布尔值不能表达 Auto。

### 4.2 Stream I/O Decision

新增内部 helper:

```rust
struct StreamIoDecision {
    use_parallel_io: bool,
}
```

初版 `Auto` 只看稳定输入:

- `block_size`
- `reader_count` / `writer_count`
- `total_shards`
- `operation`

不要在第一版根据运行时历史吞吐自适应，避免行为不可复现。

### 4.3 Buffer Management

优化目标:

- data buffers 初始化后保持 capacity。
- 满块路径避免重复 zero-fill。
- 短读尾部只清 `total_read..actual_len`，不总是清 `total_read..block_size`。
- parity buffers 按 `actual_len` 维护 len，写前清零只覆盖 `actual_len`。
- `Vec<&[u8]>` / `Vec<&mut [u8]>` 作为 per-call scratch 重用。

注意:

- 对 data shard，必须保证传给 codec 的切片长度一致。
- 对短 shard，zero-padding 是 correctness 语义，不是可删除优化。
- 对 parity shard，encode 前必须保证输出区为 0 或 codec 本身完整覆盖。若不能证明完整覆盖，保守清零 `actual_len`。

### 4.4 Verify Stream

目标:

- 从 `verify_par` 切到可复用 buffer / workspace 形式，优先评估 `verify_with_buffer` 或 `verify_with_buffer_opt` 是否能降低每 block 临时分配。
- 复用 `refs` scratch。
- 在 benchmark 中保留 `verify_stream_current` 和 `verify_stream_reuse_buffer` 对照，确认收益来源。

### 4.5 Reconstruct Stream

短期目标:

- 将 per-block `indexed` 构造挪到 loop 外。
- 将 present/missing metadata 和 required output metadata 预计算。
- 复用 `Vec<Option<Vec<u8>>>` 容器，避免每个 block 重新分配外层 Vec。

中期目标:

- 如果 missing pattern 稳定，优先接入 `prepare_reconstruct_opt_workspace` + `reconstruct_opt_with_workspace`。
- 对只需要恢复 data shard 的场景，评估 `reconstruct_some` 或 data-only workspace，避免恢复不需要的 parity。

边界:

- 不在本轮把 `reconstruct_stream` 改成完全通用 `Read + Write` API。
- 不在未完成 benchmark 前重写 reconstruct 内核。

## 5. Implementation Plan

### Phase 0: Baseline and Instrumentation

交付:

- 新增 `tests/benchmark_stream_paths.rs` 或 `benches/stream_paths.rs`。
- 新增 `RSE_STREAM_PROFILE=quick|fast|extended`。
- 输出:
  - `target/benchmark-smoke/stream-path-results.json`
  - `target/benchmark-smoke/stream-path-results.csv`

建议命令:

```bash
RSE_STREAM_PROFILE=fast \
cargo test --release --features "std simd-accel" \
  --test benchmark_stream_paths \
  benchmark_stream_path_matrix_runs_and_exports_results -- --ignored --nocapture
```

验收:

- 能在当前 main 上生成 artifact。
- artifact 字段符合 `docs/benchmark-methodology.md` 的 schema 习惯。
- 至少覆盖 `encode_stream` / `verify_stream` / `reconstruct_stream`。

### Phase 1: Low-Risk Fixed-Cost Cleanup

交付:

- 收敛 `read_block` / `read_block_all` 重复逻辑。
- 减少 `resize(max_len, 0)` 的全量清零。
- 复用 refs scratch。
- 保留当前默认并行行为，不引入策略变化。

验收:

- `cargo test --lib --features std -- stream`
- stream fast profile 不回退。
- `4+2_64k`、`10+4_64k` 的 `ns_per_block` 有改善或持平。

Rollback:

- 若 correctness 测试失败，整批回滚。
- 若小 block 明显回退，保留 benchmark，回滚代码优化。

### Phase 2: StreamIoMode and Auto Policy

交付:

- 新增 `StreamIoMode`。
- `StreamOptions::default()` 使用 Auto。
- `with_io_mode(...)` builder。
- Auto 初始阈值基于 benchmark 结果落定。
- benchmark 增加 `stream_io_mode` 字段。

验收:

- Serial / Parallel / Auto 输出一致。
- Auto 在 small block 不慢于当前强制 parallel。
- Parallel 在 large block 或 TempFile 多 shard 场景保留收益。

Rollback:

- 如果 Auto 策略不稳定，将默认临时设为 Parallel 兼容当前行为，但保留显式 Serial/Parallel API 和 benchmark。

### Phase 3: Verify Stream Workspace Reuse

交付:

- `verify_stream` 复用 per-call parity buffer / refs scratch。
- 若合适，接入 `verify_with_buffer_opt`。

验收:

- `verify_stream` 在 Memory backend 小 block 的 `ns_per_block` 下降。
- `verify_stream` corrupted case 仍可早返回 false。
- read error 行为不变。

Rollback:

- 若 verify 逻辑复杂度过高且收益不稳定，只保留 scratch refs 复用，不接 workspace。

### Phase 4: Reconstruct Stream Workspace

交付:

- loop 外预计算 present/missing metadata。
- loop 内复用 option container。
- 对稳定 missing pattern 评估 `reconstruct_opt_with_workspace`。

验收:

- `reconstruct_stream` small block `ns_per_block` 改善。
- minimum present、missing data + parity、multi-block 语义不变。
- reconstruct fast/extended profile 不出现 broad regression。

Rollback:

- 如果 workspace 接入引入过多所有权复杂度，退回 metadata 预计算和 outer Vec 复用的小改动。

## 6. Validation Matrix

Correctness:

```bash
cargo test --lib --features std -- stream
cargo test --features std test_galois_8_reconstruct_opt_with_workspace_matches_reconstruct
cargo test --features std test_verify_with_buffer_par_matches_verify_with_buffer
```

Style:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Performance smoke:

```bash
RSE_STREAM_PROFILE=quick \
cargo test --release --features "std simd-accel" \
  --test benchmark_stream_paths \
  benchmark_stream_path_matrix_runs_and_exports_results -- --ignored --nocapture
```

Performance decision:

```bash
RSE_STREAM_PROFILE=fast \
cargo test --release --features "std simd-accel" \
  --test benchmark_stream_paths \
  benchmark_stream_path_matrix_runs_and_exports_results -- --ignored --nocapture
```

Extended release evidence:

```bash
RSE_STREAM_PROFILE=extended \
cargo test --release --features "std simd-accel" \
  --test benchmark_stream_paths \
  benchmark_stream_path_matrix_runs_and_exports_results -- --ignored --nocapture
```

## 7. Regression Criteria

Treat as a regression:

- `4+2` or `10+4` at `64 KiB` block has `ns_per_block` worse by more than 8% across repeated median.
- `1 MiB` or larger block has throughput worse by more than 8% without compensating small-block gain.
- Auto slower than both Serial and Parallel for a stable case.
- TempFile backend shows broad regression across encode and verify.
- Any correctness test changes output length semantics for unequal inputs.

Acceptable tradeoff:

- A single isolated 1 KiB/4 KiB point regresses once but disappears under high-iteration filtered rerun.
- Parallel mode regresses small block if Auto chooses Serial and explicit Parallel is documented as force mode.
- Memory backend and TempFile backend disagree, as long as the decision is documented and Auto is conservative.

## 8. Documentation Updates Required

After implementation, update:

- `docs/task-P0-2-streaming-api.md`: reflect actual `StreamIoMode`, not old `concurrent_streams`.
- `docs/benchmark-methodology.md`: add stream benchmark command and artifact schema.
- `docs/README-performance-index.md`: add stream performance plan and result links.
- `README.md` / `README_CN.md`: only if public API changes are finalized.

## 9. Recommended Execution Order

1. Implement Phase 0 benchmark and commit it alone.
2. Run current-main baseline and archive artifacts under `benchmarks/stream-path/`.
3. Implement Phase 1 fixed-cost cleanup.
4. Compare Phase 1 against baseline and commit only if data is clean.
5. Implement Phase 2 `StreamIoMode`.
6. Implement Phase 3 verify reuse.
7. Implement Phase 4 reconstruct workspace.

This order deliberately keeps benchmark infrastructure ahead of optimization. It also keeps each PR reviewable: benchmark first, low-risk cleanup second, behavior/API third, reconstruct workspace last.

## 10. Open Questions

1. 是否需要支持非 `Send` 的串行 stream reader/writer?
   - v2 建议暂不处理。它会影响 public signature，收益和兼容风险需要单独评估。
2. 是否要把 `reconstruct_stream` 改成 `Read + Write` 通用 API?
   - v2 建议暂不处理。当前 cursor-backed API 的语义已经不同于真正通用流式恢复。
3. Auto policy 是否需要环境变量覆盖?
   - 建议需要，至少支持 benchmark 和线上故障回退，例如 `RSE_STREAM_IO_MODE=serial|parallel|auto`。
4. 是否把 stream benchmark 放在 `tests/` 还是 `benches/`?
   - 若要稳定生成 JSON/CSV 并复用现有 ignored test 风格，优先放 `tests/benchmark_stream_paths.rs`。

## 11. Final Recommendation

采用 v2 路线。第一步不是优化代码，而是新增 stream benchmark。只有当 `Memory / TempFile / SlowReader` 三类 backend 的 baseline 可重复后，再推进 buffer cleanup 和 I/O mode。这样可以避免把 rayon 调度、文件系统、codec 内核和 buffer 清零混成一个不可解释的结果。

