# reed-solomon-erasure 7.0.2 发布清单

本清单用于 7.0.2 patch release 的发布收口。7.0.2 聚焦流式路径（`encode_stream` / `verify_stream` / `reconstruct_stream`）的正确性与健壮性加固，来自一次逐行 + 多专家对抗验证的深度审计（PR #4）。

## 1. 版本冻结

- `Cargo.toml` package version: `7.0.2`
- workspace dependency `rustfs-erasure-codec`: `7.0.2`
- `Cargo.lock` root package version: `7.0.2`
- README / README_CN 安装示例版本: `7.0.2`
- `CHANGELOG.md` 已新增 `7.0.2 (2026-07-13)` 条目。
- release 之前确认工作树无未提交变更（`git status --short` 为空）。

## 2. 7.0.2 发布重点

流式路径修复（详见 `CHANGELOG.md`）：

- `reconstruct_stream`：校验同一块内 present 分片长度一致，不一致返回 `IncorrectShardSize`，不再静默补零导致错误恢复。
- `reconstruct_stream`：读取前将 present cursor 的 position 重置为 `0`，避免 position≠0（如刚被写入）被误读为空而静默不恢复。
- `encode_stream` / `verify_stream` / `reconstruct_stream`：入口 clamp `block_size` 到 `[1 KiB, 16 MiB]`，杜绝 `block_size = 0` 的静默空输出与超大值 OOM。
- `encode_stream` / `verify_stream` / `reconstruct_stream`：数量校验改为运行时返回 `TooFewShards`，避免 release 下 `debug_assert` 被移除导致的越界 panic。
- `encode_stream` / `verify_stream` / `reconstruct_stream`：对 Leopard 家族显式返回 `UnsupportedCodecFamily`。
- `reconstruct_stream`：空数据集返回 `Ok`，与 `encode_stream` 一致。
- 并行写错误上报改为写错误 kind 且保留最小 `shard_index`，上报确定化。

同时更新：README / README_CN 流式说明、CHANGELOG、文档版本引用，并补充回归测试。

## 3. 推荐发布前验证

```bash
cargo test --lib --features std -- stream
cargo clippy --all-targets --all-features -- -D warnings
```

预期结果：

- `cargo test --lib`: pass（281 原有 + 6 新增回归测试 = 287 passed）
- `cargo clippy --all-targets --all-features -- -D warnings`: pass

## 4. 性能验证

内存态吞吐基准（`ReedSolomon(10, 4)`、8 MiB/分片、4 MiB 块）对比 `7.0.1 base` 与本次修复：

- `encode_stream` / `verify_stream` / `reconstruct_stream` 吞吐差异均在 ±0.5% 以内（测量噪声范围），无回归。
- 所有修复都在非热路径（入口校验、每块一次的长度检查、错误路径），未触碰 GF 编解码计算；`reconstruct_stream` 因去掉冗余 `resize` 甚至有噪声级微小正收益。

## 5. Tag 到发布

```bash
git tag -a v7.0.2 -m "release: v7.0.2"
git push origin v7.0.2
```

Release note 模板：

```text
## reed-solomon-erasure 7.0.2

Patch release focused on streaming-path correctness and robustness hardening from a deep audit.

### Highlights
- Validate present shard lengths in `reconstruct_stream` (no more silent wrong recovery).
- Reset present cursor position before reading (no more silent no-recovery).
- Clamp `block_size` on entry (no more `block_size = 0` empty output or huge-value OOM).
- Validate stream counts at runtime instead of `debug_assert` (no release-build panics).
- Reject Leopard-family codecs from streaming with `UnsupportedCodecFamily`.
- Return `Ok` for an empty dataset in `reconstruct_stream`; deterministic parallel-write error reporting.
- Added regression tests; updated README, CHANGELOG, and docs.

### Validation
- `cargo test --lib --features std -- stream`: PASS (287 passed)
- `cargo clippy --all-targets --all-features -- -D warnings`: PASS
- Streaming throughput: no measurable regression (within ±0.5%).
```
