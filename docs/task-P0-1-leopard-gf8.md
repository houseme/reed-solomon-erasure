# P0-1: Leopard GF8 完整编解码 — 子任务详细文档

> **状态: ✅ 已完成 (2026-06-01)** — 11/11 子任务全部完成
> 文档日期: 2026-05-31
> 预估总工作量: 3-4 周
> 前置依赖: 无

---

## 概述

将 Leopard GF8 从 prototype (仅编码引擎存在) 升级为完整支持编码、重建、验证的正式 codec。

## 文件影响范围

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/core/encode.rs` | 修改 | 移除 leopard guard，添加 leopard dispatch |
| `src/core/reconstruct.rs` | 修改 | 添加 leopard 重建 dispatch |
| `src/core/verify.rs` | 修改 | 添加 leopard 验证 dispatch |
| `src/core/leopard_gf8/mod.rs` | 修改 | 导出 decode 模块 |
| `src/core/leopard_gf8/decode.rs` | **新建** | Forney 重建算法 |
| `src/core/leopard.rs` | 修改 | FamilyState 辅助函数 |
| `src/errors.rs` | 修改 | 错误类型清理 |
| `src/core/options.rs` | 修改 | CodecFamily 文档 |
| `src/tests/mod.rs` | 修改 | 添加 leopard 测试 |

---

## P0-1a: 接入 Leopard GF8 编码到公共 API

### P0-1a-1: 移除 encode_sep 的 leopard guard

**目标**: 让 `encode_sep` 在 LeopardGF8 模式下不再返回 `UnsupportedLeopardPrototype`

**文件**: `src/core/encode.rs`

**当前代码** (line 350-372):
```rust
pub fn encode_sep(
    &self,
    data: &[impl AsRef<[u8]>],
    parity: &mut [impl AsMut<[u8]>],
) -> Result<(), Error> {
    // ... validation ...
    if leopard::leopard_gf8_state(&self.family_state).is_ok() {
        return Err(Error::UnsupportedLeopardPrototype);  // ← 移除此行
    }
    // ... classic encode path ...
}
```

**修改**: 将 guard 替换为 leopard dispatch 分支

**验收标准**: `encode_sep` 在 LeopardGF8 模式下不再返回 `UnsupportedLeopardPrototype`

**预估**: 0.5 天

### P0-1a-2: 实现 leopard encode dispatch

**目标**: 在 `encode_sep` 中调用已有的 `leopard_gf8::encode_with_tables()`

**文件**: `src/core/encode.rs`

**实现**:
```rust
pub fn encode_sep(
    &self,
    data: &[impl AsRef<[u8]>],
    parity: &mut [impl AsMut<[u8]>],
) -> Result<(), Error> {
    // ... validation ...

    // Leopard GF8 路径
    if let FamilyState::LeopardGF8(ref leopard_state) = self.family_state {
        let tables = leopard_gf8::init_leopard_gf8_tables();
        let data_refs: Vec<&[u8]> = data.iter().map(|d| d.as_ref()).collect();
        let mut parity_refs: Vec<&mut [u8]> = parity.iter_mut().map(|p| p.as_mut()).collect();
        leopard_gf8::encode_with_tables(
            &data_refs,
            &mut parity_refs,
            self.data_shard_count,
            self.parity_shard_count,
            leopard_state,
            &tables,
        );
        return Ok(());
    }

    // Classic 路径
    // ... existing code ...
}
```

**需要验证**:
- `leopard_gf8::encode_with_tables` 的函数签名是否匹配
- 是否需要额外的生命周期处理

**验收标准**: `encode_sep` 在 LeopardGF8 模式下成功执行 FFT 编码

**预估**: 1 天

### P0-1a-3: 编码 roundtrip 测试

**目标**: 验证 Leopard GF8 编码的正确性

**文件**: `src/tests/mod.rs`

**测试用例**:
```rust
#[test]
fn test_leopard_gf8_encode_roundtrip() {
    let rs = ReedSolomon::with_options(
        10, 4,
        CodecOptions { codec_family: CodecFamily::LeopardGF8, ..Default::default() },
    ).unwrap();

    let shard_size = 1024;
    let mut shards = alloc_test_shards(14, shard_size);
    fill_random(&mut shards);

    let (data, mut parity) = split_data_parity(&mut shards, 10);
    rs.encode_sep(&data, &mut parity).unwrap();

    // 验证: 重新编码应产生相同结果
    let mut parity2 = alloc_parity(4, shard_size);
    rs.encode_sep(&data, &mut parity2).unwrap();
    assert_eq!(parity_bytes(&parity), parity_bytes(&parity2));
}

#[test]
fn test_leopard_gf8_encode_various_sizes() {
    // 测试不同分片大小: 64B, 1KB, 64KB, 1MB
    for size in [64, 1024, 65536, 1048576] {
        // ... encode and verify consistency
    }
}

