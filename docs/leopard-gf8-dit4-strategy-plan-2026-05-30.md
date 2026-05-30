# 三策略 Dit4 蝶形运算实现方案

> 日期: 2026-05-30
> 背景: unsafe 消除导致 20-31% 吞吐量下降，需要可配置的三策略实现以平衡安全性与性能

---

## 一、性能回归根因

上一轮将 `dit4_at` 从单次 `fft_dit4_full_lut` 调用分解为 4 次 pairwise `fft_dit2` 调用:

| 指标 | 上一轮 | 本轮 | 变化 |
|------|--------|------|------|
| 32x16 1M | 31.02 MB/s | 24.46 MB/s | −21.2% |
| 64x32 1M | 29.66 MB/s | 20.49 MB/s | −31.0% |
| 96x48 1M | 8.57 MB/s | 6.88 MB/s | −19.7% |
| 128x64 1M | 10.46 MB/s | 8.22 MB/s | −21.4% |

**根因**: 分解后每个字节位置被触摸 4 次（每个 dit2 调用一次），而 `fft_dit4_full_lut` 在单次遍历中处理所有 4 个 lane，每个字节位置只触摸 1 次。

---

## 二、三策略定义

| 策略 | 名称 | unsafe | 性能预期 | 说明 |
|------|------|--------|---------|------|
| A | `decomposed` | 无 | 基线 (~24 MB/s) | 4 次 pairwise `get_pair_mut` + `fft_dit2` |
| B | `direct` | 1 处 | **最优** (~31 MB/s) | 快速路径用 `fft_dit4_full_lut`（unsafe ptr），慢速路径用 safe 分解 |
| C | `direct-safe` | 无 | 中等 | 快速路径用 `split_at_mut` 链提取 4 引用 + `fft_dit4_full_lut`，慢速路径同 A |

**默认策略**: `direct` — 最大性能恢复。

---

## 三、配置机制

**环境变量**: `RSE_DIT4_STRATEGY`

| 值 | 策略 |
|----|------|
| `direct` (默认) | 方案 B — unsafe 快速路径 |
| `decomposed` | 方案 A — 纯 safe pairwise |
| `direct-safe` | 方案 C — safe 4-lane |

**缓存**: `std::sync::OnceLock<Dit4Strategy>` (feature = "std")

---

## 四、实现细节

### 4.1 策略枚举

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dit4Strategy {
    Decomposed,   // 纯 safe pairwise
    Direct,       // unsafe 快速路径
    DirectSafe,   // safe 4-lane
}

