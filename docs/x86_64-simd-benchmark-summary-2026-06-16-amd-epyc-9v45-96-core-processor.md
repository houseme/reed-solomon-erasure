# x86_64 SIMD Benchmark Summary (2026-06-16, amd-epyc-9v45-96-core-processor)

## 范围

本摘要对应以下实测产物：

1. [benchmarks/x86_64-simd/2026-06-16-amd-epyc-9v45-96-core-processor.json](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/2026-06-16-amd-epyc-9v45-96-core-processor.json)
2. [benchmarks/x86_64-simd/2026-06-16-amd-epyc-9v45-96-core-processor.run-meta.json](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/2026-06-16-amd-epyc-9v45-96-core-processor.run-meta.json)
3. `target/benchmark-smoke/smoke-results-release-*.csv`
4. `cargo bench --bench galois_backend --features 'std simd-accel'` 的当前 Criterion 输出

机器环境：

1. 机器标识：`amd-epyc-9v45-96-core-processor`
2. 主机名：`rustfs-jumpbox`
3. 测试日期：`2026-06-16`
4. 架构：`x86_64`
5. CPU：`AMD EPYC 9V45 96-Core Processor`
6. 指令集能力包含 `ssse3 / avx2 / avx512f / avx512bw / gfni`

## 本轮前置修复

本轮在正式压测前，先修复了当前 `main` 上的一个编译问题：

1. [src/core/mod.rs](/data/rustfs/reed-solomon-erasure/src/core/mod.rs:99) 原本错误引用了 `super::leopard::build_family_state`
2. 实际修复先落成同模块内调用，随后在 rebase 过程中与远端收敛为 `crate::core::leopard::build_family_state`
3. 修复后 `cargo check --features 'std simd-accel' --lib` 可以通过，压测流程才得以完成

## 当前代码自动选路现实

当前 `main` 的 `x86_64` 自动选路实现位于 [src/galois_8/backend.rs](/data/rustfs/reed-solomon-erasure/src/galois_8/backend.rs:489)，实际优先级是：

1. `rust-gfni-avx512`
2. `rust-gfni-avx2`
3. `rust-avx2`
4. `rust-avx512`
5. `rust-ssse3`
6. `simd-c`
7. `scalar-rust`

本轮 `auto` 的 release smoke 结果也确认，当前主机上 `auto` 实际选中的 backend 是 `rust-gfni-avx512`。

## 10x4_1m Release Smoke 排名

`encode`

1. `auto` (`rust-gfni-avx512`): `639.9361 MB/s`
2. `rust-gfni-avx512`: `606.8683 MB/s`
3. `rust-ssse3`: `594.7480 MB/s`
4. `rust-gfni-avx2`: `593.8489 MB/s`

`verify`

1. `auto` (`rust-gfni-avx512`): `817.6464 MB/s`
2. `rust-gfni-avx512`: `744.9607 MB/s`
3. `rust-avx512`: `729.6699 MB/s`
4. `rust-gfni-avx2`: `729.5057 MB/s`

`reconstruct`

1. `auto` (`rust-gfni-avx512`): `893.0404 MB/s`
2. `rust-avx512`: `838.7359 MB/s`
3. `rust-avx2`: `830.0097 MB/s`
4. `rust-gfni-avx512`: `828.2781 MB/s`

`reconstruct_data`

1. `auto` (`rust-gfni-avx512`): `897.7649 MB/s`
2. `rust-avx512`: `848.6149 MB/s`
3. `rust-avx2`: `844.5187 MB/s`
4. `rust-gfni-avx512`: `838.9544 MB/s`

## Microbenchmark 观察

基于 `galois_backend` 的 `1 MiB / 4 MiB` 重点长度：

1. `galois_mul_slice` 的 `1 MiB` 点位由 `rust-gfni-avx512` 轻微领先
2. `galois_mul_slice` 的 `4 MiB` 点位仍由 `rust-gfni-avx512` 领先
3. `galois_mul_slice_xor` 的 `1 MiB` 点位由 `rust-gfni-avx512` 领先
4. `galois_mul_slice_xor` 的 `4 MiB` 点位由 `rust-gfni-avx512` 领先

这说明当前主机上，`GFNI` 与 `AVX512` 都已经不只是单点优势，至少在这轮同机采样里属于主力候选，而不再符合旧文档中的“GFNI 仅 override-only”描述。

## 综合打分结果

### Raw Benchmark Ranking

1. `rust-gfni-avx512`
2. `rust-avx512`
3. `rust-gfni-avx2`
4. `rust-avx2`
5. `rust-ssse3`
6. `scalar`
7. `simd-c`

### Policy Eligible Default Priority

1. `rust-gfni-avx512`
2. `rust-avx512`
3. `rust-gfni-avx2`
4. `rust-avx2`
5. `rust-ssse3`
6. `scalar`
7. `simd-c`

## 本轮结论

1. 这轮 `x86_64` 同机完整采集表明，当前代码现实已经是 `GFNI` 自动优先，而不是旧文档记录的 `AVX2` 保守优先
2. 在本机 `10x4_1m` release smoke 上，`auto` 全部四项都跑在 `rust-gfni-avx512` 上，且结果都处于第一
3. 汇总脚本此前仍按旧口径把 `GFNI` 排除出默认候选，这和当前代码、测试以及本轮实测不一致；本轮已将结果记录口径校正到当前代码现实
4. 在同日基于更新后的远端 `main` 复跑后，`auto` 的 `encode / verify / reconstruct / reconstruct_data` 分别提升到 `639.9361 / 817.6464 / 893.0404 / 897.7649 MB/s`
5. 尽管如此，machine JSON 仍保留 `manual-review-required`，因为当前证据依然只有同一台 `AMD EPYC 9V45` 主机的一轮采集

## 后续建议

1. 如果要继续坚持“GFNI 仅实验性、不应自动启用”的产品策略，应先回收并统一 [src/galois_8/backend.rs](/data/rustfs/reed-solomon-erasure/src/galois_8/backend.rs:489)、相关测试和文档口径
2. 如果接受当前 `main` 的实现方向，则应继续在第二台支持 `GFNI` 的 `x86_64` 主机上复跑同一套采集，确认这种排序是否稳定
3. 无论后续策略选择哪条路，都应继续把“代码当前实现”和“文档建议策略”分开写，避免再次把旧结论误投射到新代码