#[test]
fn test_leopard_gf8_encode_single_shard() {
    // 测试最小配置: 1 data + 1 parity
    let rs = ReedSolomon::with_options(1, 1, leopard_opts()).unwrap();
    // ...
}
```

**验收标准**: 所有测试通过

**预估**: 1 天

---

## P0-1b: 实现 Leopard GF8 重建

### P0-1b-1: Forney 算法核心

**目标**: 实现基于 Forney 算法的 Leopard GF8 分片重建

**新建文件**: `src/core/leopard_gf8/decode.rs`

**算法概述**:

Leopard 使用 FFT-based 编码。重建过程:
1. 标识缺失分片位置
2. 对已有的 data shards 做 FFT
3. 使用 Forney 算法在频域中插值恢复缺失分片
4. IFFT 将结果转回时域

**详细算法**:

```
输入:
  - shards: Vec<Option<&[u8]>>  (None 表示缺失)
  - shard_size: usize
  - data_shard_count: usize (N)
  - parity_shard_count: usize (P)

步骤:
1. 计算缺失位置集合 erasure_set = {i | shards[i].is_none()}
2. 如果 |erasure_set| > P, 返回 Err(TooFewShardsPresent)

3. 对每个 chunk (按 chunk_size 分块):
   a. 准备工作缓冲区 work[0..N+P]，每个大小 shard_size
   b. 将已有 shards 复制到 work 对应位置
   c. 对缺失位置填零

   d. FFT(work, N+P):
      - 使用与编码相同的 fft_dit4 蝶形运算
      - fft_skew 扭转因子

   e. Forney 插值:
      - 构建 erasure locator 多项式 σ(x)
      - 对每个缺失位置 j:
        - 计算修正值 using σ'(x) 和已知频域值
        - 恢复 work[j]

   f. IFFT(work, N+P):
      - 使用与编码相同的 ifft_dit4 蝶形运算

   g. 将恢复的 shards 写回输出

4. 返回 Ok(())
```

**关键复用**:
- `leopard_gf8::ops::fft_dit4_full_lut` — FFT 蝶形
- `leopard_gf8::ops::ifft_dit4_full_lut` — IFFT 蝶形
- `leopard_gf8::ops::lut_xor` — SIMD GF 乘法
- `leopard_gf8::ops::slice_xor` — SIMD XOR
- `leopard_gf8::work::FlatWork` — 工作缓冲区
- `leopard_gf8::mod::LeopardGf8Tables` — FFT 表

**Go 参考**: `klauspost/reedsolomon` 的 `leopard8.go` 中的 `reconstruct()` 函数

**结构**:
```rust
/// Leopard GF8 解码/重建驱动
pub(crate) struct LeopardGf8DecodeDriver {
    pub data_shard_count: usize,
    pub parity_shard_count: usize,
    pub shard_size: usize,
    pub chunk_size: usize,
    pub work_slices: usize,
    pub m: usize,           // next_power_of_2(parity_shards)
    pub mtrunc: usize,      // min(data_shards, m)
}

