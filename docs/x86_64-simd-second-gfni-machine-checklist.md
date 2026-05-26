# x86_64 SIMD Second GFNI Machine Checklist

在第二台支持 `GFNI` 的机器上复跑 benchmark 前，建议按以下清单执行。

## 采集前

1. 确认 `lscpu` 输出包含 `gfni`
2. 确认当前代码已包含最新的 benchmark 汇总脚本
3. 确认 `cargo test --features simd-accel test_x86_cross_backend_conformance_matrix -- --nocapture` 通过

## 采集命令

1. 运行 `./scripts/run_x86_backend_smoke_matrix.sh <date> <machine-slug>`
2. 运行 `rust-avx2 / rust-avx512 / rust-gfni-avx2 / rust-gfni-avx512` 的 `galois_backend` 微基准
3. 保留生成的 JSON 与 `target/benchmark-smoke/smoke-results.csv` / `smoke-results.json`

## 归档要求

1. 生成新的 machine JSON，命名为 `benchmarks/x86_64-simd/<date>-<machine-slug>.json`
2. 生成对应的 markdown 摘要
3. 追加一段到 `x86_64-simd-benchmark-ledger.md`
4. 对照 [x86_64-simd-gfni-design.md](/data/rustfs/reed-solomon-erasure/docs/x86_64-simd-gfni-design.md) 的准入清单，明确写出本轮是否改变 `GFNI` 的策略状态

## 最低结论要求

1. 明确 `recommended_default_priority`
2. 明确 `policy_eligible_default_priority`
3. 明确 `GFNI` 是否仅在 raw ranking 中靠前
4. 明确 `GFNI` 是否仍保持 `override-only`
