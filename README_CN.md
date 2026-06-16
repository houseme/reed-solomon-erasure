# rustfs-erasure-codec

[![CI](https://github.com/houseme/reed-solomon-erasure/actions/workflows/ci.yml/badge.svg)](https://github.com/houseme/reed-solomon-erasure/actions/workflows/ci.yml)
[![Crates](https://img.shields.io/crates/v/rustfs-erasure-codec.svg)](https://crates.io/crates/rustfs-erasure-codec)
[![Documentation](https://docs.rs/rustfs-erasure-codec/badge.svg)](https://docs.rs/rustfs-erasure-codec)
[![dependency status](https://deps.rs/repo/github/houseme/reed-solomon-erasure/status.svg)](https://deps.rs/repo/github/houseme/reed-solomon-erasure)
[![Crates.io Total Downloads](https://img.shields.io/crates/d/rustfs-erasure-codec)](https://crates.io/crates/rustfs-erasure-codec)
[![Crates.io License](https://img.shields.io/crates/l/rustfs-erasure-codec)](https://crates.io/crates/rustfs-erasure-codec)
[![Crates.io Version](https://img.shields.io/crates/v/rustfs-erasure-codec)](https://crates.io/crates/rustfs-erasure-codec)

[English](README.md) | 中文

Reed-Solomon 纠删码的 Rust 实现。

当前仓库中的主线代码已经对齐到较新的 Rust 2024 重构版本，核心能力包括：

- 经典 `GF(2^8)` 与 `GF(2^16)` Reed-Solomon 编解码
- 面向 `galois_8` 的运行时 SIMD 后端分发
- 多种经典矩阵模式
- 面向高分片场景的 Leopard 编解码器族
- 面向热点路径的低分配验证与恢复辅助 API
- 流式接口、基准测试与发布校验脚本

WASM 绑定见 [wasm/README.md](wasm/README.md)。

## 亮点

- `galois_8::ReedSolomon` 是当前最主要、优化最完整的执行路径。
- `galois_16::ReedSolomon` 仍可用于经典 `GF(2^16)` 场景。
- `CodecOptions` 可统一配置编解码器族、矩阵模式、缓存策略和并行策略。
- `VerifyWorkspace` 与 `ShardSlot<T>` 可降低重复调用时的分配成本。
- `decode_idx(...)` 与 `reconstruct_some(...)` 适合经典族上的渐进式恢复或定向恢复。
- `stream::StreamOptions` 支持大文件按块编码、校验和恢复。

## 安装

添加 crate：

```toml
[dependencies]
rustfs-erasure-codec = "7.0.0"
```

如果关注吞吐，建议开启 SIMD：

```toml
[dependencies]
rustfs-erasure-codec = { version = "7.0.0", features = ["simd-accel"] }
```

也可以只开启目标平台需要的后端：

```toml
[dependencies]
rustfs-erasure-codec = { version = "7.0.0", features = ["simd-neon"] }   # aarch64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-ssse3"] } # x86_64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-avx2"] }  # x86_64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-avx512"] }# x86_64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-gfni"] }  # x86_64
# rustfs-erasure-codec = { version = "7.0.0", features = ["simd-vsx"] }   # powerpc64
```

说明：

- `simd-accel` 是总开关，会启用所有 Rust/C SIMD 后端。
- 运行时会自动探测 ISA，不支持时会安全回退到标量路径。
- 默认启用 `std`；如需 `no_std`，请关闭默认特性。

## 快速开始

```rust
use rustfs_erasure_codec::galois_8::ReedSolomon;
use rustfs_erasure_codec::VerifyWorkspace;

fn main() {
    let rs = ReedSolomon::new(3, 2).unwrap();

    let mut shards = vec![
        vec![0, 1, 2, 3],
        vec![4, 5, 6, 7],
        vec![8, 9, 10, 11],
        vec![0, 0, 0, 0],
        vec![0, 0, 0, 0],
    ];

    rs.encode(&mut shards).unwrap();

    let original = shards.clone();
    let mut missing: Vec<Option<Vec<u8>>> = shards.into_iter().map(Some).collect();
    missing[0] = None;
    missing[4] = None;

    rs.reconstruct(&mut missing).unwrap();

    let rebuilt: Vec<Vec<u8>> = missing.into_iter().map(|shard| shard.unwrap()).collect();
    let mut verify_workspace = VerifyWorkspace::new(&rs, rebuilt[0].len());

    assert!(rs.verify_with_workspace(&rebuilt, &mut verify_workspace).unwrap());
    assert_eq!(rebuilt, original);
}
```

如果 `verify(...)` 需要高频调用，优先使用 `verify_with_workspace(...)`
或 `verify_with_buffer(...)` 来复用校验分片临时缓冲区。

## 低分配辅助接口

对于重复恢复场景，`ShardSlot<T>` 可以避免每次丢片后重新分配缓冲区：

```rust
use rustfs_erasure_codec::galois_8::{ReedSolomon, mark_missing_slots, shards_to_slots};

fn main() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let mut shards = vec![
        vec![0, 1, 2, 3],
        vec![4, 5, 6, 7],
        vec![8, 9, 10, 11],
        vec![12, 13, 14, 15],
        vec![0, 0, 0, 0],
        vec![0, 0, 0, 0],
    ];

    rs.encode(&mut shards).unwrap();

    let mut slots = shards_to_slots(&shards);
    mark_missing_slots(&mut slots, &[1, 5]);
    rs.reconstruct(&mut slots).unwrap();

    assert!(slots[1].is_present());
    assert!(slots[5].is_present());
}
```

对 `galois_8` 的 SIMD 敏感负载，还可以使用下列接口分配 64 字节对齐分片：

- `rustfs_erasure_codec::galois_8::alloc_aligned_shards(...)`
- `galois_8::ReedSolomon::alloc_aligned(...)`

## 编解码器族

`CodecOptions::codec_family` 用于选择算法族：

| 编解码器族         | 状态                   | 说明                                                                                                                 |
|---------------|----------------------|--------------------------------------------------------------------------------------------------------------------|
| `Classic`     | 完整支持                 | 默认族。支持 `update`、`encode_single`、`decode_idx`、`reconstruct_some` 以及矩阵模式切换。                                          |
| `LeopardGF8`  | 适用于 `galois_8` 高分片场景 | 基于 FFT 的 `GF(2^8)` Leopard 路径。要求分片长度是 64 字节的整数倍，总分片数不超过 256。`update`、`encode_single*`、`decode_idx` 等经典族专属 API 不支持。 |
| `LeopardGF16` | 适用于更高总分片数场景          | 基于 FFT 的 Leopard 族。`update`、`encode_single*`、`decode_idx` 等经典族专属 API 不支持。                                          |

示例：

```rust
use rustfs_erasure_codec::galois_8::ReedSolomon;
use rustfs_erasure_codec::{CodecFamily, CodecOptions};

let rs = ReedSolomon::with_options(
32,
16,
CodecOptions {
codec_family: CodecFamily::LeopardGF8,
..CodecOptions::default ()
},
).unwrap();
```

## 矩阵模式

`CodecOptions::matrix_mode` 只对 `CodecFamily::Classic` 生效：

- `Vandermonde`：默认经典行为
- `Cauchy`：替代编码矩阵
- `JerasureLike`：Jerasure 风格矩阵布局
- `Custom`：通过 `ReedSolomon::with_custom_matrix(...)` 显式提供矩阵行

如果你需要兼容已有经典载荷格式，建议继续使用 `MatrixMode::Vandermonde`。

最小自定义矩阵示例：

```rust
use rustfs_erasure_codec::galois_8::ReedSolomon;
use rustfs_erasure_codec::CodecOptions;

let custom_rows = vec![vec![1u8, 1, 1], vec![1u8, 2, 4]];
let rs = ReedSolomon::with_custom_matrix(3, 2, & custom_rows, CodecOptions::default ()).unwrap();
```

## 进阶 API

### 渐进式恢复

`decode_idx(...)` 仅适用于经典 `galois_8::ReedSolomon`，适合输入分片分批到达、无法一次性执行完整 `reconstruct(...)` 的场景。

### 定向恢复

`reconstruct_some(...)` 只恢复你标记为必需的分片，适合不需要还原整条 stripe 的场景。

### 流式处理

流式接口位于 `rustfs_erasure_codec::stream`，默认 `std` 特性下可用：

- `encode_stream(...)`
- `verify_stream(...)`
- `reconstruct_stream(...)`

当数据量太大、不适合整组分片常驻内存时，优先使用它。`StreamOptions`
默认块大小是 4 MiB，可按吞吐和内存占用自行调节。

## 运行时后端控制

`galois_8` 主路径支持运行时后端查看与强制覆盖。

环境变量：

- `RSE_BACKEND_OVERRIDE`：强制指定后端，例如 `scalar`、`rust-neon`、`rust-avx2`、`rust-avx512`、`rust-gfni-avx2`、
  `rust-gfni-avx512`、`rust-ssse3`、`rust-vsx`、`simd-c`
- `RSE_STRICT_BACKEND_OVERRIDE=1`：若请求后端未生效，则让校验失败
- `RUST_REED_SOLOMON_ERASURE_ARCH`：调整 legacy C 后端的构建目标

公开辅助函数：

- `galois_8::active_backend_name()`
- `galois_8::active_backend_kind()`
- `galois_8::active_backend_id()`

## 调优选项

`CodecOptions` 暴露了几个常用调优项：

- `fast_one_parity`：当 `parity_shards == 1` 时启用 XOR 快路径
- `inversion_cache`：开启或关闭解码矩阵缓存
- `inversion_cache_capacity`：显式控制缓存容量
- `max_parallel_jobs`：限制单个 codec 实例的并行度

并行策略也可通过环境变量控制：

- `RS_PARALLEL_POLICY_MIN_PARALLEL_SHARD_BYTES`
- `RS_PARALLEL_POLICY_MIN_BYTES_PER_JOB`
- `RS_PARALLEL_POLICY_MAX_JOBS`

## 基准测试与校验

常见工作流：

```bash
# 运行测试
cargo test

# 运行基准
cargo bench --features simd-accel

# 执行发布校验流程
bash scripts/release-check.sh

# 执行扩展校验流程
VALIDATION_PROFILE=extended bash scripts/release-check.sh

# 采集 x86_64 SIMD 基准产物
bash scripts/collect_x86_simd_benchmarks.sh
```

推荐同时参考：

- [docs/benchmark-methodology.md](docs/benchmark-methodology.md)
- [docs/README-performance-index.md](docs/README-performance-index.md)
- [scripts/README.md](scripts/README.md)
- [docs/README.md](docs/README.md)

## 项目来源

版本 `0.9.0` 到 `6.0.0` 最初由
[Darren Ldl](https://github.com/darrenldl) 创建，并由
[rust-rse](https://github.com/rust-rse) 社区继续维护。当前仓库中的主线代码则包含
[houseme](https://github.com/houseme) 维护的 Rust 2024 重构版本，以及新的 SIMD /
Leopard 相关工作。

## 贡献

欢迎贡献。对于后端相关、性能敏感或基准敏感的修改，建议附带聚焦的验证结果。

## 许可证

本项目采用 MIT License，详见 [LICENSE](LICENSE)。

仓库内打包的 `simd_c` 源码派生自
[Nicolas Trangez 的 Haskell 实现](https://github.com/NicolasT/reedsolomon)，同样遵循 MIT License。
