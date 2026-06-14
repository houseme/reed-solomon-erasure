# EC AArch64 验证结论数据（2026-06-14）

## 数据来源

- `docs/ec-aarch64-validation-review-2026-06-14.md`
- `docs/benchmark-methodology.md`
- `docs/ec-small-file-benchmark-playbook.md`
- `benchmarks/small-file/2026-05-27-aarch64-apple-silicon-extended.csv`

## 结论数据

- 验证环境：Apple Silicon / `aarch64-apple-darwin`
- 目标分支：`main`（当时 HEAD `d1edce6`）
- 小文件基线文件：`benchmarks/small-file/2026-05-27-aarch64-apple-silicon-extended.csv`
- 触发命令：
  - `RSE_SMALL_FILE_PROFILE=extended bash scripts/run_small_file_benchmark_matrix.sh`
  - `RSE_SMALL_FILE_PROFILE=extended RSE_SMALL_FILE_CASE_FILTER=10x4_1k RSE_SMALL_FILE_ITERATIONS=40 cargo test --release --features "std simd-accel" --test benchmark_small_files benchmark_small_file_matrix_runs_and_exports_results -- --ignored --nocapture`
  - `RSE_SMALL_FILE_PROFILE=extended RSE_SMALL_FILE_CASE_FILTER=4x2_1k,10x4_512k RSE_SMALL_FILE_ITERATIONS=40 cargo test --release --features "std simd-accel" --test benchmark_small_files benchmark_small_file_matrix_runs_and_exports_results -- --ignored --nocapture`

## 关键对比（小文件）

| 度量点 | 当前值 | 对照值（基线） | 结论 |
| --- | --- | --- | --- |
| `10x4_1k encode` | `3383.20 ns` | `3375.00 ns` | 波动极小，未形成稳定回退 |
| `4x2_1k verify`（高迭代复跑） | `432.27 ns` | 未能形成一致单点可重复可比值 | 对单点异常值敏感，不能作为稳定回退判据 |
| `10x4_512k verify`（高迭代复跑） | `566816.65 ns` | 与先前一次性回放偏差不一致 | 单次回放的“回退”与重复复跑结果不一致 |

## 判断

- 小文件是否要评测：是，至少覆盖 `1 KiB / 4 KiB / 16 KiB / 64 KiB / 128 KiB / 256 KiB / 512 KiB`
- 当前是否存在可复现小文件回退：否（1 KiB~512 KiB 未见稳定可复现回退）
- 需要的优先动作：修复验证链路与文档一致性，而不是立刻新增 EC 内核优化
