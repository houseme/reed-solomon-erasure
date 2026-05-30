## 2026-05-26 amd-epyc-9v45

### 机器

1. 机器标识：`amd-epyc-9v45`
2. 日期：`2026-05-26`
3. 对应 JSON：`benchmarks/x86_64-simd/2026-05-26-amd-epyc-9v45.json`

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

### 结论

1. 当前默认自动策略不调整，继续保持 `rust-avx2 -> rust-avx512 -> rust-ssse3 -> simd-c -> scalar-rust`
2. `GFNI` 仍保持 `override-only`
3. 与当前已归档的 `AMD EPYC 9V45` 汇总结论一致，仍不支持把 `GFNI` 直接纳入默认自动路径
4. 仍需要更多机器样本，尤其用于复核 `AVX2 / AVX512 / GFNI` 的跨机器稳定性
