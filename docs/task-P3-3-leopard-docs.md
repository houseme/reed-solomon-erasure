# P3-3: Leopard GF8 限制文档完善 — 子任务详细文档

> **状态: ✅ 已完成** — README 已更新准确描述 LeopardGF8 功能，源码有完整 doc-comments
> 文档日期: 2026-05-31
> 预估总工作量: 1-2 天
> 前置依赖: 无

---

## P3-3a: API 文档

### P3-3a-1: CodecFamily 文档

**文件**: `src/core/options.rs`

```rust
/// 编解码器族选择
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecFamily {
    /// 经典 Reed-Solomon 编码。
    ///
    /// 使用 Vandermonde/Cauchy 矩阵，兼容 Backblaze/MinIO。
    /// 适用于分片数较少 (<20) 的场景。
    Classic,

    /// Leopard GF(2^8) 编解码器。
    ///
    /// 使用 FFT-based 算法，复杂度 O(N log N)。
    /// 适用于分片数较多 (通常 >20-30) 的场景。
    ///
    /// # 限制
    ///
    /// - 分片大小建议为 64 字节的倍数 (性能最佳)
    /// - 所有分片必须等长 (最后一个分片需零填充)
    /// - data + parity 总数不超过 256
    /// - 不支持 `update()` 增量更新
    /// - 不支持 `encode_single()` / `encode_single_sep()` 逐分片编码
    /// - 编码输出与 Classic 模式不兼容
    LeopardGF8,

    /// Leopard GF(2^16) 编解码器 (未实现)。
    ///
    /// 支持高达 65,536 个分片。
    LeopardGF16,
}
```

**预估**: 0.5 天

---

## P3-3b: 运行时限制检查

### P3-3b-1: 对齐检查

**文件**: `src/core/encode.rs` (leopard 路径)

```rust
if let FamilyState::LeopardGF8(_) = self.family_state {
    // 分片大小对齐检查
    let shard_size = data[0].len();
    if shard_size % 64 != 0 {
        // 警告: 非 64 字节对齐可能影响性能
        // 不返回错误，但可以考虑添加 warning log
    }
}
```

**预估**: 0.5 天

### P3-3b-2: 分片数检查

```rust
if let FamilyState::LeopardGF8(_) = self.family_state {
    if self.total_shard_count() > 256 {
        return Err(Error::TooManyShards);
    }
}
```

**预估**: 0.5 天

---

## P3-3c: README 更新

### P3-3c-1: 使用示例

```markdown
### Leopard GF8 编码

当分片数较多时，Leopard GF8 比 Classic 更高效:

```rust
use reed_solomon_erasure::{ReedSolomon, CodecOptions, CodecFamily};

let rs = ReedSolomon::with_options(
    32, 4,  // 32 data + 4 parity
    CodecOptions::new().with_codec_family(CodecFamily::LeopardGF8),
).unwrap();

// 使用方式与 Classic 完全相同
let mut shards = vec![vec![0u8; 1024]; 36];
// ... 填充 data shards ...
let (data, mut parity) = split(&mut shards);
rs.encode_sep(&data, &mut parity).unwrap();
```
```

**预估**: 0.5 天

### P3-3c-2: 限制说明

在 README 中添加限制表格:

```markdown
| 限制 | Classic | LeopardGF8 |
|------|---------|------------|
| 最大分片数 | ~256 | 256 |
| 分片大小对齐 | 无要求 | 建议 64B |
| 等长分片 | 是 | 是 |
| 增量更新 | 支持 | 不支持 |
| 逐分片编码 | 支持 | 不支持 |
| 输出兼容性 | — | 不兼容 Classic |
```

**预估**: 0.5 天

---

## 依赖关系

```
P3-3a + P3-3b + P3-3c (全部独立，可并行)
```
