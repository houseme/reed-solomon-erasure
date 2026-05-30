# EC 小文件检测结果（2026-05-27）

## 1. 运行信息

- machine: Apple Silicon MacBook Pro
- target: `aarch64-macos-unknown`
- commit: `7b23860`
- command:

```bash
RSE_SMALL_FILE_PROFILE=fast \
cargo test --release --features "std simd-accel" \
  --test benchmark_small_files \
  benchmark_small_file_matrix_runs_and_exports_results -- --nocapture
```

- profile: `fast`
- backend: `rust-neon` (`backend_override=auto`)
- features: `std|simd-accel`

## 2. artifact

- json: `target/benchmark-smoke/small-file-results.json`
- csv: `target/benchmark-smoke/small-file-results.csv`

## 3. 覆盖范围

本轮 `fast` 覆盖：

- `4+2`: `1 KiB / 4 KiB / 16 KiB / 64 KiB / 128 KiB / 256 KiB / 512 KiB`
- `10+4`: `16 KiB / 64 KiB / 256 KiB / 512 KiB`
- operation: `encode / verify / reconstruct / reconstruct_data`

## 4. 关键结果摘要

### 4.1 `4+2` 小尺寸区间

| case | encode ns | verify ns | reconstruct ns | reconstruct_data ns |
| --- | ---: | ---: | ---: | ---: |
| `4x2_1k` | 11343.75 | 8145.75 | 10010.25 | 13187.50 |
| `4x2_4k` | 28729.25 | 29906.25 | 29812.50 | 29073.00 |
| `4x2_16k` | 109291.75 | 115260.50 | 118364.50 | 114135.50 |
| `4x2_64k` | 435437.50 | 457250.00 | 422333.25 | 389323.00 |

观察：

- `1 KiB` 明显是固定开销主导区，`throughput_mb_s` 参考意义弱于 `ns_per_iter`
- 到 `4 KiB` 后四个操作开始靠拢，说明最极端的小尺寸固定成本已经被摊薄一部分
- `64 KiB` 时 `reconstruct_data` 开始明显优于 `encode/verify`

### 4.2 `4+2` 过渡区间

| case | encode tp | verify tp | reconstruct tp | reconstruct_data tp |
| --- | ---: | ---: | ---: | ---: |
| `4x2_128k` | 677.69 | 699.97 | 724.44 | 723.53 |
| `4x2_256k` | 848.67 | 796.19 | 892.02 | 907.49 |
| `4x2_512k` | 1005.68 | 995.24 | 1072.03 | 1114.56 |

观察：

- `128 KiB -> 256 KiB -> 512 KiB` 过渡比较平滑，没有出现明显的异常回落
- `reconstruct` / `reconstruct_data` 在这几个点上普遍不弱于 `encode`
- `verify` 在 `256 KiB` 相对偏弱，后续值得结合主 throughput 或 profile 继续关注

### 4.3 `10+4` 中等扇出小文件区间

| case | encode tp | verify tp | reconstruct tp | reconstruct_data tp |
| --- | ---: | ---: | ---: | ---: |
| `10x4_16k` | 1096.41 | 1008.33 | 1061.50 | 1091.86 |
| `10x4_64k` | 1161.28 | 1046.65 | 1098.54 | 1096.01 |
| `10x4_256k` | 1205.81 | 1122.84 | 1199.83 | 1225.67 |
| `10x4_512k` | 1285.72 | 1161.21 | 1235.49 | 1208.53 |

观察：

- `10+4` 在 `16 KiB` 就已经明显高于 `4+2`，说明当前实现对更高数据扇出并没有在这个区间显著失稳
- `verify` 在所有 `10+4` 点位都略弱于其他操作，这个现象和 `4+2` 一致，后续可以单独观察是否与 verify 路径本身的常量开销有关
- `10x4_512k` 下 `encode` 最强，而 `reconstruct_data` 不再持续领先，说明到了更大尺寸后不同路径开始进入各自稳态，不应简单把小尺寸结论外推到全部区间

## 5. 本轮结论

1. 小文件专项检测是必要的，尤其是 `1 KiB / 4 KiB / 16 KiB`，这些点位与主 smoke 的 `64 KiB+` 结论不能互相替代。
2. 当前 Apple Silicon `rust-neon` 路径下，`4+2` 的 `1 KiB` 明显是固定开销主导区，应以 `ns_per_iter` 为主，不应只看吞吐。
3. `64 KiB -> 512 KiB` 的过渡整体平滑，暂时没有看到显著异常阈值切换。
4. `verify` 在 `4+2` 和 `10+4` 上都略弱于 `encode/reconstruct`，值得在后续专项中持续跟踪，但这轮还不能直接判定为问题。

## 6. 作为后续 baseline 的建议

当前这轮结果可以作为：

- Apple Silicon
- `aarch64`
- `rust-neon`
- `std|simd-accel`
- `fast` profile

下的小文件专项首轮参考基线。

但在正式作为长期 regression baseline 前，建议补两件事：

1. 再重复跑至少 `2~3` 轮 `fast`
2. 至少补一轮 `extended`，把 `1 MiB` 衔接点和 `10+4` 的更完整小尺寸梯度也纳入

## 7. 后续动作建议

1. 补跑 `extended` profile，形成更完整的小文件基线
2. 结合 `throughput_matrix` 或 profile 继续核查 `verify` 在 `256 KiB` 前后的相对弱势是否稳定复现
3. 若后续涉及并行阈值、small-output、`reconstruct_data` data-stage 的修改，优先把本文件作为回写入口持续追加
