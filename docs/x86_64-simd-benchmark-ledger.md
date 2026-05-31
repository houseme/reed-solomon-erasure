# x86_64 SIMD Benchmark Ledger

本文档用于持续记录每一轮与 `x86_64` SIMD runtime dispatch 相关的代码改进、压测命令、机器环境与结论。

## 2026-05-26 Baseline Revalidation

### 背景

本轮目标不是按既有文档强推默认策略，而是先以当前机器实测结果验证默认 runtime dispatch 是否满足性能优先。

机器：

1. `AMD EPYC 9V45 96-Core Processor`
2. 指令集能力包含 `ssse3 / avx2 / avx512f / avx512bw / gfni`
3. 采样环境来自 `lscpu`，完整信息已写入 [benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json)
4. 本轮可读摘要见 [x86_64-simd-benchmark-summary-2026-05-26.md](/data/rustfs/reed-solomon-erasure/docs/x86_64-simd-benchmark-summary-2026-05-26.md)

### 已执行命令

1. `./scripts/run_x86_backend_smoke_matrix.sh 2026-05-26 amd-epyc-9v45`
2. `RSE_BACKEND_OVERRIDE=rust-avx2 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
3. `RSE_BACKEND_OVERRIDE=rust-avx512 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
4. `RSE_BACKEND_OVERRIDE=rust-gfni-avx512 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`

### Release Smoke 结论

关注案例：`10x4_1m`

1. `encode` 最快是 `rust-avx2`，`315.6882 MB/s`
2. `verify` 最快是 `rust-avx2`，`534.4593 MB/s`
3. `reconstruct` 最快是 `rust-avx2`，`721.9651 MB/s`
4. `reconstruct_data` 最快是 `rust-avx2`，`753.3249 MB/s`
5. `auto` 当前选中的 backend 是 `rust-avx2`，但实测仍略低于显式 `rust-avx2` override

综合排序：

1. `rust-avx2`
2. `rust-avx512`
3. `rust-gfni-avx512`
4. `rust-gfni-avx2`
5. `rust-ssse3`
6. `scalar`
7. `simd-c`

### Microbenchmark 观察

基于 `galois_backend`：

1. `rust-avx512` 在当前采样里没有稳定证明优于 `rust-avx2`
2. `rust-gfni-avx512` 在当前采样里也没有证明适合作为默认优先
3. 当前机器上，`AVX512 / GFNI` 更适合作为可选实验或定向 override，而不是自动主路径

### 本轮改进与结论

