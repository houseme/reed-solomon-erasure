# 子任务 06：x86_64 GFNI backend 设计、验证与实验集成

## 实施状态

已完成实验性接入。

实际提交：

1. `8d33660` `feat(x86): add experimental gfni backend`

实际落地结果：

1. 已实现 `rust-gfni-avx2`
2. 已支持 `RSE_BACKEND_OVERRIDE=rust-gfni-avx2`
3. 当前仍保持 override-only，没有进入自动优先级
4. 已完成本地域同构验证，确认当前 `0x11d` 域与 GFNI 常见 `0x11b` 域之间存在可逆线性同构

残余风险：

1. 数学说明已补入代码注释，但仍缺少更完整的设计文档与推导记录
2. benchmark 数据尚未补齐
3. 仍需补更多 golden vector 与性能回归验证

## 1. 子任务目标

在数学正确性可证明、测试完备、benchmark 有收益的前提下，为 `x86_64` 引入 `GFNI` backend，并以实验性策略逐步接入 runtime dispatch。

## 2. 这是最高风险阶段

该阶段风险高于 `SSSE3 / AVX2 / AVX512`，原因如下：

1. `GFNI` 不只是宽度变化，而是乘法语义路径变化。
2. 当前库的 GF(2^8) 表示与 GFNI 工作域未必直接等价。
3. 贸然替换可能产生 silent corruption。

## 3. 强制前置条件

在开始前，必须先满足：

1. `x86_64` 平台拆分完成。
2. selector 稳定。
3. `SSSE3 / AVX2 / AVX512` 路径与 scalar 全量一致。
4. 有完整 cross-backend tests 基础设施。

## 4. 实施路线

### 步骤 1：先写设计说明

必须先形成以下内容，再写代码：

1. 当前 GF(2^8) 生成多项式定义与表示说明
2. GFNI 目标域的乘法语义说明
3. basis conversion 的设计方案
4. 常量系数如何参与变换的说明

### 步骤 2：实现 prototype

建议先做实验版：

1. 只实现 `mul_slice`
2. 小范围测试
3. 不进入自动优先级

### 步骤 3：扩展到 `mul_slice_xor`

要求：

1. 与 prototype 一样先对 scalar 做强对照
2. 覆盖所有边界长度

### 步骤 4：接入 runtime override

新增：

1. `rust-gfni-avx2`
2. 可选 `rust-gfni-avx512`

此阶段建议先 override-only，不自动启用。

### 步骤 5：benchmark 决策

确认：

1. basis conversion 开销是否被吞吐收益覆盖
2. 大块数据上是否优于 AVX2/AVX512

## 5. 强制测试要求

必须覆盖：

1. scalar 对照
2. AVX2 对照
3. AVX512 对照
4. golden vector
5. 随机输入高覆盖
6. 不同系数高覆盖
7. 长度边界与非对齐高覆盖

## 6. 自动启用条件

只有同时满足以下条件才允许进入自动选择顺序：

1. 数学语义经过审查确认
2. 所有正确性测试通过
3. benchmark 有稳定正收益
4. 在至少一种真实 GFNI 机器上完成验证

否则保持为：

1. 实验 backend
2. 仅 override 可用

## 7. 完成定义

1. GFNI backend 设计明确。
2. prototype 或正式实现可运行。
3. 是否自动启用有明确结论。

## 8. 推荐 commit

若是实验接入：

```text
feat(x86): add experimental gfni backend
```

若经过验证进入正式路径：

```text
feat(x86): enable gfni backend in runtime dispatch
```
