# P2-1: Leopard GF16 实现方案

> 文档日期: 2026-06-02
> 基于 GF8 实现 (src/core/leopard_gf8/) 的结构化移植

---

## 概述

实现 GF(2^16) 域上的 Leopard FFT 编解码，支持高达 65,536 个分片。算法与 GF8 同构，但域元素从 `u8` 变为 `u16`。

## 关键设计决策

1. **无 Mul16Lut 结构体**: GF8 的 `Mul8Lut` 使用 SIMD nibble 查找表 (`_mm256_shuffle_epi8`)。GF16 无法使用此技巧——16 位元素不适用。改用 log 域直接乘法: `gf16_mul(a,b) = exp_lut[(log_lut[a] + log_lut[b]) % 65535]`。

2. **u16 切片重解释**: 公共 API 仍接受 `&[u8]` 切片。内部，GF16 代码通过 `unsafe` 指针转换重解释为 `&[u16]`（shard_size 验证为 2 的倍数，64 字节对齐检查已保证）。

3. **FlatWork16**: 与 GF8 相同的平坦分配模式，但使用 `u16` 元素。避免 `Vec<Vec<u16>>` 开销。

4. **线程本地缓存**: 与 GF8 相同的模式——在 thread-local 中缓存 FlatWork16 和 scratch 缓冲区。

5. **Plan 构建器**: 结构上与 GF8 相同——`build_fft_dit16_plan` 和 `build_ifft_dit16_plan` 使用相同算法，但 skew 值为 `u16`。

---

## 新增模块结构

```
src/core/leopard_gf16/
├── mod.rs      — 常量、表结构体、初始化、驱动器、plan 构建器
├── tables.rs   — build_tables16(): init_luts16, init_fft_skew16, init_mul16_lut
├── ops.rs      — gf16 算术、fwht16、fft/ifft 蝶形运算、slice_xor_u16
├── encode.rs   — encode_with_tables16
├── decode.rs   — reconstruct_with_tables16 (Forney)
├── work.rs     — FlatWork16 (u16 版本)
└── tests.rs    — 单元测试
```

## 关键常量

```rust
const BITWIDTH16: usize = 16;
const ORDER16: usize = 1 << 16;       // 65536
const MODULUS16: usize = ORDER16 - 1; // 65535
const POLYNOMIAL16: usize = 0x1100B;  // x^16 + x^12 + x^3 + x + 1
const WORK_SIZE16: usize = 32 << 10;  // 处理块大小
```

## 表结构

```rust
pub(crate) struct LeopardGf16Tables {
    pub(crate) log_lut: Box<[u16; ORDER16]>,       // 128 KB
    pub(crate) exp_lut: Box<[u16; ORDER16 * 2]>,   // 256 KB (wraparound)
    pub(crate) fft_skew: Box<[u16; MODULUS16]>,    // 128 KB
    pub(crate) log_walsh: Box<[u16; ORDER16]>,     // 128 KB
}
```

总内存: ~640 KB — 可接受

---

## 实现步骤

### Step 1: 模块骨架 + 表 (`leopard_gf16/mod.rs`, `tables.rs`, `ops.rs`)

**`mod.rs`**: 常量、模块声明、静态 TABLES16、init 函数、plan 构建器、驱动器。

**`tables.rs`**: `build_tables16()`:
- `init_luts16()`: 使用本原多项式 0x1100B 构建 log/exp LUT
- `init_fft_skew16()`: 构建 FFT skew 因子 (移植 `init_fft_skew8`)
- `init_log_walsh16()`: log_lut 的 FWHT

**`ops.rs`**: GF16 算术、FWHT、FFT/IFFT 蝶形运算:
- `gf16_mul(a, b, tables)` — log 域乘法
- `mulgf16(out, input, log_m, tables)` — 切片乘以 log 域系数
- `fwht16`, `fwht16_mtrunc`, `fwht16_variable` — Walsh-Hadamard 变换
- `fft_dit2_16`, `ifft_dit2_16` — radix-2 蝶形
- `fft_dit4_16`, `ifft_dit4_16` — radix-4 蝶形
- `slice_xor_u16` — u16 切片 XOR