1. 保留 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust` 的默认自动顺序
2. 保留 `GFNI` 为 override-only 实验 backend
3. 更新 benchmark 汇总脚本中的 `current_runtime_priority_x86`，使其反映当前真实代码策略
4. 新增 `scripts/run_x86_backend_smoke_matrix.sh`，为后续每轮改进提供统一 smoke 采集入口
5. 已将 `mul_slice_xor` 补入 x86 cross-backend conformance matrix，并通过当前机器定向测试
6. benchmark 汇总结果已增加 `policy_eligible_default_priority`，显式区分“纯性能排序”和“当前可进入默认策略的排序”

### 后续准入规则

1. 只有当 `AVX512` 在 release smoke 与关键微基准上稳定优于 `AVX2`，才考虑提升默认优先级
2. 只有当 `GFNI` 在正确性、性能与文档说明三方面同时收口，才考虑退出 override-only
3. 每次涉及 selector 或 backend 行为调整，都应先更新本 ledger，再同步更新摘要文档

### 第二台 GFNI 机器预留

在第二台支持 `GFNI` 的 x86_64 机器可用前，已经预留以下文档：

1. [x86_64-simd-second-gfni-machine-template.md](/data/rustfs/reed-solomon-erasure/docs/x86_64-simd-second-gfni-machine-template.md)
2. [x86_64-simd-second-gfni-machine-checklist.md](/data/rustfs/reed-solomon-erasure/docs/x86_64-simd-second-gfni-machine-checklist.md)

## 2026-05-26 Auto Priority Recheck After Conservative Rollback

### 背景

在提交 `51d6e44 fix: restore conservative x86 dispatch policy` 之后，需要回答一个更具体的问题：

1. 是否应该恢复 `GFNI` 自动优先？
2. 是否应该把 `rust-avx512` 提升到 `rust-avx2` 之前？

本轮目标不是重新追求单点峰值，而是确认在当前 `AMD EPYC 9V45` 主机上，哪些 backend 已经拥有足够稳定的证据进入自动默认路径。

### 已执行命令

Release smoke:

1. `RSE_SMOKE_PROFILE=extended RSE_SMOKE_ITERATIONS=3 RSE_BACKEND_OVERRIDE=rust-avx2 RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
2. `RSE_SMOKE_PROFILE=extended RSE_SMOKE_ITERATIONS=3 RSE_BACKEND_OVERRIDE=rust-avx512 RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
3. `RSE_SMOKE_PROFILE=extended RSE_SMOKE_ITERATIONS=3 RSE_BACKEND_OVERRIDE=rust-gfni-avx2 RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`
4. `RSE_SMOKE_PROFILE=extended RSE_SMOKE_ITERATIONS=3 RSE_BACKEND_OVERRIDE=rust-gfni-avx512 RSE_STRICT_BACKEND_OVERRIDE=1 cargo test --release --features 'std simd-accel' --test benchmark_smoke benchmark_smoke_matrix_runs_and_exports_results -- --nocapture`

Microbenchmark：

1. `RSE_BACKEND_OVERRIDE=rust-avx2 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
2. `RSE_BACKEND_OVERRIDE=rust-avx512 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
3. `RSE_BACKEND_OVERRIDE=rust-gfni-avx2 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
4. `RSE_BACKEND_OVERRIDE=rust-gfni-avx512 cargo bench --bench galois_backend --features 'std simd-accel' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`

### Release Smoke 结论

关注案例：`10x4_1m`

`encode`

1. `rust-avx512`: `681.4085 MB/s`
2. `rust-avx2`: `677.1211 MB/s`
3. `rust-gfni-avx512`: `668.6751 MB/s`
4. `rust-gfni-avx2`: `667.4278 MB/s`

`verify`

1. `rust-avx2`: `727.4593 MB/s`
2. `rust-avx512`: `725.9077 MB/s`
3. `rust-gfni-avx512`: `705.9474 MB/s`
4. `rust-gfni-avx2`: `692.6624 MB/s`

`reconstruct`

1. `rust-avx512`: `801.0335 MB/s`
2. `rust-avx2`: `794.5096 MB/s`
3. `rust-gfni-avx512`: `774.9127 MB/s`
4. `rust-gfni-avx2`: `741.6540 MB/s`

`reconstruct_data`

1. `rust-avx512`: `812.2612 MB/s`
2. `rust-avx2`: `807.2315 MB/s`
3. `rust-gfni-avx512`: `779.7315 MB/s`
4. `rust-gfni-avx2`: `764.1924 MB/s`

解释：

1. `rust-avx512` 在 `encode / reconstruct / reconstruct_data` 上小幅领先
2. `rust-avx2` 在 `verify` 上仍然最好
3. 两条 `GFNI` 路径都未在当前 smoke workload 上拿到综合第一

### Microbenchmark 观察

基于 `galois_backend`：

1. `rust-avx512` 的 `mul_slice` 短长度吞吐很强，但中大长度和 `mul_slice_xor` 并未稳定优于 `rust-avx2`
2. `rust-gfni-avx512` 在个别 `mul_slice` 大长度点位表现亮眼，但 `mul_slice_xor` 多个长度不稳定，不能支持直接进入自动优先
3. `rust-gfni-avx2` 更不像默认候选，尤其 `xor` 路径没有形成优势
4. 当前主机上，microbench 没有给出“`GFNI` 综合稳定优于 `AVX2/AVX512`”的证据

