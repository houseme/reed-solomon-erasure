# x86_64 SIMD Runtime Dispatch 上线检查清单

## 2026-06-16 状态勘误

本清单主体对应的是 `2026-05-26` 的发布前快照。当前工作区若继续沿用本页中的“默认策略”勾选项，会与最新 `main` 的真实行为不一致。

截至 `2026-06-16` 当前工作区：

1. 当前自动选路不再是 `AVX2 -> AVX512 -> SSSE3 -> simd-c -> scalar`
2. `GFNI` 也不再只是“仅通过 override 暴露”的代码现实
3. 若要做新的发布前核查，应先以 [src/galois_8/backend.rs](/data/rustfs/reed-solomon-erasure/src/galois_8/backend.rs:489) 和 [x86_64-simd-benchmark-summary-2026-06-16-amd-epyc-9v45-96-core-processor.md](/data/rustfs/reed-solomon-erasure/docs/x86_64-simd-benchmark-summary-2026-06-16-amd-epyc-9v45-96-core-processor.md) 重建一版新的 checklist

## 1. 文档用途

本文档用于在合并、发布、上线、交接前，快速核对当前 `x86_64` SIMD runtime dispatch 改造链路是否满足最低可交付要求。

说明：

1. 本文档仅保存在 `docs/` 下
2. 本文档不进入代码 commit

截至 `2026-05-26` 的当前核查快照如下。

## 2. 代码状态检查

发布前确认：

- [ ] 工作区干净，没有未预期代码改动
- [ ] `docs/` 文档更新不会误进入 commit
- [ ] 最近阶段性 commit 链完整、语义清晰
- [x] `scalar / simd-c / rust-ssse3 / rust-avx2 / rust-avx512 / rust-gfni-avx2 / rust-gfni-avx512` 状态与文档一致

## 3. 默认策略检查

当前建议默认策略：

1. `rust-avx2`
2. `rust-avx512`
3. `rust-ssse3`
4. `simd-c`
5. `scalar-rust`

上线前确认：

- [x] 默认优先级仍为 `AVX2 -> AVX512 -> SSSE3 -> simd-c -> scalar`
- [x] `GFNI` 仍保持 override-only
- [x] `simd-c` 仍保持 legacy fallback
- [x] README 中对默认策略和 legacy/fallback 的描述与代码一致

## 4. 构建检查

最低要求：

- [x] `cargo check --lib` 通过
- [x] `cargo check --features simd-accel --lib` 通过

建议补跑：

- [ ] 在联网/依赖完整环境中执行完整 `cargo test`
- [ ] 在联网/依赖完整环境中执行 `cargo test --features simd-accel`

## 5. 测试检查

上线前确认：

- [x] scalar baseline 正常
- [x] dispatch 元数据测试通过
- [x] override 测试通过
- [x] x86 cross-backend conformance matrix 可运行
- [x] ISA 定向测试都带有 CPU feature gating

特别核对：

- [x] `SSSE3` 测试不会在不支持 `SSSE3` 的机器上直接执行
- [x] `AVX2` 测试不会在不支持 `AVX2` 的机器上直接执行
- [x] `AVX512` 测试不会在不支持 `AVX512` 的机器上直接执行
- [x] `GFNI` 测试不会在不支持 `GFNI` 的机器上直接执行

## 6. Benchmark 检查

建议至少补齐：

- [ ] `cargo bench --bench galois_backend --features simd-accel`
- [x] `benchmark_smoke` 输出可正常生成 JSON/CSV
- [x] 输出包含 `backend`
- [x] 输出包含 `backend_id`
- [x] 输出包含 `backend_kind`
- [x] 输出包含 `backend_override`
- [x] 本轮 benchmark 结论已追加到 `x86_64-simd-benchmark-ledger.md`

## 7. 性能决策检查

若准备进一步调整默认优先级，必须先确认：

- [x] 已有 `AVX2 vs AVX512` 真实性能对比
- [ ] 已在目标 CPU 上确认 `AVX512` 不会造成整体性能回退
- [x] 若考虑启用 `GFNI` 自动优先级，已有 `GFNI vs AVX2` 数据
- [x] 不会仅凭单机单次数据调整默认策略

## 8. 实验功能边界检查

当前实验性功能：

1. `rust-gfni-avx2`
2. `rust-gfni-avx512`

上线前确认：

- [x] 文档中明确说明 `GFNI` 为实验性
- [x] 没有把 `GFNI` 宣传为默认生产路径
- [x] `GFNI` 仅通过 override 暴露

## 9. 文档检查

建议联动核对：

- [x] [总执行指南](./x86_64-simd-runtime-dispatch-execution-guide.md)
- [x] [验证结果与收官评审记录](./x86_64-simd-verification-results.md)
- [x] [最终交付总结](./x86_64-simd-final-delivery-summary.md)
- [x] [Benchmark Ledger](./x86_64-simd-benchmark-ledger.md)

文档一致性确认：

- [x] 已实现 backend 与文档一致
- [x] 默认优先级与文档一致
- [x] 残余风险与文档一致
- [x] 推荐后续动作与当前代码状态一致

## 10. 结论模板

### 可发布

满足以下条件可判定为“可发布/可合并”：

1. 构建通过
2. 关键测试通过
3. 默认策略保守稳定
4. 实验功能边界清晰
5. 文档与代码一致

### 暂缓发布

出现以下任一情况建议暂缓：

1. 关键测试未通过
2. ISA 测试仍可能触发非法指令
3. 默认优先级与 benchmark 结论不一致
4. `GFNI` 被误纳入默认路径
5. 文档与代码状态明显漂移