/// 执行 Leopard GF8 重建
pub(crate) fn reconstruct_with_tables(
    shards: &mut [Option<&mut [u8]>],
    data_shard_count: usize,
    parity_shard_count: usize,
    tables: &LeopardGf8Tables,
) -> Result<(), Error> {
    // 1. 检查缺失数
    let erasures: Vec<usize> = shards.iter().enumerate()
        .filter_map(|(i, s)| if s.is_none() { Some(i) } else { None })
        .collect();
    if erasures.len() > parity_shard_count {
        return Err(Error::TooFewShardsPresent);
    }
    if erasures.is_empty() {
        return Ok(()); // 无缺失
    }

    // 2. 构建解码驱动
    let driver = build_decode_driver(data_shard_count, parity_shard_count, shard_size);

    // 3. 分块处理
    for chunk_offset in (0..shard_size).step_by(driver.chunk_size) {
        let chunk_len = driver.chunk_size.min(shard_size - chunk_offset);
        reconstruct_chunk(shards, chunk_offset, chunk_len, &driver, tables)?;
    }

    Ok(())
}
```

**验收标准**:
- 能正确重建 1 个缺失分片
- 能正确重建 parity_shard_count 个缺失分片
- 缺失超过 parity_shard_count 时返回错误
- 重建结果与原始数据一致

**预估**: 1 周

### P0-1b-2: reconstruct 入口集成

**目标**: 在 `ReedSolomon::reconstruct()` 中添加 leopard dispatch

**文件**: `src/core/reconstruct.rs`

**当前代码** (line 422-425):
```rust
pub fn reconstruct<T: AsMut<[u8]>>(
    &self,
    slices: &mut [Option<T>],
) -> Result<(), Error> {
    self.ensure_classic_family_execution()?;  // ← 移除
    // ...
}
```

**修改**:
```rust
pub fn reconstruct<T: AsMut<[u8]>>(
    &self,
    slices: &mut [Option<T>],
) -> Result<(), Error> {
    // Leopard GF8 路径
    if let FamilyState::LeopardGF8(_) = self.family_state {
        let tables = leopard_gf8::init_leopard_gf8_tables();
        let mut refs: Vec<Option<&mut [u8]>> = slices.iter_mut()
            .map(|s| s.as_mut().map(|b| b.as_mut()))
            .collect();
        return leopard_gf8::decode::reconstruct_with_tables(
            &mut refs,
            self.data_shard_count,
            self.parity_shard_count,
            &tables,
        );
    }

    // Classic 路径
    self.ensure_classic_family_execution()?;
    // ... existing code ...
}
```

**验收标准**: `reconstruct` 在 LeopardGF8 模式下执行重建

**预估**: 2 天

### P0-1b-3: reconstruct_data 实现

**目标**: 实现仅重建 data shards (跳过 parity)

**文件**: `src/core/reconstruct.rs`

**实现**: 在 leopard 路径中，重建后仅保留 data shards 的结果，parity shards 保持 None

```rust
pub fn reconstruct_data<T: AsMut<[u8]>>(
    &self,
    slices: &mut [Option<T>],
) -> Result<(), Error> {
    if let FamilyState::LeopardGF8(_) = self.family_state {
        // 调用完整重建
        self.reconstruct(slices)?;
        // 将 parity 位置恢复为 None
        for i in self.data_shard_count..self.total_shard_count() {
            slices[i] = None;
        }
        return Ok(());
    }
    // ...
}
```

**注意**: 这是一个简化实现。更高效的方案是仅计算 data shards 的 Forney 插值，跳过 parity 的恢复。但作为初始实现，完整重建后截断是可接受的。

**验收标准**: `reconstruct_data` 仅恢复 data shards

**预估**: 1 天

### P0-1b-4: 重建测试矩阵

**目标**: 全面测试 Leopard GF8 重建

**文件**: `src/tests/mod.rs`

**测试用例**:

```rust
#[test]
fn test_leopard_gf8_reconstruct_single_data_missing() {
    // 缺失 1 个 data shard
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    shards[0] = None; // 缺失 data[0]
    rs.reconstruct(&mut shards).unwrap();
    // 验证 data[0] 被正确重建
}

#[test]
fn test_leopard_gf8_reconstruct_single_parity_missing() {
    // 缺失 1 个 parity shard
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    shards[10] = None; // 缺失 parity[0]
    rs.reconstruct(&mut shards).unwrap();
}

#[test]
fn test_leopard_gf8_reconstruct_max_erasures() {
    // 缺失恰好 parity_shard_count 个
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    shards[0] = None;
    shards[3] = None;
    shards[7] = None;
    shards[11] = None;
    rs.reconstruct(&mut shards).unwrap();
}

#[test]
fn test_leopard_gf8_reconstruct_too_many_erasures() {
    // 缺失超过 parity_shard_count → 错误
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    shards[0] = None;
    shards[1] = None;
    shards[2] = None;
    shards[3] = None;
    shards[4] = None; // 5 > 4
    assert!(rs.reconstruct(&mut shards).is_err());
}

#[test]
fn test_leopard_gf8_reconstruct_all_data_missing() {
    // 所有 data shards 缺失
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    for i in 0..10 { shards[i] = None; }
    rs.reconstruct(&mut shards).unwrap();
}

#[test]
fn test_leopard_gf8_reconstruct_no_erasures() {
    // 无缺失 → 无操作
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    rs.reconstruct(&mut shards).unwrap();
}