### 本轮策略结论

1. 当前证据不支持恢复 `GFNI` 自动优先
2. 当前证据也还不足以把 `rust-avx512` 提升到 `rust-avx2` 之前
3. 更稳妥的默认顺序仍应保持：
   - `rust-avx2`
   - `rust-avx512`
   - `rust-ssse3`
   - `simd-c`
   - `scalar-rust`
4. `GFNI` 继续保持 `override-only`

### 恢复自动优先所需的额外证据

若未来要重新讨论恢复 `GFNI / AVX512` 自动优先，至少应补齐：

1. 不止 `10x4_1m` 的更多 release smoke workload
2. 多轮重复采样，降低当前主机上的测量噪声
3. 第二台支持 `GFNI` 的 x86_64 主机结果
4. `mul_slice` 与 `mul_slice_xor` 两条 microbench 主线都具备更稳定的优势证据

## 2026-05-26 Extended Smoke Matrix Follow-up

### 背景

上一轮自动优先复核主要用 `10x4_1m` 作为代表 workload。为避免单点样本误导，本轮继续复用同一组 `extended smoke` 结果，补做整组 case 汇总，观察：

1. `rust-avx512` 是否只在单一场景领先
2. `rust-gfni-avx512` 是否已经具备跨 workload 的稳定优势
3. `rust-avx2` 的“保守默认”是否仍由更广覆盖的 smoke 证据支撑

### 覆盖范围

本轮整理由以下 `extended smoke` case 组成：

1. `4x2_64k`
2. `10x4_1m`
3. `32x16_1m`

每个 case 均对以下操作比较 4 个 backend：

1. `encode`
2. `verify`
3. `reconstruct`
4. `reconstruct_data`

候选 backend：

1. `rust-avx2`
2. `rust-avx512`
3. `rust-gfni-avx2`
4. `rust-gfni-avx512`

### 汇总结果

按 12 个 workload-operation 点位统计：

1. `rust-avx512` 取得第一：`8` 次
2. `rust-gfni-avx512` 取得第一：`3` 次
3. `rust-avx2` 取得第一：`1` 次
4. `rust-gfni-avx2` 取得第一：`0` 次

分场景观察：

`4x2_64k`

1. `encode`: `rust-avx512` 第一
2. `verify`: `rust-gfni-avx512` 第一
3. `reconstruct`: `rust-gfni-avx512` 第一
4. `reconstruct_data`: `rust-gfni-avx512` 第一

`10x4_1m`

1. `encode`: `rust-avx512` 第一
2. `verify`: `rust-avx2` 第一
3. `reconstruct`: `rust-avx512` 第一
4. `reconstruct_data`: `rust-avx512` 第一

`32x16_1m`

1. `encode`: `rust-avx512` 第一
2. `verify`: `rust-avx512` 第一
3. `reconstruct`: `rust-avx512` 第一
4. `reconstruct_data`: `rust-avx512` 第一

### 解释

1. 与只看 `10x4_1m` 相比，`rust-avx512` 在整组 `extended smoke` 上的证据明显更强，不再只是个别 workload 小幅领先
2. `rust-gfni-avx512` 在小 workload（尤其 `4x2_64k`）上有亮点，但仍未形成跨场景的综合第一
3. `rust-avx2` 当前更像“保守稳定默认”的代表，而不是“extended smoke 综合最优”的代表

### 当前结论更新

1. 这轮补充证据削弱了“`rust-avx2` 在 smoke 层面仍是综合最优默认路径”的说法
2. 但它仍不足以单独推动默认策略切换，因为：
   - 现有 `galois_backend` microbench 证据对 `AVX512 / GFNI` 还不够稳定
   - `GFNI` 尤其缺少 `mul_slice_xor` 主线上的稳定优势
   - 目前仍只有当前这台 `AMD EPYC 9V45` 主机结果
3. 因此，本轮更准确的状态应表述为：
   - `AVX512` 已经具备“值得继续追证、可能进入默认优先候选”的 smoke 证据
   - `GFNI` 仍不足以退出 `override-only`
   - 在没有更多重复采样和第二台机器前，默认顺序暂不变更

