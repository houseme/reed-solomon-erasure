# 子任务 07：cross-backend tests、benchmark 门禁、收尾治理

## 实施状态

已完成首轮收口。

实际提交：

1. `c9dd387` `test(simd): add cross-backend conformance matrix`

实际落地结果：

1. 已新增 x86 cross-backend conformance matrix
2. `benchmark_smoke` 已输出 `backend_id / backend_kind`
3. `galois_backend` benchmark 标签已带上 `backend_id / backend_kind`

补充查阅：

1. [x86_64 SIMD 验证结果与收官评审记录](./x86_64-simd-verification-results.md)

残余风险：

1. cross-backend conformance matrix 已在运行前做 CPU feature gating，但覆盖范围仍以 `mul_slice` 为主
2. 尚未把同等粒度的矩阵覆盖扩展到 `mul_slice_xor`
3. benchmark 门禁仍缺少多机型、可复现实测基线

## 1. 子任务目标

建立一套长期可维护的 SIMD backend 验收体系，使未来任何 backend 升级、降级、退役都有一致的 correctness 与 performance 判据。

## 2. 本阶段作用

前面各阶段解决的是“把东西做出来”，本阶段解决的是“确保以后不悄悄变坏”。

## 3. 测试体系建设

### 3.1 backend 函数级对照框架

建议引入统一 helper，支持对每个 backend 直接执行：

1. `mul_slice`
2. `mul_slice_xor`

并与 scalar 或指定基线做字节级对照。

### 3.2 输入矩阵

长度矩阵：

1. `0`
2. `1`
3. `15`
4. `16`
5. `17`
6. `31`
7. `32`
8. `33`
9. `63`
10. `64`
11. `65`
12. `255`
13. `256`
14. `257`
15. `4096`
16. `65536`
17. `1048576`

输入模式矩阵：

1. 全 0
2. 全 `0xff`
3. 递增
4. 重复模式
5. 固定 seed 随机
6. 非对齐视图

系数矩阵：

1. `0`
2. `1`
3. `2`
4. `15`
5. `16`
6. `31`
7. `127`
8. `173`
9. `255`
10. 若干随机值

### 3.3 selector 测试

必须单独覆盖：

1. auto 选择
2. override 合法值
3. override 非法值
4. override 指定不可用 backend
5. 平台特性不足时的回退逻辑

## 4. 集成测试建设

要求在不同 backend 下复用相同的：

1. `encode` 测试
2. `verify` 测试
3. `reconstruct` 测试
4. `reconstruct_data` 测试
5. golden vectors

## 5. benchmark 门禁建设

### 5.1 最低要求

保留并增强现有：

1. `benches/galois_backend.rs`
2. `tests/benchmark_smoke.rs`

### 5.2 benchmark 输出增强建议

建议结果中增加：

1. backend id
2. backend kind
3. CPU 型号
4. 关键 target features
5. commit SHA

### 5.3 门禁策略

建议最少形成以下原则：

1. 新 backend 未优于旧 backend 时，不得自动升优先级。
2. benchmark 结果没有重复验证时，不得据此退役旧 backend。
3. 只要 correctness 存疑，性能数据一律无效。

## 6. 退役策略

未来若要让 Rust backend 彻底替代 `simd_c`，必须满足：

1. 主流 ISA 都有 Rust 路径覆盖。
2. 跨 backend 一致性长期稳定。
3. benchmark 连续多轮无明显退化。
4. 老平台 fallback 行为仍明确。

## 7. 最终交付清单

本阶段完成后，应具备：

1. 可维护的 backend 测试框架
2. 可解释的 selector 测试框架
3. benchmark smoke 输出
4. criterion 基准对比能力
5. backend 升级/降级/退役判据

## 8. 推荐 commit

```text
test(simd): add cross-backend conformance matrix
bench(simd): add backend-gated performance smoke checks
```
