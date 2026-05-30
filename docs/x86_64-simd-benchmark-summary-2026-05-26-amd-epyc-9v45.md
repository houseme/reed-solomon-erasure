# x86_64 SIMD Benchmark Summary (2026-05-26, amd-epyc-9v45)

## 范围

本摘要对应以下实测产物：

1. [benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json](/data/rustfs/reed-solomon-erasure/benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json)
2. `target/benchmark-smoke/smoke-results-release-*.csv`
3. `cargo bench --bench galois_backend --features 'std simd-accel'` 的当前 Criterion 输出

机器环境：

1. 机器标识：`amd-epyc-9v45`
2. 测试日期：`2026-05-26`
3. 详细 `lscpu` 信息已包含在 machine JSON 中

## 10x4_1m Release Smoke 排名

`encode`

1. `auto`: `660.6166 MB/s`
2. `rust-gfni-avx512`: `656.0620 MB/s`
3. `rust-avx512`: `645.3753 MB/s`

`verify`

1. `rust-avx512`: `620.5794 MB/s`
2. `rust-gfni-avx512`: `604.8421 MB/s`
3. `rust-avx2`: `591.7865 MB/s`

`reconstruct`

1. `rust-avx512`: `823.5043 MB/s`
2. `rust-gfni-avx512`: `821.6861 MB/s`
3. `rust-gfni-avx2`: `806.6535 MB/s`

`reconstruct_data`

1. `rust-gfni-avx512`: `854.2107 MB/s`
2. `rust-gfni-avx2`: `854.1865 MB/s`
3. `rust-avx2`: `841.6229 MB/s`

## 综合打分结果

### Raw Benchmark Ranking

1. `rust-gfni-avx2`
2. `rust-gfni-avx512`
3. `rust-avx2`
4. `rust-avx512`
5. `rust-ssse3`
6. `scalar`
7. `simd-c`

### Policy Eligible Default Priority

1. `rust-avx2`
2. `rust-avx512`
3. `rust-ssse3`
4. `simd-c`
5. `scalar-rust`

## 结论模板

1. 当前默认自动策略是否应调整：不调整，继续保持 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust`
2. `GFNI` 是否仍保持 `override-only`：是，当前仍不进入自动优先级
3. 与已有 `AMD EPYC 9V45` 结果是否一致：一致，仍支持把 `rust-avx2` 作为当前默认首选
4. 是否需要更多机器样本：需要，尤其是跨机器复核 `AVX2 / AVX512 / GFNI` 的稳定收益
