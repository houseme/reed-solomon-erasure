# EC 小文件检测结果（2026-05-27, extended）

## 1. 运行信息

- machine: Apple Silicon MacBook Pro
- target: `aarch64-macos-unknown`
- commit: `7b23860`
- command:

```bash
RSE_SMALL_FILE_PROFILE=extended \
cargo test --release --features "std simd-accel" \
  --test benchmark_small_files \
  benchmark_small_file_matrix_runs_and_exports_results -- --nocapture
```

- profile: `extended`
- backend: `rust-neon` (`backend_override=auto`)
- features: `std|simd-accel`
- iterations: `5`

## 1.1 当前口径

本文件当前采用修正后的检测口径：

- `verify / reconstruct / reconstruct_data` 的计时已剥离每轮前置 `encode` 成本
- 输入构造仍保留，但目标操作本身不再被“先 encode 一次”的成本污染

这份文档应视为当前主版本的 `extended` 结果。

## 2. artifact

- live json: `target/benchmark-smoke/small-file-results.json`
- live csv: `target/benchmark-smoke/small-file-results.csv`
- archived json: `benchmarks/small-file/2026-05-27-aarch64-apple-silicon-extended.json`
- archived csv: `benchmarks/small-file/2026-05-27-aarch64-apple-silicon-extended.csv`

## 3. 覆盖范围

本轮 `extended` 覆盖：

- `4+2`: `1 KiB / 4 KiB / 16 KiB / 64 KiB / 128 KiB / 256 KiB / 512 KiB / 1 MiB`
- `10+4`: `1 KiB / 4 KiB / 16 KiB / 64 KiB / 128 KiB / 256 KiB / 512 KiB / 1 MiB`
- operation: `encode / verify / reconstruct / reconstruct_data`

## 4. 关键结果摘要

### 4.1 `4+2` 的 `1 MiB` 衔接点

| case | encode tp | verify tp | reconstruct tp | reconstruct_data tp |
| --- | ---: | ---: | ---: | ---: |
| `4x2_1m` | 3673.38 | 3442.81 | 3385.75 | 3620.47 |

观察：

- `1 MiB` 并没有出现和 `512 KiB` 明显断层式的跳变，整体仍处于同一稳态带内
- 修正口径后整体吞吐显著抬高，说明旧口径确实混入了不该计入的前置 `encode` 成本
- `verify` 在 `1 MiB` 依然略弱，但现在这是更接近其真实路径成本的对比

### 4.2 `10+4` 新增的小尺寸梯度

| case | encode ns | verify ns | reconstruct ns | reconstruct_data ns |
| --- | ---: | ---: | ---: | ---: |
| `10x4_1k` | 3375.00 | 4966.80 | 4741.80 | 3958.40 |
| `10x4_4k` | 11566.60 | 12458.20 | 12633.40 | 11650.00 |
| `10x4_16k` | 47041.60 | 50775.00 | 47850.00 | 42441.80 |

观察：

- `10+4` 的 `1 KiB / 4 KiB` 梯度补齐后，可以确认 `verify` 在更小尺寸上就已经偏弱，不是只在中大尺寸才出现
- 在剥离前置 `encode` 成本后，`verify` 仍然最弱，说明这个趋势不是检测口径伪影
- 到 `16 KiB` 时四个操作重新靠拢，说明极端小尺寸下的额外开销在这个量级开始被摊薄

### 4.3 `10+4` 的 `128 KiB -> 1 MiB` 过渡

| case | encode tp | verify tp | reconstruct tp | reconstruct_data tp |
| --- | ---: | ---: | ---: | ---: |
| `10x4_128k` | 3550.48 | 3320.78 | 3907.78 | 3938.33 |
| `10x4_256k` | 3808.92 | 3512.82 | 4034.29 | 4119.35 |
| `10x4_512k` | 4121.76 | 3675.57 | 4305.41 | 4473.77 |
| `10x4_1m` | 4480.93 | 4029.82 | 4440.39 | 4590.94 |

观察：

