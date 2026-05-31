# 任务主索引

> 最后更新: 2026-05-31
> 基于 reed-solomon-erasure vs klauspost/reedsolomon 对比分析

---

## 任务总数统计

| 级别 | 主任务 | 子任务 | 可独立执行的叶子任务 |
|------|--------|--------|---------------------|
| P0 | 2 | 11 | 25 |
| P1 | 3 | 8 | 18 |
| P2 | 3 | 10 | 20 |
| P3 | 4 | 8 | 14 |
| **合计** | **12** | **37** | **77** |

---

## P0 — 关键功能对等性

### P0-1: Leopard GF8 完整编解码
> 文档: [task-P0-1-leopard-gf8.md](task-P0-1-leopard-gf8.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P0-1a: 接入编码到公共 API | P0-1a-1 移除 encode guard | 0.5d |
| | P0-1a-2 实现 leopard encode dispatch | 1d |
| | P0-1a-3 编码 roundtrip 测试 | 1d |
| P0-1b: 重建实现 | P0-1b-1 Forney 算法核心 | 1w |
| | P0-1b-2 reconstruct 入口集成 | 2d |
| | P0-1b-3 reconstruct_data 实现 | 1d |
| | P0-1b-4 重建测试矩阵 | 2d |
| P0-1c: 验证实现 | P0-1c-1 verify leopard dispatch | 1d |
| | P0-1c-2 verify 测试 | 0.5d |
| P0-1d: reconstruct_some | P0-1d-1 selective 重建逻辑 | 1d |
| | P0-1d-2 测试 | 0.5d |
| P0-1e: 移除 prototype | P0-1e-1 错误类型清理 | 0.5d |
| | P0-1e-2 文档更新 | 0.5d |

### P0-2: 流式 API
> 文档: [task-P0-2-streaming-api.md](task-P0-2-streaming-api.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P0-2a: API 设计 | P0-2a-1 StreamOptions 设计 | 0.5d |
| | P0-2a-2 StreamError 设计 | 0.5d |
| | P0-2a-3 API review | 1d |
| P0-2b: encode_stream | P0-2b-1 块读取逻辑 | 1d |
| | P0-2b-2 编码调用集成 | 1d |
| | P0-2b-3 parity 写入逻辑 | 1d |
| | P0-2b-4 短读/EOF 处理 | 1d |
| | P0-2b-5 测试 | 1d |
| P0-2c: reconstruct_stream | P0-2c-1 缺失分片检测 | 1d |
| | P0-2c-2 块级重建逻辑 | 2d |
| | P0-2c-3 测试 | 1d |
| P0-2d: verify_stream | P0-2d-1 块级验证逻辑 | 1d |
| | P0-2d-2 测试 | 0.5d |
| P0-2e: 并发流 | P0-2e-1 rayon 并发读取 | 1d |
| | P0-2e-2 rayon 并发写入 | 0.5d |
| | P0-2e-3 测试 | 0.5d |
| P0-2f: 文档 | P0-2f-1 README 示例 | 0.5d |
| | P0-2f-2 doc comments | 0.5d |

---

## P1 — 性能优化

### P1-1: ARM64 NEON XOR 优化
> 文档: [task-P1-1-arm64-xor.md](task-P1-1-arm64-xor.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P1-1a: c=1 快速路径 | P1-1a-1 xor_slice_neon 函数 | 1d |
| | P1-1a-2 集成到 mul_slice_xor | 0.5d |
| | P1-1a-3 正确性测试 | 0.5d |
| | P1-1a-4 性能基准测试 | 0.5d |
| P1-1b: c=0 快速路径 | P1-1b-1 实现 | 0.5d |
| P1-1c: const-generic 统一 | P1-1c-1 合并函数签名 | 1d |
| | P1-1c-2 调用方更新 | 0.5d |
| | P1-1c-3 回归测试 | 0.5d |
| P1-1d: scalar 快速路径 | P1-1d-1 scalar_mul_slice 优化 | 0.5d |
| | P1-1d-2 scalar_mul_slice_xor 优化 | 0.5d |

### P1-2: SIMD 生成式代码
> 文档: [task-P1-2-simd-codegen.md](task-P1-2-simd-codegen.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P1-2a: 收益评估 | P1-2a-1 配置分布调研 | 0.5d |
| | P1-2a-2 基准测试对比 | 1d |
| | P1-2a-3 评估报告 | 0.5d |
| P1-2b: build.rs 代码生成 | P1-2b-1 生成器框架 | 2d |
| | P1-2b-2 10x4 AVX2 生成 | 2d |
| | P1-2b-3 其他配置生成 | 1d |
| P1-2c: 集成 | P1-2c-1 encode dispatch | 1d |
| | P1-2c-2 测试 | 1d |

### P1-3: GFNI 后端修正
> 文档: [task-P1-3-gfni-fix.md](task-P1-3-gfni-fix.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P1-3a: 文档修正 | P1-3a-1 更新 doc comments | 0.5d |
| P1-3b: 性能验证 | P1-3b-1 基准测试设计 | 0.5d |
| | P1-3b-2 执行与记录 | 1d |
| | P1-3b-3 结果文档 | 0.5d |
| P1-3c: 策略决策 | P1-3c-1 分析与决策 | 1d |

---

## P2 — 功能扩展

### P2-1: Leopard GF16
> 文档: [task-P2-1-leopard-gf16.md](task-P2-1-leopard-gf16.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P2-1a: 表构建 | P2-1a-1 GF16 log/exp LUT | 1d |
| | P2-1a-2 GF16 fft_skew | 1d |
| | P2-1a-3 log_walsh | 0.5d |
| | P2-1a-4 表测试 | 0.5d |
| P2-1b: FFT/IFFT | P2-1b-1 fft_dit2_gf16 | 1d |
| | P2-1b-2 fft_dit4_gf16 | 2d |
| | P2-1b-3 ifft_dit4_gf16 | 1d |
| | P2-1b-4 FFT 测试 | 1d |
| P2-1c: 编码 | P2-1c-1 encode_with_tables_gf16 | 2d |
| | P2-1c-2 驱动参数 | 1d |
| | P2-1c-3 编码测试 | 1d |
| P2-1d: 解码 | P2-1d-1 Forney GF16 | 2d |
| | P2-1d-2 解码测试 | 1d |
| P2-1e: 集成 | P2-1e-1 API dispatch | 1d |
| | P2-1e-2 限制检查 | 0.5d |
| P2-1f: 测试文档 | P2-1f-1 完整测试矩阵 | 1d |
| | P2-1f-2 README 更新 | 0.5d |

### P2-2: ppc64le SIMD
> 文档: [task-P2-2-ppc64le.md](task-P2-2-ppc64le.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P2-2a: C SIMD 启用 | P2-2a-1 build.rs 修改 | 0.5d |
| | P2-2a-2 编译验证 | 0.5d |
| P2-2b: Rust VSX 后端 | P2-2b-1 nibble-lookup VSX | 3d |
| | P2-2b-2 mul_slice 实现 | 2d |
| | P2-2b-3 mul_slice_xor 实现 | 1d |
| P2-2c: 后端注册 | P2-2c-1 backend.rs dispatch | 1d |
| | P2-2c-2 自动选择逻辑 | 0.5d |
| P2-2d: 测试 | P2-2d-1 正确性测试 | 1d |
| | P2-2d-2 性能基准 | 1d |

### P2-3: 细粒度 SIMD Flags
> 文档: [task-P2-3-simd-flags.md](task-P2-3-simd-flags.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P2-3a: 方案设计 | P2-3a-1 flag 定义 | 0.5d |
| | P2-3a-2 兼容性分析 | 0.5d |
| P2-3b: 实现 | P2-3b-1 Cargo.toml 修改 | 0.5d |
| | P2-3b-2 cfg guards 添加 | 1d |
| | P2-3b-3 构建验证 | 0.5d |
| P2-3c: 测试文档 | P2-3c-1 组合测试 | 0.5d |
| | P2-3c-2 README 更新 | 0.5d |

---

## P3 — 开发体验

### P3-1: Builder 模式与 max_threads
> 文档: [task-P3-1-builder.md](task-P3-1-builder.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P3-1a: Builder 方法 | P3-1a-1 实现 builder 方法 | 0.5d |
| | P3-1a-2 测试 | 0.5d |
| P3-1b: max_parallel_jobs | P3-1b-1 字段添加 | 0.5d |
| | P3-1b-2 policy 集成 | 0.5d |
| | P3-1b-3 测试 | 0.5d |
| P3-1c: 文档 | P3-1c-1 doc comments | 0.5d |

### P3-2: 自动并行度调优
> 文档: [task-P3-2-auto-parallel.md](task-P3-2-auto-parallel.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P3-2a: 缓存感知 | P3-2a-1 算法设计 | 1d |
| | P3-2a-2 实现 | 1d |
| | P3-2a-3 测试 | 0.5d |
| P3-2b: 缓存检测 | P3-2b-1 Linux 检测 | 1d |
| | P3-2b-2 macOS 检测 | 0.5d |
| | P3-2b-3 回退默认值 | 0.5d |

### P3-3: Leopard GF8 文档
> 文档: [task-P3-3-leopard-docs.md](task-P3-3-leopard-docs.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P3-3a: API 文档 | P3-3a-1 CodecFamily 文档 | 0.5d |
| P3-3b: 运行时检查 | P3-3b-1 对齐检查 | 0.5d |
| | P3-3b-2 分片数检查 | 0.5d |
| P3-3c: README | P3-3c-1 使用示例 | 0.5d |
| | P3-3c-2 限制说明 | 0.5d |

### P3-4: 跨平台基准对比
> 文档: [task-P3-4-benchmarks.md](task-P3-4-benchmarks.md)

| 任务 | 叶子任务 | 预估 |
|------|----------|------|
| P3-4a: 配置定义 | P3-4a-1 配置矩阵 | 0.5d |
| P3-4b: Rust 基准 | P3-4b-1 Criterion 框架 | 1d |
| | P3-4b-2 encode 基准 | 0.5d |
| | P3-4b-3 reconstruct 基准 | 0.5d |
| P3-4c: Go 基准 | P3-4c-1 Go 基准代码 | 1d |
| P3-4d: 结果分析 | P3-4d-1 数据收集 | 0.5d |
| | P3-4d-2 报告撰写 | 0.5d |

---

## 依赖关系总览

```
P0-1a ─┬─→ P0-1b ──→ P0-1d ──→ P0-1e
       └─→ P0-1c ──→ P0-1e

P0-2a ─┬─→ P0-2b ──→ P0-2c
       ├─→ P0-2d
       └─→ P0-2e
              └─→ P0-2f

P1-1a ─┬─→ P1-1c
P1-1b ─┘
P1-1d (独立)

P1-2a → P1-2b → P1-2c

P1-3a (独立)
P1-3b → P1-3c

P0-1 完成后 → P2-1a → P2-1b → P2-1c → P2-1d → P2-1e → P2-1f
P2-2a (独立)
P2-2b → P2-2c → P2-2d
P2-3a → P2-3b → P2-3c

P3-1a + P3-1b (独立)
P3-2a → P3-2b
P3-3a + P3-3b + P3-3c (独立)
P3-4a → P3-4b + P3-4c → P3-4d
```

---

## 执行建议

1. **立即可启动 (无依赖)**: P0-1a, P0-2a, P1-1a, P1-1d, P1-3a, P2-2a, P2-3a, P3-1a, P3-3a
2. **关键路径**: P0-1a → P0-1b → P0-1d → P0-1e (Leopard GF8 功能链)
3. **可并行**: P0-1 和 P0-2 无依赖，可由两人并行开发
4. **P2-1 阻塞于 P0-1**: Leopard GF16 必须在 GF8 完成后开始