## 2026-05-26 Full Collection Plus Throughput Matrix Follow-up

### 背景

为把证据链补到“kernel + smoke + end-to-end”三层，本轮继续完成：

1. `scripts/collect_x86_simd_benchmarks.sh amd-epyc-9v45`
2. `throughput_matrix` 端到端旁证，仅比较：
   - `rust-avx2`
   - `rust-avx512`
   - `rust-gfni-avx512`

### 已执行命令

完整采集：

1. `./scripts/collect_x86_simd_benchmarks.sh amd-epyc-9v45`

端到端旁证：

1. `RSE_BACKEND_OVERRIDE=rust-avx2 RSE_PROFILE_REPORT_PATH=/tmp/throughput-rust-avx2-profile.json RSE_WRITE_PROFILE_REPORT=1 cargo bench --bench throughput_matrix --features 'std simd-accel benchmark-metrics' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
2. `RSE_BACKEND_OVERRIDE=rust-avx512 RSE_PROFILE_REPORT_PATH=/tmp/throughput-rust-avx512-profile.json RSE_WRITE_PROFILE_REPORT=1 cargo bench --bench throughput_matrix --features 'std simd-accel benchmark-metrics' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`
3. `RSE_BACKEND_OVERRIDE=rust-gfni-avx512 RSE_PROFILE_REPORT_PATH=/tmp/throughput-rust-gfni-avx512-profile.json RSE_WRITE_PROFILE_REPORT=1 cargo bench --bench throughput_matrix --features 'std simd-accel benchmark-metrics' -- --sample-size 10 --warm-up-time 1 --measurement-time 1`

### 完整采集结论

新生成的机器级结果：

1. [benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json)
2. [benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.run-meta.json](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.run-meta.json)

汇总 JSON 给出的结果：

1. `recommended_default_priority`：
   - `rust-avx512`
   - `rust-gfni-avx512`
   - `rust-avx2`
   - `rust-gfni-avx2`
   - `rust-ssse3`
   - `scalar`
   - `simd-c`
2. `policy_eligible_default_priority`：
   - `rust-avx512`
   - `rust-avx2`
   - `rust-ssse3`
   - `scalar`
   - `simd-c`
3. `adoption_decision_stub.status = manual-review-required`

解释：

1. 在同一批次、同一机器、完整 `release smoke + galois_backend` 采集下，`AVX512` 已经成为最强的默认候选
2. `GFNI` 即便在纯推荐排序里很高，也仍被 `policy_eligible_default_priority` 排除在默认候选之外，说明当前治理逻辑仍然认为它缺少准入证据

### Throughput Matrix 端到端观察

本轮 `throughput_matrix` 没有给出足够干净、足以直接改默认顺序的结论：

1. `rust-avx512` 在部分 `throughput_matrix` case 上表现积极，但端到端结果波动仍然较大
2. `rust-gfni-avx512` 虽然在某些 kernel / smoke 点位强，但端到端层面仍然缺少稳定、全面的领先证据
3. profile JSON 显示三者在并行/串行调用、chunk 使用、reconstruct data-stage 调用量上存在可观差异，但这些差异还不足以单独证明“更优默认策略”

### 本轮最终判断

1. 相比前一轮，这次完整采集明显增强了“`rust-avx512` 应进入默认优先候选”的证据
2. 但 `throughput_matrix` 端到端旁证仍然不够稳定，因此今天仍不建议仅凭这一轮就直接修改自动默认顺序
3. `GFNI` 仍然不应恢复自动优先，继续保持 `override-only`
4. 如果下一步要推进默认顺序变更，更合理的候选已经从“`GFNI/AVX512` 二选一”收敛成：
   - 只继续严肃评估 `rust-avx512` 是否提升到 `rust-avx2` 前面
   - 不再优先考虑恢复 `GFNI` 自动优先
