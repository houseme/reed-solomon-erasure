# x86_64 SIMD 验证结果与收官评审记录

## 当前结论

截至 2026-05-26，`x86_64` SIMD runtime dispatch 链路已完成首轮实现，且已经过当前机器实测复核：

1. 默认自动优先级为 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust`
2. `GFNI` 保持实验性 backend，仅通过 `RSE_BACKEND_OVERRIDE` 暴露
3. cross-backend conformance matrix 已对 `mul_slice / mul_slice_xor` 做运行时 CPU feature gating
4. 在 `AMD EPYC 9V45` 上，release smoke 的 `encode / verify / reconstruct / reconstruct_data` 都显示 `rust-avx2` 是当前综合最优默认路径

## 已核实项

1. `src/galois_8/backend.rs` 已实现稳定 `BackendId`、override 解析和 feature-driven selector
2. `src/galois_8/x86/ssse3.rs`、`avx2.rs`、`avx512.rs`、`gfni.rs` 均有定向正确性测试
3. `tests/benchmark_smoke.rs` 已输出 `backend`、`backend_id`、`backend_kind`、`backend_override`
4. `benches/galois_backend.rs` 的 benchmark 标签已带上 backend 元数据

## 本轮补充验证

已执行：

1. `cargo check --lib`
2. `cargo check --features 'std simd-accel' --lib`
3. `cargo test --features 'std simd-accel' test_select_x86_backend_priority -- --nocapture`
4. `cargo test --features 'std simd-accel' test_select_x86_override_backend_allows_experimental_gfni -- --nocapture`
5. `cargo test --features 'std simd-accel' test_active_backend_metadata -- --nocapture`
6. `cargo test --features 'std simd-accel' test_x86_cross_backend_conformance_matrix -- --nocapture`
7. `cargo test --features 'std simd-accel' test_reconstruct_data_one_missing_skips_small_output_chunk_parallel_path -- --nocapture`
8. `cargo test --features 'std simd-accel' test_reconstruct_data_two_missing_skips_small_output_chunk_parallel_path -- --nocapture`
9. `RSE_BACKEND_OVERRIDE=auto RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
10. `RSE_BACKEND_OVERRIDE=rust-gfni-avx512 RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`

结果：

1. `cargo check` 与 `cargo check --features 'std simd-accel' --lib` 均通过
2. selector 默认优先级测试通过，确认自动路径重新稳定在 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust`
3. GFNI override 测试通过，确认 `rust-gfni-avx2 / rust-gfni-avx512` 仍可显式启用，但不会进入自动默认路径
4. active backend 元数据测试通过；在当前 `AMD EPYC 9V45`（支持 `ssse3 / avx2 / avx512f / avx512bw / gfni`）上，`auto` 实际命中 `rust-avx2`
5. x86 cross-backend conformance matrix 通过，`mul_slice / mul_slice_xor` 在当前机器上的各 backend 一致性保持正确
6. `reconstruct_data` 的单缺片/双缺片并行路径测试通过，确认小输出恢复路径仍可执行；同时测试已与 `benchmark-metrics` feature gate 的真实语义对齐
7. release smoke 归档结果显示：
   - `auto` 结果文件中 `backend=rust-avx2`、`backend_override=auto`、`override_honored=true`
   - `rust-gfni-avx512` override 结果文件中 `backend=rust-gfni-avx512`、`backend_override=rust-gfni-avx512`、`override_honored=true`
8. 当前机器上 `rust-gfni-avx512` 在部分 smoke 场景有单点优势，但 `rust-avx2` 仍是当前默认策略的保守最优解；现阶段仍无充分证据把 `AVX512` 或 `GFNI` 提升为自动优先

## 本轮 Review 结论

1. 当前分支最明显的“补丁叠补丁”问题不在 SIMD 内核本身，而在 runtime dispatch 选路层：后续修改一度把 `GFNI / AVX512` 推进到自动默认路径前面，和既有文档、验证结论、上线检查口径发生漂移
2. 本轮已将 `src/galois_8/backend.rs` 收敛为更清晰的两层语义：
   - 自动默认选路只保留保守稳定路径
   - 实验/强制 backend 仅通过显式 override 进入
3. `Refactor SIMD mul slice backends` 这轮 ISA 内核重构本身以去重为主，没有发现新的性能退化证据，当前可以保留
4. `reconstruct_data` 的 1/2 输出并行恢复路径没有发现算法性错误，但测试口径中存在旧的 metrics 假设；本轮已把测试预期收敛到 `benchmark-metrics` feature gate 的真实行为，避免后续把“统计未开启”误判为功能回归

## 2026-05-26 再次压测复核

本轮追加目标：

1. 确认提交 `5b3cf9c` 当前默认行为是否相对 `253ff40` 出现性能回退
2. 区分“默认 backend 策略切换导致的回退”和“同 backend 实现本身回退”

本轮额外执行：

1. 在当前 `HEAD`（`5b3cf9c`）执行：
   - `RSE_SMOKE_PROFILE=extended RSE_SMOKE_ITERATIONS=3 RSE_BACKEND_OVERRIDE=auto RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
   - `cargo test --release --features 'std simd-accel' benchmark_reconstruction_hotspots -- --nocapture`