- `10+4` 在 `128 KiB -> 1 MiB` 的过渡没有出现明显的结构性断层，但波动比 `4+2` 更大
- `verify` 在整个区间都相对偏弱，这个现象和 `1 KiB / 4 KiB` 的小尺寸结果一致，说明它更像是路径特征，而不是单一点位噪声
- `reconstruct_data` 与 `reconstruct` 在中大点位普遍更强，说明此前 mixed-cost 结果低估了恢复路径本身的能力

### 4.4 `4x2_1k` 更新后读数

| case | encode tp | verify tp | reconstruct tp | reconstruct_data tp |
| --- | ---: | ---: | ---: | ---: |
| `4x2_1k` | 1233.45 | 943.21 | 610.35 | 571.60 |

观察：

- 修正口径后，`4x2_1k reconstruct_data` 没有再出现之前那次极慢异常值
- 它在 `1 KiB` 下仍然比 `encode/verify` 慢，但已经回到合理量级
- 当前更像“小尺寸恢复固定成本确实更高”，而不是“存在偶发 10x 级退化”

## 5. 本轮结论

1. `1 MiB` 衔接点已经补齐，当前 Apple Silicon `rust-neon` 路径下，小文件到主吞吐区之间没有看到明显断层。
2. 修正检测口径后，可以更明确地确认 `verify` 在小尺寸到中等尺寸上持续偏弱，这不是前置 `encode` 成本混入造成的假象。
3. `10+4` 的 `128 KiB -> 1 MiB` 区间仍未看到必须立刻怀疑阈值切换失控的证据。
4. `4x2_1k reconstruct_data` 的前次极慢值没有在修正口径下再次出现，当前不再把它视为稳定异常点。

## 6. `verify` 专项跟进结论（2026-05-27）

针对 `10+4` 的 `1 KiB -> 1 MiB`，补了一组 `verify_with_buffer` 对照，目的是量化普通 `verify` 的临时 buffer 分配成本。

关键观察：

- `10x4_16k`: `verify = 1562.6 MB/s`, `verify_with_buffer = 1717.0 MB/s`
- `10x4_128k`: `verify = 2064.7 MB/s`, `verify_with_buffer = 2327.5 MB/s`
- `10x4_512k`: `verify = 3128.0 MB/s`, `verify_with_buffer = 3461.9 MB/s`
- `10x4_1m`: `verify = 3820.3 MB/s`, `verify_with_buffer = 3983.7 MB/s`

结论：

1. `verify` 的持续弱势里，确实包含了“每次调用都重新分配 scratch/buffer”的实现成本。
2. 这个额外成本在 `16 KiB` 到 `512 KiB` 区间最明显。
3. 上层如果能复用 parity buffer，优先使用 `verify_with_buffer` / `verify_with_buffer_opt` 更合适。

## 7. 本轮代码级优化结论（2026-05-27）

本轮对普通 `verify` / `verify_par` 做了一个低风险优化：

- 改为优先使用扁平 `SmallVec` scratch，而不是每次都新建多段 `Vec<Vec<_>>` 或大型堆分配 scratch

结果：

1. small-file 专项里的普通 `verify` 已经整体向 `verify_with_buffer` 靠近。
2. `cargo bench --bench throughput_matrix --features "std simd-accel"` 下的 `verify_10x4_1m` 变化落在噪声范围内，没有看到明显回退。
3. 说明这个优化可以保留，但它主要改善的是 small-file / 中小尺寸路径，不是大点位的颠覆性吞吐提升。

## 8. baseline 建议

这轮 `extended` 已经足够作为：

- Apple Silicon
- `aarch64`
- `rust-neon`
- `std|simd-accel`
- `extended` profile

的小文件专项完整参考样本。

并且由于口径已经修正为“只统计目标操作本身”，这份结果比旧版 mixed-cost 读数更适合作为后续 regression baseline。

## 9. 后续动作建议

1. 继续以这份修正口径的 `extended` 结果作为当前机器的主 baseline
2. 若后续再次出现单点异常，优先用 `RSE_SMALL_FILE_CASE_FILTER` 做最小复测
3. 如果要继续深入优化，下一步更值得关注的是是否给上层默认暴露/推广 `verify_with_buffer` 这一类可复用缓冲的校验路径