fn active_dit4_strategy() -> Dit4Strategy {
    // RSE_DIT4_STRATEGY env var, cached in OnceLock, default: Direct
}
```

### 4.2 策略 A: `decomposed` (当前实现)

```rust
fn dit4_at_decomposed(...) {
    for i in 0..dist {
        // 4 次 get_pair_mut + fft_dit2/ifft_dit2
        // 每个字节位置被触摸 4 次
    }
}
```

### 4.3 策略 B: `direct` (unsafe 快速路径)

```rust
fn dit4_at_direct(...) {
    for i in 0..dist {
        if d < work.len() {
            // 快速路径: unsafe ptr 直接访问 4 个 lane
            unsafe {
                let ptr = work.as_mut_ptr();
                fft_dit4_full_lut(
                    (*ptr.add(a)).as_mut(), (*ptr.add(b)).as_mut(),
                    (*ptr.add(c)).as_mut(), (*ptr.add(d)).as_mut(),
                    lut01, lut23, lut02,
                );
            }
        } else {
            // 慢速路径: safe pairwise 分解
            dit4_pairwise_one(...);
        }
    }
}
```

**SAFETY**: `a < b < c < d < work.len()`，所有索引互不相同，`ptr.add()` 在分配范围内。

### 4.4 策略 C: `direct-safe` (safe 4-lane)

```rust
fn dit4_at_direct_safe(...) {
    for i in 0..dist {
        if d < work.len() {
            // split_at_mut 链获取 4 个不重叠的 &mut [u8]
            let (left_bc, right_d) = work.split_at_mut(d);
            let (left_b, right_c) = left_bc.split_at_mut(c);
            let (left_a, right_b) = left_b.split_at_mut(b);
            fft_dit4_full_lut(
                left_a[a].as_mut(), right_b[0].as_mut(),
                right_c[0].as_mut(), right_d[0].as_mut(),
                lut01, lut23, lut02,
            );
        } else {
            dit4_pairwise_one(...);
        }
    }
}
```

**安全性**: `split_at_mut` 保证不重叠，无 `unsafe`。额外开销来自 3 次 `split_at_mut` 调用和索引计算。

### 4.5 共用辅助函数

```rust
fn dit4_pairwise_one(...) {
    // 从当前 dit4_at 提取的单次迭代逻辑
    // 供 direct 和 direct-safe 的慢速路径共用
}
```

---

## 五、核心函数对比

| 函数 | 字节触摸次数 | LUT 查找/字节 | unsafe | 函数调用/迭代 |
|------|-------------|--------------|--------|-------------|
| `fft_dit4_full_lut` | 1 次 | 4 次 | 0 (被调用侧) | 1 |
| 4x `fft_dit2` | 4 次 | 4 次 | 0 | 4 |
| `split_at_mut` + `fft_dit4_full_lut` | 1 次 | 4 次 | 0 | 1 + split 开销 |
| unsafe ptr + `fft_dit4_full_lut` | 1 次 | 4 次 | 1 | 1 |

---

## 六、修改文件

| 文件 | 变更 |
|------|------|
| `src/core/leopard_gf8/ops.rs` | 移除 2 处 `#[allow(dead_code)]` |
| `src/core/leopard_gf8/encode.rs` | 添加 `Dit4Strategy` 枚举、`active_dit4_strategy()`、3 个策略函数、`dit4_pairwise_one`；重写 `dit4_at` 为分发函数 |

---

## 七、验证结果

### 7.1 编译和测试

| 检查项 | 结果 |
|--------|------|
| `cargo build --features std` | ✅ 通过 |
| `cargo test --lib --features std` | ✅ 199 测试通过 |
| `cargo clippy --features std` | ✅ 无新增警告 |

### 7.2 三策略性能对比 (64x32_1m)

| 策略 | 吞吐量 (MB/s) | vs 上一轮重构 | vs decomposed |
|------|--------------|-------------|--------------|
| `decomposed` | 21.97 | −25.9% | baseline |
| `direct-safe` | 35.48 | **+19.6%** | +61.5% |
| `direct` (默认) | 35.57 | **+19.9%** | +61.9% |

### 7.3 完整基准测试 (direct 策略)

| 配置 | 上一轮重构 (MB/s) | unsafe 消除后 (MB/s) | direct 策略 (MB/s) | vs 上一轮 |
|------|------------------|--------------------|--------------------|----------|
| 32x16 1M | 31.02 | 24.46 | **37.39** | **+20.5%** |
| 32x16 4M | 31.04 | 24.47 | **37.69** | **+21.4%** |
| 64x32 64K | 29.61 | 20.32 | **32.03** | **+8.2%** |
| 64x32 1M | 29.66 | 20.49 | **32.75** | **+10.4%** |
| 64x32 4M | 29.25 | 20.06 | **32.63** | **+11.6%** |
| 96x48 1M | 8.57 | 6.88 | **13.24** | **+54.5%** |
| 96x48 4M | 8.82 | 7.01 | **13.47** | **+52.7%** |
| 128x64 1M | 10.46 | 8.22 | **15.77** | **+50.8%** |
| 128x64 4M | 10.74 | 8.39 | **16.08** | **+49.7%** |

**结论**: `direct` 策略不仅恢复了性能，还显著超越了上一轮重构的水平（平均 +30%）。`direct-safe` 与 `direct` 性能几乎相同（差距 <0.3%），说明 `split_at_mut` 链的开销可以忽略。`decomposed` 策略保持在 ~22 MB/s 水平。