2. 在基线 worktree `253ff40` 上执行同样两条命令
3. 对 `rust-avx2` 做同 backend 公平对比：
   - `RSE_SMOKE_PROFILE=extended RSE_SMOKE_ITERATIONS=3 RSE_BACKEND_OVERRIDE=rust-avx2 RSE_STRICT_BACKEND_OVERRIDE=1 ... benchmark_smoke ...`
   - `RSE_BACKEND_OVERRIDE=rust-avx2 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`

结论：

1. 若按“默认 auto 行为”对比，当前 `HEAD` 相对 `253ff40` 的确存在明显性能回退
   - 原因不是 SIMD 内核实现突然退化，而是 `253ff40` 的 `auto` 在当前机器上实际命中 `rust-gfni-avx512`
   - 当前 `HEAD` 的 `auto` 已被收敛回保守策略，实际命中 `rust-avx2`
2. `benchmark_smoke` 的默认 `auto` 回归门对比失败，主要集中在 `encode / verify`
   - `encode` 回退约 `13.2% ~ 41.7%`
   - `verify` 回退约 `14.3% ~ 33.0%`
   - `reconstruct / reconstruct_data` 虽有下降，但仍在脚本阈值内
3. `reconstruction_hotspot` 对比同样显示当前默认路径相对 `253ff40` 下降
   - `reconstruct_data_missing_1_data`: `-25.96%`
   - `reconstruct_data_missing_2_data`: `-19.31%`
   - `reconstruct_data_missing_data_plus_parity`: `-20.60%`
   - `reconstruct_data_32x16_missing_2_data`: `-29.63%`
4. 但在“同 backend = rust-avx2” 的公平对比下，没有观察到同等级别的实现回退
   - `benchmark_smoke` 的 `rust-avx2` 对 `253ff40` 基线全部通过回归阈值
   - 多数 case 波动在约 `0.1% ~ 4.6%` 之间，属于同机基准常见抖动范围
   - 其中 `verify 32x16 1MiB` 反而提升约 `29.45%`
5. 因此，本轮可确认的事实是：
   - “当前默认性能”相对 `253ff40` 有回退
   - 这份回退主要来自默认 backend 从 `rust-gfni-avx512` 切回 `rust-avx2`
   - 不能把它简单归因于这次代码收敛引入了 `rust-avx2` 内核退化

## 仍待完成

1. 仍缺多轮、跨机器的 `AVX2 vs AVX512` 对比数据
2. `GFNI` 的系统化性能验证仍未补齐
3. 若要提升 `AVX512` 或 `GFNI` 默认优先级，仍需先补多轮 benchmark 证据

## 收官建议

1. 在当前已采样的 `AMD EPYC 9V45` 机器上，继续保持 `AVX2 -> AVX512` 的默认顺序
2. 继续保持 `GFNI` 为 override-only 实验路径
3. 下一步优先补更多跨机器 benchmark 结果归档，并继续完善 `GFNI` 设计与性能说明