### Step 2: 编码 (`leopard_gf16/encode.rs`)

移植 GF8 的 `encode_with_tables`:
- `LeopardGf16EncodeDriver` 结构体
- `build_leopard_gf16_encode_driver()`
- `encode_with_tables16()` — 主编码入口
- 线程本地 FlatWork16 和 scratch 缓存

### Step 3: 解码 (`leopard_gf16/decode.rs`, `leopard_gf16/work.rs`)

移植 GF8 的 `reconstruct_with_tables`:
- `FlatWork16` 结构体 (u16 版 FlatWork)
- `LeopardGf16DecodeDriver`
- `reconstruct_with_tables16()` — 主解码入口
- `compute_error_locs16()` — 基于 FWHT 的错误定位
- `compute_formal_derivative16()`

### Step 4: 集成分发

**`src/core/leopard.rs`**:
- 添加 `LeopardGF16Codec<F>` 结构体
- 更新 `build_family_state` 支持 GF16
- 更新 `validate_leopard_family` 支持 GF16 (total_shards <= 65536)
- 添加 `leopard_gf16_encode()` 分发
- 添加 `leopard_gf16_reconstruct()` 分发
- 更新 `ensure_classic_family_execution()`

**`src/core/mod.rs`**: 添加 `pub(crate) mod leopard_gf16;`

**`src/core/encode.rs`**: 添加 `encode_leopard_gf16_sep` — 当 `FamilyState::LeopardGF16` 时分发到 GF16 引擎。

**`src/core/reconstruct.rs`**: 添加 `reconstruct_leopard_gf16` — 分发到 GF16 引擎。

**`src/core/verify.rs`**: 添加 `verify_leopard_gf16` — 重新编码 + 比较。

### Step 5: 测试

**`leopard_gf16/tests.rs`**:
- 表形状验证
- Log/exp LUT 往返测试
- FFT/IFFT 往返测试
- 基本编码 (10+4 配置)
- 编码验证往返
- 重建单丢失
- 重建多丢失 (data + parity)
- 最大擦除 (parity_shards 个丢失)
- 小配置 (1+1)

### Step 6: 文档

- 更新 README 添加 LeopardGF16 章节
- 更新 CodecFamily doc comments
- 更新 task-master-index.md

---

## 修改文件列表

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/core/leopard_gf16/mod.rs` | **新建** | 模块根、常量、表、plans、驱动器 |
| `src/core/leopard_gf16/tables.rs` | **新建** | build_tables16, log/exp/fft_skew/log_walsh |
| `src/core/leopard_gf16/ops.rs` | **新建** | GF16 算术、FWHT、FFT/IFFT、切片操作 |
| `src/core/leopard_gf16/encode.rs` | **新建** | encode_with_tables16 |
| `src/core/leopard_gf16/decode.rs` | **新建** | reconstruct_with_tables16 (Forney) |
| `src/core/leopard_gf16/work.rs` | **新建** | FlatWork16 (u16 工作缓冲区) |
| `src/core/leopard_gf16/tests.rs` | **新建** | 单元测试 |
| `src/core/leopard.rs` | 修改 | 添加 LeopardGF16Codec、分发 |
| `src/core/mod.rs` | 修改 | 添加 `pub(crate) mod leopard_gf16` |
| `src/core/encode.rs` | 修改 | 添加 GF16 分发 |
| `src/core/reconstruct.rs` | 修改 | 添加 GF16 分发 |
| `src/core/verify.rs` | 修改 | 添加 GF16 分发 |
| `README.md` | 修改 | 添加 LeopardGF16 章节 |
| `docs/task-master-index.md` | 修改 | 更新 P2-1 状态 |

---

## 验证

1. `cargo check` — 所有新代码编译通过
2. `cargo test --lib` — 所有现有 238 个测试仍然通过
3. 新 GF16 测试通过 (表、FFT 往返、编码、重建)
4. `cargo test --features "simd-accel"` — SIMD 路径仍然工作
5. 测试各种配置的 `CodecFamily::LeopardGF16` (1+1, 10+4, 100+20)
