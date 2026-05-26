# 阶段 1-2 第一批实现检查清单

## 1. 目的

本清单面向“立刻开始编码”的场景，帮助快速进入实施状态。

使用方式：

- 每完成一项就打勾
- 每一项尽量对应一个小 PR 或一个清晰的提交主题

> 状态更新时间：2026-05-24（基于当前仓库代码与测试结果核实）

## 2. 阶段 1：立即可做清单

### benchmark 骨架

- [x] 新建 `benches/common.rs`（当前实现为 `benches/common/mod.rs`）
- [x] 提取统一 `BenchCase`
- [x] 固定 `BASE_SEED`
- [x] 提供 `smoke` case 列表
- [x] 提供 `full` case 列表

### benchmark operation 覆盖

- [x] `encode`
- [x] `verify`
- [x] `reconstruct`
- [x] `reconstruct_data`

### benchmark 元信息输出

- [x] 输出 `JSON`
- [x] 输出 `CSV`
- [x] 记录 `git revision`
- [x] 记录 `features`
- [x] 记录 `target triple`
- [x] 记录 `backend`
- [x] 记录 `seed`

### correctness baseline

- [x] 新建 `tests/golden_vectors.rs`
- [x] 固定递增序列输入
- [x] 固定 seeded 随机输入
- [x] 固定全零输入
- [x] 固定重复模式输入
- [x] 校验 `encode` hash
- [x] 校验 `reconstruct_data`
- [x] 校验 `reconstruct`
- [x] 校验 `verify`

## 3. 阶段 2：立即可做清单

### options 层

- [x] 新建 `CodecOptions`
- [x] 实现 `Default`
- [x] 新增 `ReedSolomon::with_options`
- [x] 让 `ReedSolomon::new` 委托给 `with_options`

### cache 配置

- [x] 增加 `inversion_cache`
- [x] 增加 `inversion_cache_capacity`
- [x] 关闭 cache 路径测试
- [x] capacity 非法值测试

### fast one parity

- [x] encode XOR 快路径
- [x] verify XOR 快路径
- [x] 与通用路径一致性测试

### split / join

- [x] 新增 `split`
- [x] 新增 `join`
- [x] padding 测试
- [x] 边界长度测试

### reconstruct_some

- [x] 定义 API
- [x] required 长度校验
- [x] 单 data shard 恢复测试
- [x] 多 data shard 中只恢复部分测试
- [x] data + parity 混合缺失测试
- [x] required 标记已存在 shard 测试

## 4. 第一批推荐提交顺序

建议顺序：

1. benchmark skeleton
2. golden vectors
3. `CodecOptions`
4. cache 开关
5. `split` / `join`
6. `fast_one_parity`
7. `reconstruct_some`

## 5. 第一批完成定义

当以下条件全部满足时，视为阶段 1-2 第一批可收口：

- [x] benchmark smoke 能跑
- [x] golden vector 测试稳定
- [x] `CodecOptions` 已接入
- [x] `fast_one_parity` 已可用
- [x] `split` / `join` 已可用
- [x] `reconstruct_some` 已有第一版
