# reed-solomon-erasure

[English](README.md) | 中文

[![CI](https://github.com/houseme/reed-solomon-erasure/actions/workflows/ci.yml/badge.svg)](https://github.com/houseme/reed-solomon-erasure/actions/workflows/ci.yml)
[![Crates](https://img.shields.io/crates/v/reed-solomon-erasure.svg)](https://crates.io/crates/reed-solomon-erasure)
[![Documentation](https://docs.rs/reed-solomon-erasure/badge.svg)](https://docs.rs/reed-solomon-erasure)
[![dependency status](https://deps.rs/repo/github/houseme/reed-solomon-erasure/status.svg)](https://deps.rs/repo/github/houseme/reed-solomon-erasure)

Reed-Solomon 纠删码的 Rust 实现

> **项目来源：** 版本 0.9.0–6.0.0 由 [Darren Ldl](https://github.com/darrenldl) 创建（2017–2021），并由 [rust-rse](https://github.com/rust-rse) 社区维护（2021–2022）。版本 >6.0.0 由 [houseme](https://github.com/houseme) 开发（2026–至今），基于 Rust 2024 edition 重写，包含运行时 SIMD 分发、Leopard-GF8 编解码器以及 NEON/x86 SIMD 后端。

CI 已迁移至统一的 GitHub Actions 工作流（`.github/workflows/ci.yml`），包含测试、构建、安全检查、拼写检查和基于标签的发布阶段。

WASM 构建同样可用，详见下方 **WASM 用法** 章节。

本项目移植自 [BackBlaze 的 Java 实现](https://github.com/Backblaze/JavaReedSolomon)、[Klaus Post 的 Go 实现](https://github.com/klauspost/reedsolomon) 和 [Nicolas Trangez 的 Haskell 实现](https://github.com/NicolasT/reedsolomon)。

版本 `1.X.X` 移植自 BackBlaze 的实现，性能较低，因为可添加并行性的位置较少。

版本 `>= 2.0.0` 移植自 Klaus Post 的实现。SIMD C 代码移植自 Nicolas Trangez 的实现，并做了少量修改。

详见 [注意事项](#注意事项) 和 [许可证](#许可证) 章节。

## WASM 用法

详见 [wasm/README.md](wasm/README.md)（英文）。

## Rust 用法

在 `Cargo.toml` 中添加以下依赖：

```toml
[dependencies]
reed-solomon-erasure = "6.0"
```

或启用 SIMD 加速（推荐用于性能敏感场景）：

```toml
[dependencies]
reed-solomon-erasure = { version = "6.0", features = ["simd-accel"] }
```

> **注意：** `simd-accel` 特性现在优先使用 Rust 运行时分发的 SIMD 后端（x86_64 上的 SSSE3、AVX2、AVX512、GFNI；aarch64 上的 NEON）。捆绑的 `simd_c` 实现保留作为旧版回退。
>
> 运行时可通过 `RSE_BACKEND_OVERRIDE` 环境变量强制指定后端，或在构建时通过 `RUST_REED_SOLOMON_ERASURE_ARCH` 配置旧版 C 后端的 `-march` 参数。

## 示例

```rust
use reed_solomon_erasure::galois_8::ReedSolomon;
use reed_solomon_erasure::VerifyWorkspace;
// 或使用 Galois 2^16 后端
// use reed_solomon_erasure::galois_16::ReedSolomon;

fn main() {
    let r = ReedSolomon::new(3, 2).unwrap(); // 3 个数据分片，2 个校验分片

    let mut master_copy = vec![
        vec![0, 1,  2,  3],
        vec![4, 5,  6,  7],
        vec![8, 9, 10, 11],
        vec![0, 0,  0,  0], // 最后 2 行是校验分片
        vec![0, 0,  0,  0],
    ];

    // 构建校验分片
    r.encode(&mut master_copy).unwrap();

    // 复制并转换为 Option 分片排列，用于 reconstruct_shards
    let mut shards: Vec<_> = master_copy.iter().cloned().map(Some).collect();

    // 最多可移除 2 个分片（可以是数据分片或校验分片）
    shards[0] = None;
    shards[4] = None;

    // 尝试重建丢失的分片
    r.reconstruct(&mut shards).unwrap();

    // 转换回普通分片排列
    let result: Vec<_> = shards.into_iter().filter_map(|x| x).collect();

    let mut verify_workspace = VerifyWorkspace::new(&r, master_copy[0].len());
    assert!(r.verify_with_workspace(&result, &mut verify_workspace).unwrap());
    assert_eq!(master_copy, result);
}
```

对于重复的验证调用，推荐使用 `verify_with_workspace` 或 `verify_with_buffer`，而非普通的 `verify`，以便在多次调用间复用校验分片的临时缓冲区。

对于 `galois_8` 上的 SIMD 敏感工作负载，可以使用 `reed_solomon_erasure::galois_8::alloc_aligned_shards(...)` 或 `galois_8::ReedSolomon::alloc_aligned(...)` 分配 64 字节对齐的分片。这些辅助函数返回实现了 `AsRef<[u8]>` 和 `AsMut<[u8]>` 的 `AlignedShard` 缓冲区，可直接传递给现有的编码/验证 API，无需更改编解码器输出语义。

## 编解码器族

`CodecOptions::codec_family` 显式指定算法族：

- `CodecFamily::Classic`
  - 默认族
  - 保持当前经典行为和兼容性假设
- `CodecFamily::LeopardGF8`
  - 保留用于显式高分片数的 Leopard GF(2^8) 族
  - 现在可以作为显式内部族原型构建
  - 暴露设置元数据，如 `leopard_setup_matrix_shape()`
  - `encode(...)`、`encode_sep(...)` 和 `encode_opt(...)` 现在连接到显式的族特定原型路径
  - `verify`、`reconstruct`、`update` 和 `decode_idx` 仍返回 `Error::UnsupportedLeopardPrototype`
- `CodecFamily::LeopardGF16`
  - 保留用于未来的 Leopard GF(2^16) 族
  - 目前仅作为原型骨架暴露，返回 `Error::UnsupportedLeopardPrototype`

这意味着经典用户不会被静默切换到其他族。任何未来的 Leopard 使用都将是显式选择加入的。

## 矩阵模式

`CodecOptions::matrix_mode` 现在支持多种矩阵族：

- `MatrixMode::Vandermonde`
  - 默认模式
  - 保持经典输出行为
- `MatrixMode::Cauchy`
  - 替代编码矩阵
  - 输出与默认模式不兼容
- `MatrixMode::JerasureLike`
  - 受 Jerasure 风格布局启发的替代编码矩阵
  - 输出与默认模式不兼容
- `MatrixMode::Custom`
  - 需要通过 `ReedSolomon::with_custom_matrix(...)` 显式指定校验行
  - `with_options(... MatrixMode::Custom ...)` 不带矩阵载荷时返回 `Error::InvalidCustomMatrix`

如果需要与现有经典 Reed-Solomon 载荷或 MinIO 导向的经典输出期望兼容，请使用 `MatrixMode::Vandermonde`。

`with_custom_matrix(...)` 最简示例：

```rust
use reed_solomon_erasure::{CodecOptions, galois_8::ReedSolomon};

// 3 个数据分片和 2 个校验分片的自定义校验行示例。
// 这些行定义了生成矩阵的校验部分。
let custom_rows = vec![vec![1u8, 1, 1], vec![1u8, 2, 4]];

let custom = ReedSolomon::with_custom_matrix(3, 2, &custom_rows, CodecOptions::default()).unwrap();

let mut shards = vec![
    vec![1u8, 2, 3, 4],
    vec![5u8, 6, 7, 8],
    vec![9u8, 10, 11, 12],
    vec![0u8; 4],
    vec![0u8; 4],
];
custom.encode(&mut shards).unwrap();
assert!(custom.verify(&shards).unwrap());
```

## 渐进式解码

对于多步恢复流程，`decode_idx(...)` 允许在输入分片到达时逐步累积重建贡献，而不需要一次性调用 `reconstruct(...)`：

```rust
use reed_solomon_erasure::galois_8::ReedSolomon;

let rs = ReedSolomon::new(5, 3).unwrap();

let mut shards = vec![
    vec![1u8, 2, 3, 4],
    vec![5u8, 6, 7, 8],
    vec![9u8, 10, 11, 12],
    vec![13u8, 14, 15, 16],
    vec![17u8, 18, 19, 20],
    vec![0u8; 4],
    vec![0u8; 4],
    vec![0u8; 4],
];
rs.encode(&mut shards).unwrap();

// 增量重建分片 1 和 4。
let mut dst = vec![None; 8];
dst[1] = Some(vec![0u8; 4]);
dst[4] = Some(vec![0u8; 4]);

// 标记为 `true` 的位置预期在多次调用中作为输入到达。
let expect_input = vec![true, false, true, true, false, true, true, false];

let mut first_input = vec![None; 8];
first_input[0] = Some(shards[0].clone());
first_input[2] = Some(shards[2].clone());
rs.decode_idx(&mut dst, Some(&expect_input), &first_input).unwrap();

let mut second_input = vec![None; 8];
second_input[3] = Some(shards[3].clone());
second_input[5] = Some(shards[5].clone());
second_input[6] = Some(shards[6].clone());
rs.decode_idx(&mut dst, Some(&expect_input), &second_input).unwrap();

assert_eq!(dst[1].as_deref(), Some(shards[1].as_slice()));
assert_eq!(dst[4].as_deref(), Some(shards[4].as_slice()));
```

`decode_idx(...)` 还支持合并模式：传入 `expect_input = None`，将另一个部分解码结果 XOR 累积到目标缓冲区中。

## 自行测试性能

您可以使用标准 `cargo bench` 命令快速测试不同配置下的性能（例如数据/校验分片比率、并行参数）。

## 性能

版本 1.X.X 和 2.0.0 不使用 SIMD。版本 2.1.0 起使用 Nicolas 的 C 代码进行 SIMD 操作。

版本 >= 4.0.0 已经过大幅架构重构，支持运行时 SIMD 分发。详细的基准测试方法和结果请参阅 [`docs/benchmark-methodology.md`](docs/benchmark-methodology.md)。

### 参考基准测试

| 平台 | CPU | 后端 | 编码 10x2x1M | 重建 10x2x1M |
|---|---|---|---|---|
| x86_64 | AMD EPYC 9V45 | AVX2 | ~12 GB/s | ~10 GB/s |
| aarch64 | Apple M5 Max | NEON | ~10 GB/s | ~8 GB/s |

> 以上为冒烟基准测试的近似数据。实际性能取决于分片数量、分片大小和 CPU。请运行 `cargo bench` 获取您硬件上的精确数据。

## 基准测试

```bash
# 运行所有基准测试
cargo bench

# 启用 SIMD 加速运行基准测试
cargo bench --features simd-accel

# 运行基准冒烟测试（快速配置）
VALIDATION_PROFILE=fast cargo test --test benchmark_smoke

# 运行扩展冒烟测试
VALIDATION_PROFILE=extended cargo test --test benchmark_smoke
```

详见 [`docs/benchmark-methodology.md`](docs/benchmark-methodology.md)（英文）了解完整的方法论、配置和结果解读。

## 更新日志

[更新日志](CHANGELOG.md)

## 贡献

欢迎贡献。提交贡献即表示您同意将您的作品按照本项目 LICENSE 文件中声明的相同许可证进行授权。

## 致谢

#### 2026 年重大重写

感谢 [houseme](https://github.com/houseme) 对库的重大重写：

- 迁移至 Rust 2024 edition（rust-version 1.95）
- 运行时分发的 GF(2^8) 后端架构，包含 SIMD 后端（SSSE3、AVX2、AVX512、GFNI、NEON）
- Leopard-GF8 编解码器族实现
- 全面的基准测试、CI 和发布自动化基础设施
- 详尽的文档和设计文档

#### 库重构和 Galois 2^16 后端

感谢以下人员对库的重构和引入 Galois 2^16 后端的贡献：

  - [@drskalman](https://github.com/drskalman)

  - Jeff Burdges [@burdges](https://github.com/burdges)

  - Robert Habermeier [@rphmeier](https://github.com/rphmeier)

#### WASM 构建

感谢 Nazar Mokrynskyi [@nazar-pc](https://github.com/nazar-pc) 提交的 WASM 构建包。

他是 `wasm` 文件夹中文件的原始作者。文件后续可能已被修改。

#### AVX512 支持

感谢 [@sakridge](https://github.com/sakridge) 添加 AVX512 支持（见 [PR #69](https://github.com/darrenldl/reed-solomon-erasure/pull/69)）

#### build.rs 改进

感谢 [@ryoqun](https://github.com/ryoqun) 在交叉编译场景下改进库的可用性（见 [PR #75](https://github.com/darrenldl/reed-solomon-erasure/pull/75)）

#### no_std 支持

感谢 Nazar Mokrynskyi [@nazar-pc](https://github.com/nazar-pc) 添加 `no_std` 支持（见 [PR #90](https://github.com/darrenldl/reed-solomon-erasure/pull/90)）

#### 测试人员

感谢以下人员在各种平台上进行测试和基准测试：

  - Laurențiu Nicola [@lnicola](https://github.com/lnicola/)（平台：Linux、Intel）

  - Roger Andersen [@hexjelly](https://github.com/hexjelly)（平台：Windows、AMD）

## 注意事项

#### 代码质量审查

如果您想评估本库的代码质量，可以参考审计注释。

搜索 "AUDIT" 即可查看面向代码审查的开发笔记。

#### 实现说明

`1.X.X` 实现主要移植自 [BackBlaze 的 Java 实现](https://github.com/Backblaze/JavaReedSolomon)。

`2.0.0` 起主要移植自 [Klaus Post 的 Go 实现](https://github.com/klauspost/reedsolomon)，C 文件移植自 [Nicolas Trangez 的 Haskell 实现](https://github.com/NicolasT/reedsolomon)。

`>= 7.0.0` 引入了运行时分发的 GF(2^8) 后端架构，包含 Rust SIMD 实现（x86_64 上的 SSSE3、AVX2、AVX512、GFNI；aarch64 上的 NEON）、Leopard-GF8 编解码器族、多种矩阵模式（Vandermonde、Cauchy、JerasureLike、Custom）以及通过 `decode_idx` 实现的渐进式解码。

所有版本的测试套件均以 [Klaus Post 的 Go 实现](https://github.com/klauspost/reedsolomon) 为基础。

## 许可证

#### Nicolas Trangez 的 Haskell Reed-Solomon 实现

用于 SIMD 操作的 C 文件（无/少量修改）复制自 [Nicolas Trangez 的 Haskell 实现](https://github.com/NicolasT/reedsolomon)，遵循与 NicolasT 项目相同的 MIT 许可证。

#### 总结

所有文件均在 MIT 许可证下发布。
