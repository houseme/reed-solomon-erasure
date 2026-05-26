# x86_64 SIMD 验证结果与收官评审记录

## 当前结论

截至 2026-05-26，`x86_64` SIMD runtime dispatch 链路已完成首轮实现，且已经过当前机器实测复核：

1. 默认自动优先级为 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust`
2. `GFNI` 保持实验性 backend，仅通过 `RSE_BACKEND_OVERRIDE` 暴露
3. cross-backend conformance matrix 已对 `mul_slice` 做运行时 CPU feature gating
4. 在 `AMD EPYC 9V45` 上，release smoke 的 `encode / verify / reconstruct / reconstruct_data` 都显示 `rust-avx2` 是当前综合最优默认路径

## 已核实项

1. `src/galois_8/backend.rs` 已实现稳定 `BackendId`、override 解析和 feature-driven selector
2. `src/galois_8/x86/ssse3.rs`、`avx2.rs`、`avx512.rs`、`gfni.rs` 均有定向正确性测试
3. `tests/benchmark_smoke.rs` 已输出 `backend`、`backend_id`、`backend_kind`、`backend_override`
4. `benches/galois_backend.rs` 的 benchmark 标签已带上 backend 元数据

## 本轮补充验证

已执行：

1. `cargo test --features simd-accel test_select_x86_backend_priority -- --nocapture`
2. `cargo test --features simd-accel test_active_backend_metadata -- --nocapture`
3. `./scripts/run_x86_backend_smoke_matrix.sh 2026-05-26 amd-epyc-9v45`
4. `cargo bench --bench galois_backend --features 'std simd-accel'`，分别对 `rust-avx2 / rust-avx512 / rust-gfni-avx512` 做 override 采样

结果：

1. selector 优先级测试通过
2. active backend 元数据测试通过
3. 当前代码行为与 release checklist 的默认策略一致
4. 当前机器上 `rust-avx2` 在关键 release smoke 场景里综合领先
5. 当前机器上 `AVX512 / GFNI` 尚无充分证据证明应提升为默认优先

## 仍待完成

1. 仍缺多轮、跨机器的 `AVX2 vs AVX512` 对比数据
2. `GFNI` 的系统化性能验证仍未补齐
3. `mul_slice_xor` 仍缺少与 `mul_slice` 同等粒度的 cross-backend matrix
4. 若要提升 `AVX512` 或 `GFNI` 默认优先级，仍需先补多轮 benchmark 证据

## 收官建议

1. 在当前已采样的 `AMD EPYC 9V45` 机器上，继续保持 `AVX2 -> AVX512` 的默认顺序
2. 继续保持 `GFNI` 为 override-only 实验路径
3. 下一步优先补 `mul_slice_xor` cross-backend conformance matrix 与更多 benchmark 结果归档