#[test]
fn test_leopard_gf8_reconstruct_various_shard_sizes() {
    for size in [64, 256, 1024, 65536] {
        // ...
    }
}
```

**验收标准**: 所有测试通过

**预估**: 2 天

---

## P0-1c: 实现 Leopard GF8 验证

### P0-1c-1: verify leopard dispatch

**目标**: 在 `verify` 中添加 leopard 路径

**文件**: `src/core/verify.rs`

**当前代码** (line 51-52):
```rust
pub fn verify(&self, slices: &[impl AsRef<[u8]>]) -> Result<bool, Error> {
    self.ensure_classic_family_execution()?;  // ← 移除
    // ...
}
```

**修改**:
```rust
pub fn verify(&self, slices: &[impl AsRef<[u8]>]) -> Result<bool, Error> {
    // Leopard GF8 路径
    if let FamilyState::LeopardGF8(_) = self.family_state {
        let tables = leopard_gf8::init_leopard_gf8_tables();
        let data: Vec<&[u8]> = slices[..self.data_shard_count].iter()
            .map(|s| s.as_ref()).collect();
        let existing_parity: Vec<&[u8]> = slices[self.data_shard_count..].iter()
            .map(|s| s.as_ref()).collect();

        // 重新编码生成期望的 parity
        let mut expected_parity: Vec<Vec<u8>> = (0..self.parity_shard_count)
            .map(|_| vec![0u8; data[0].len()])
            .collect();
        let mut parity_refs: Vec<&mut [u8]> = expected_parity.iter_mut()
            .map(|p| p.as_mut_slice()).collect();

        leopard_gf8::encode_with_tables(
            &data, &mut parity_refs,
            self.data_shard_count, self.parity_shard_count,
            leopard_state, &tables,
        );

        // 比较
        for (existing, expected) in existing_parity.iter().zip(expected_parity.iter()) {
            if existing != expected.as_slice() {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    // Classic 路径
    // ...
}
```

**预估**: 1 天

### P0-1c-2: verify 测试

**测试用例**:
```rust
#[test]
fn test_leopard_gf8_verify_valid() {
    let rs = leopard_rs(10, 4);
    let shards = encode_random(&rs, 10, 4, 1024);
    assert!(rs.verify(&shards).unwrap());
}

#[test]
fn test_leopard_gf8_verify_corrupted_parity() {
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    shards[10][0] ^= 0xFF; // 篡改 1 字节
    assert!(!rs.verify(&shards).unwrap());
}

#[test]
fn test_leopard_gf8_verify_corrupted_data() {
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    shards[0][0] ^= 0xFF;
    assert!(!rs.verify(&shards).unwrap());
}
```

**预估**: 0.5 天

---

## P0-1d: 实现 reconstruct_some

### P0-1d-1: selective 重建逻辑

**目标**: 在 `reconstruct_some` 中添加 leopard 路径

**文件**: `src/core/reconstruct.rs`

**实现策略**: 调用完整重建，然后清除不需要的分片

```rust
pub fn reconstruct_some<T: AsMut<[u8]>>(
    &self,
    shards: &mut [Option<T>],
    required: &[bool],
) -> Result<(), Error> {
    if let FamilyState::LeopardGF8(_) = self.family_state {
        // 先完整重建
        self.reconstruct(shards)?;
        // 清除不需要的分片
        for (i, req) in required.iter().enumerate() {
            if !req && i < self.total_shard_count() {
                shards[i] = None;
            }
        }
        return Ok(());
    }
    // ...
}
```

**预估**: 1 天

### P0-1d-2: 测试

```rust
#[test]
fn test_leopard_gf8_reconstruct_some() {
    let rs = leopard_rs(10, 4);
    let mut shards = encode_random(&rs, 10, 4, 1024);
    shards[0] = None;
    shards[10] = None;

    let required = vec![true, true, false, false, false,
                        false, false, false, false, false,
                        false, false, false, false];
    rs.reconstruct_some(&mut shards, &required).unwrap();

    // data[0] 应被重建, data[1] 也应被重建 (因为是完整重建的一部分)
    assert!(shards[0].is_some());
}
```

**预估**: 0.5 天

---

## P0-1e: 移除 prototype 标记

### P0-1e-1: 错误类型清理

**目标**: 将 `UnsupportedLeopardPrototype` 仅用于 LeopardGF16

**文件**: `src/errors.rs`, `src/core/mod.rs`

**修改**: 更新 `ensure_classic_family_execution()` 使其仅对 LeopardGF16 返回错误:
```rust
pub(crate) fn ensure_classic_family_execution(&self) -> Result<(), Error> {
    match self.family_state {
        FamilyState::Classic | FamilyState::LeopardGF8(_) => Ok(()),
        FamilyState::LeopardGF16 => Err(Error::UnsupportedLeopardPrototype),
    }
}
```

**预估**: 0.5 天

### P0-1e-2: 文档更新

**文件**: `src/core/options.rs`, `README.md`, `README_CN.md`

**修改**: 更新 `CodecFamily::LeopardGF8` 文档，移除 "prototype" 描述，添加完整功能说明。

**预估**: 0.5 天

---

## 依赖关系

```
P0-1a-1 → P0-1a-2 → P0-1a-3
                    ↓
P0-1a-2 → P0-1b-1 → P0-1b-2 → P0-1b-3 → P0-1b-4
                    ↓
P0-1a-2 → P0-1c-1 → P0-1c-2
                    ↓
P0-1b-2 → P0-1d-1 → P0-1d-2
                    ↓
P0-1b-4 + P0-1c-2 + P0-1d-2 → P0-1e-1 → P0-1e-2
```

**关键路径**: P0-1a-2 → P0-1b-1 → P0-1b-2 → P0-1b-4 → P0-1e
