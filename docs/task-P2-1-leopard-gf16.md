# P2-1: Leopard GF16 完整实现 — 子任务详细文档

> **状态: ✅ 已完成 (2026-06-03)**
> 文档日期: 2026-05-31
> 预估总工作量: 3-4 周
> 前置依赖: P0-1 (Leopard GF8) 完成

---

## 概述

实现 GF(2^16) 域上的 Leopard FFT 编解码，支持高达 65,536 个分片。算法与 GF8 同构，但域元素从 `u8` 变为 `u16`。

## 文件影响范围

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/core/leopard_gf16/mod.rs` | **新建** | 模块定义、表结构 |
| `src/core/leopard_gf16/tables.rs` | **新建** | GF16 FFT 表 |
| `src/core/leopard_gf16/fft.rs` | **新建** | GF16 FFT/IFFT |
| `src/core/leopard_gf16/encode.rs` | **新建** | GF16 编码 |
| `src/core/leopard_gf16/decode.rs` | **新建** | GF16 解码/重建 |
| `src/core/leopard.rs` | 修改 | 添加 GF16 dispatch |
| `src/core/encode.rs` | 修改 | 添加 GF16 分支 |
| `src/core/reconstruct.rs` | 修改 | 添加 GF16 分支 |
| `src/core/verify.rs` | 修改 | 添加 GF16 分支 |

---

## P2-1a: 表构建

### P2-1a-1: GF16 log/exp LUT

**目标**: 构建 GF(2^16) 的 log 和 exp 查找表

**新建文件**: `src/core/leopard_gf16/tables.rs`

**GF(2^16) 域**: 元素为 `u16`，使用不可约多项式 (需选择一个标准多项式)

**表结构**:
```rust
pub(crate) struct LeopardGf16Tables {
    pub(crate) log_lut: Box<[u16; 65536]>,    // 128 KB
    pub(crate) exp_lut: Box<[u16; 131072]>,   // 256 KB (扩展以支持 wraparound)
    pub(crate) fft_skew: Box<[u16; 65535]>,   // 128 KB
    pub(crate) log_walsh: Box<[u16; 65536]>,  // 128 KB
}
```

**生成算法**:
```rust
fn init_luts16() -> (Box<[u16; 65536]>, Box<[u16; 131072]>) {
    // 选择 GF(2^16) 的本原多项式
    // 例如: x^16 + x^12 + x^3 + x + 1 (0x10089)
    const POLYNOMIAL: u32 = 0x10089;

    let mut log = vec![0u16; 65536];
    let mut exp = vec![0u16; 131072];

    let mut x = 1u32;
    for i in 0..65535 {
        log[x as usize] = i;
        exp[i as usize] = x as u16;
        x <<= 1;
        if x >= 65536 {
            x ^= POLYNOMIAL;
        }
    }
    exp[65535] = 1; // wraparound

    (Box::new(log.try_into().unwrap()), Box::new(exp.try_into().unwrap()))
}
```

**内存**: 总计约 512KB — 可接受

**预估**: 1 天

### P2-1a-2: GF16 fft_skew

**目标**: 构建 GF(2^16) 的 FFT 扭转因子

**算法**: 与 GF8 的 `init_fft_skew8` 同构，但使用 GF16 乘法

```rust
fn init_fft_skew16(log: &[u16; 65536], exp: &[u16; 131072]) -> Box<[u16; 65535]> {
    let mut skew = vec![0u16; 65535];
    // ... 与 GF8 相同的算法，但使用 u16 运算
    Box::new(skew.try_into().unwrap())
}
```

**预估**: 1 天

### P2-1a-3: log_walsh

**目标**: 构建 GF16 的 Walsh-Hadamard 变换表

```rust
fn init_log_walsh16(log: &[u16; 65536]) -> Box<[u16; 65536]> {
    let mut walsh = *log;
    fwht16(&mut walsh);
    Box::new(walsh)
}
```

**预估**: 0.5 天

### P2-1a-4: 表测试

```rust
#[test]
fn test_leopard_gf16_tables_shapes() {
    let tables = build_tables16();
    assert_eq!(tables.log_lut.len(), 65536);
    assert_eq!(tables.exp_lut.len(), 131072);
    assert_eq!(tables.fft_skew.len(), 65535);
    assert_eq!(tables.log_walsh.len(), 65536);
}
```

**预估**: 0.5 天

---

## P2-1b: FFT/IFFT

### P2-1b-1: fft_dit2_gf16

**目标**: 实现 GF16 的 radix-2 蝶形运算

**新建文件**: `src/core/leopard_gf16/fft.rs`

```rust
/// GF16 radix-2 DIT butterfly
#[inline]
unsafe fn fft_dit2_gf16(a: &mut u16, b: &mut u16, skew: u16, tables: &LeopardGf16Tables) {
    let t = gf16_mul_log(*b, skew, tables);
    *a ^= t;
    *b ^= t; // 注意: GF 加法 = XOR，对 u16 也是
}
```

**GF16 乘法**:
```rust
#[inline]
fn gf16_mul_log(a: u16, b: u16, tables: &LeopardGf16Tables) -> u16 {
    if a == 0 || b == 0 { return 0; }
    let log_a = tables.log_lut[a as usize] as u32;
    let log_b = tables.log_lut[b as usize] as u32;
    tables.exp_lut[(log_a + log_b) as usize % 65535]
}
```

**注意**: GF(2^16) 的加法仍是 XOR，但对 `u16` 类型需要按位 XOR

**预估**: 1 天

### P2-1b-2: fft_dit4_gf16

**目标**: 实现 GF16 的 radix-4 蝶形运算

```rust
unsafe fn fft_dit4_gf16(
    a: &mut u16, b: &mut u16, c: &mut u16, d: &mut u16,
    skew0: u16, skew1: u16, skew2: u16,
    tables: &LeopardGf16Tables,
) {
    fft_dit2_gf16(a, c, skew0, tables);
    fft_dit2_gf16(b, d, skew1, tables);
    fft_dit2_gf16(a, b, skew2, tables);
    fft_dit2_gf16(c, d, skew2, tables);
}
```

**预估**: 2 天 (含向量化优化评估)

### P2-1b-3: ifft_dit4_gf16

**目标**: 实现 GF16 的 IFFT

与 FFT 对称，使用不同的扭转因子

**预估**: 1 天

### P2-1b-4: FFT 测试

```rust
#[test]
fn test_gf16_fft_ifft_roundtrip() {
    // FFT → IFFT 应恢复原始数据
    let tables = build_tables16();
    let mut data: Vec<u16> = (0..256).collect();
    let original = data.clone();

    fft_gf16(&mut data, &tables);
    ifft_gf16(&mut data, &tables);

    assert_eq!(data, original);
}
```

**预估**: 1 天

---

## P2-1c: 编码

### P2-1c-1: encode_with_tables_gf16

**目标**: 实现 GF16 Leopard 编码

**新建文件**: `src/core/leopard_gf16/encode.rs`

**算法**: 与 GF8 编码同构:
1. 将 data shards 的每个 u16 元素打包到工作缓冲区
2. FFT
3. 截取前 parity_shards 个频域分量
4. IFFT
5. 写回 parity shards

**关键差异**: 分片大小必须为 64 字节倍数 (与 Go 一致)

**预估**: 2 天

### P2-1c-2: 驱动参数

```rust
pub(crate) struct LeopardGf16EncodeDriver {
    pub data_shard_count: usize,
    pub parity_shard_count: usize,
    pub shard_size: usize,
    pub chunk_size: usize,
    pub m: usize,
    pub mtrunc: usize,
}
```

**预估**: 1 天

### P2-1c-3: 编码测试

```rust
#[test]
fn test_leopard_gf16_encode_basic() {
    let rs = ReedSolomon::with_options(
        10, 4,
        CodecOptions { codec_family: CodecFamily::LeopardGF16, ..Default::default() },
    ).unwrap();
    // encode and verify
}
```

**预估**: 1 天

---

## P2-1d: 解码/重建

### P2-1d-1: Forney GF16

**目标**: 实现 GF16 的 Forney 算法重建

**新建文件**: `src/core/leopard_gf16/decode.rs`

**算法**: 与 P0-1b-1 同构，但在 GF(2^16) 上

**预估**: 2 天

### P2-1d-2: 解码测试

```rust
#[test]
fn test_leopard_gf16_reconstruct() {
    // encode, erase shards, reconstruct, verify
}
```

**预估**: 1 天

---

## P2-1e: 集成

### P2-1e-1: API dispatch

**修改文件**: `src/core/encode.rs`, `src/core/reconstruct.rs`, `src/core/verify.rs`

在各公共方法中添加 `FamilyState::LeopardGF16` 分支

**预估**: 1 天

### P2-1e-2: 限制检查

```rust
// 分片大小对齐检查
if shard_size % 64 != 0 {
    return Err(Error::IncorrectShardSize); // 或新的错误类型
}
```

**预估**: 0.5 天

---

## P2-1f: 测试与文档

### P2-1f-1: 完整测试矩阵

| 场景 | data | parity | shard_size | 缺失数 |
|------|------|--------|------------|--------|
| 最小 | 1 | 1 | 64B | 1 |
| 标准 | 10 | 4 | 1KB | 4 |
| 大分片 | 10 | 4 | 1MB | 2 |
| 高分片数 | 100 | 20 | 1KB | 20 |
| 边界 | 256 | 256 | 64B | 256 |

**预估**: 1 天

### P2-1f-2: README 更新

**预估**: 0.5 天

---

## 依赖关系

```
P0-1 完成 (前置)
P2-1a-1 + P2-1a-2 + P2-1a-3 → P2-1a-4
P2-1a-4 → P2-1b-1 → P2-1b-2 + P2-1b-3 → P2-1b-4
P2-1b-4 → P2-1c-1 → P2-1c-2 → P2-1c-3
P2-1c-3 → P2-1d-1 → P2-1d-2
P2-1d-2 → P2-1e-1 + P2-1e-2 → P2-1f-1 + P2-1f-2
```
