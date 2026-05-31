# 自适应 Dit4 策略选择方案

> 日期：2026-05-30
> 状态：已确认阈值 64K

---

## 一、背景

基准测试发现不同 shard_size 下最优策略不同：

| shard_size | 4x2 encode 最优策略 | direct vs decomposed |
|-----------|-------------------|---------------------|
| 1K | decomposed (75.96 MB/s) | −48.1% |
| 4K | decomposed (79.04 MB/s) | −55.8% |
| 16K | direct-safe (82.99 MB/s) | −41.7% |
| 64K | direct-safe (82.62 MB/s) | −16.3% |
| 128K | direct (82.28 MB/s) | +0.1% |
| 256K+ | direct (~82 MB/s) | ~0% |

10x4 配置三策略差异 <3%，可忽略。核心优化点是 **4x2 + 小 shard_size** 场景。

---

## 二、方案设计

### 2.1 自适应逻辑

新增 `auto` 模式（默认），根据 shard_size 自动选择：

```
RSE_DIT4_STRATEGY=auto (默认)
  ├─ shard_size < 64K → decomposed (零 unsafe, 小文件最优)
  └─ shard_size >= 64K → direct (大文件单遍处理最优)
```

用户显式指定 (`direct`, `decomposed`, `direct-safe`) 始终优先。

### 2.2 实现方式

**两级缓存**:
1. **第一级 (全局缓存)**: env var 解析 → `OnceLock<Dit4Strategy>`，进程生命周期内只解析一次
2. **第二级 (每次调用)**: `active_dit4_strategy(shard_size)` 根据 shard_size 决定最终策略

**线程化 shard_size**: `shard_size` 参数从 `encode_with_tables` 传递到 `dit4_at`，经过：
```
encode_with_tables(shard_size 已知)
  → fft_dit8_with_plan(shard_size)
    → dit4_at(shard_size)
      → active_dit4_strategy(shard_size)
```

### 2.3 阈值依据

- **64K**: 4x2_64K 中 decomposed 仍比 direct 快 19%，4x2_128K 三策略趋同
- 10x4 配置所有尺寸三策略差异 <3%，阈值对其无影响

---

## 三、修改范围

仅修改 `src/core/leopard_gf8/encode.rs`:

1. `Dit4Strategy` 枚举添加 `Auto` 变体
2. `active_dit4_strategy()` 重构为两级：`configured_dit4_mode()` + `active_dit4_strategy(shard_size)`
3. `dit4_at()` 添加 `shard_size: usize` 参数
4. `fft_dit8_with_plan()` 添加 `shard_size: usize` 参数
5. `ifft_dit_encoder8_with_plan()` 添加 `shard_size: usize` 参数
6. 所有调用点传递 `driver.shard_size`

---

## 四、验证

1. `cargo build --features std` — 编译通过
2. `cargo test --lib --features std` — 199 测试通过
3. 小文件基准验证 auto 选择：
   - `RSE_DIT4_STRATEGY=auto` + 4x2_1K → 吞吐量应接近 decomposed (~76 MB/s)
   - `RSE_DIT4_STRATEGY=auto` + 4x2_1M → 吞吐量应接近 direct (~82 MB/s)
4. 大文件基准验证无回退
