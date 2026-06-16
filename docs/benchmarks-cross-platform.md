# Cross-Platform Benchmark Report

> Date: 2026-06-01
> Library: rustfs-erasure-codec v7.0.0

---

## Benchmark Infrastructure

### Benchmark Targets

| Target | File | Description |
|--------|------|-------------|
| `bandwidth` | `benches/bandwidth.rs` | Encode/reconstruct throughput (GiB/s) |
| `throughput_matrix` | `benches/throughput_matrix.rs` | Encode throughput across config matrix |
| `smoke` | `tests/benchmark_smoke.rs` | CI-friendly smoke tests (fast profile) |

### Configuration Matrix (22 configs)

Defined in `benches/common/mod.rs`:

| Shard Size | Data×Parity Configs |
|------------|---------------------|
| 1 KiB | 10×4, 12×4, 8×3, 8×4, 6×3, 4×2 |
| 4 KiB | 10×4, 12×4, 8×3, 8×4, 6×3, 4×2 |
| 64 KiB | 10×4, 12×4, 8×3, 8×4, 6×3, 4×2 |
| 1 MiB | 10×4, 12×4, 8×3, 8×4, 6×3, 4×2 |

### How to Run

```bash
# Full benchmark suite
cargo bench

# Specific SIMD backend
cargo bench --features simd-avx2    # x86_64 AVX2
cargo bench --features simd-neon    # aarch64 NEON
cargo bench --features simd-gfni    # x86_64 GFNI (requires Intel Ice Lake+)

# Smoke tests (CI)
VALIDATION_PROFILE=fast cargo test --test benchmark_smoke

# Export results as JSON
RSE_WRITE_PROFILE_REPORT=1 cargo bench
```

---

## Platform Results

### aarch64 (Apple Silicon)

**Hardware**: Apple M5 Max MacBook Pro
**Backend**: `rust-neon` (auto-selected)
**Feature**: `simd-neon`
**Artifact source**: `benchmarks/small-file/2026-05-27-aarch64-apple-silicon-extended.{json,csv}`
**Profile**: `extended` (`iterations = 5`)

**Benchmark Environment**
- OS: `macOS 26.5.1`
- Kernel: `Darwin 25.5.0`
- Hostname: `zhi-mbp.local`
- Target triple: `aarch64-macos-unknown`
- Architecture: `aarch64`
- Features: `std|simd-accel`
- Backend kind: `RustSimd`
- Backend override: `auto` (`override_honored = true`)
- Benchmark metrics feature: disabled
- CPU / SoC: `Apple M5 Max`
- Logical CPU parallelism: `18`
- Reported CPU frequency: `4608 MHz`
- Memory: `128 GB total`, `74.26 GB used`, `121.22 GB available`
- Swap: `0 B`
- Root disk: `1.81 TB APFS`, `390.05 GB used`, `1.43 TB available`
- Rust toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Build profile of the system info collector: `debug`
- Collector git branch / commit: `main` / `c405470f7350e0bbc01a6e2c25ab03ac2789c648`
- Runtime parallelism: `18`
- Power / thermal state during benchmark: not recorded

| Config | Shard Size | Encode (GiB/s) | Reconstruct (GiB/s) |
|--------|------------|----------------|---------------------|
| 10×4 | 4 KiB | 3.30 | 3.02 |
| 10×4 | 64 KiB | 3.62 | 3.52 |
| 10×4 | 1 MiB | 4.38 | 4.34 |

Supplementary observations from the same artifact set:

- `verify` on `10x4_4k` reached `3135.4851 MB/s` (`3.06 GiB/s`)
- `verify` on `10x4_64k` reached `3275.6813 MB/s` (`3.20 GiB/s`)
- `verify` on `10x4_1m` reached `4029.8207 MB/s` (`3.94 GiB/s`)
- `reconstruct_data` on `10x4_1m` reached `4590.9467 MB/s` (`4.48 GiB/s`)

The aarch64 curve is encouraging: throughput climbs steadily as shard size increases, and the
`1 MiB` case shows `reconstruct` staying close to `encode`, which matches the current NEON path's
large-block behavior.

### x86_64 (AMD EPYC)

**Hardware**: AMD EPYC 9V45 (96-core)
**Backend**: `rust-avx2` (auto-selected in the archived cross-platform sample)
**Feature**: `simd-avx2`

**Benchmark Environment**
- CPU: AMD EPYC 9V45
- Logical core note: 96-core processor
- Target triple: `x86_64-linux-unknown`
- Features: `std|simd-accel`
- Expected default backend: `rust-avx2`
- Backend override: `auto` in the archived small-file sample
- Machine-level `lscpu` details: recorded in the x86 machine JSON and summary docs
- Rust toolchain version: not summarized in this document
- Memory size: not summarized in this document

**Host Baseline**

#### System Information

| Property | Value |
|----------|-------|
| OS | ubuntu |
| OS Version | Linux (Ubuntu 24.04) |
| Architecture | x86_64 |
| Hostname | rustfs-jumpbox |
| Kernel Version | Linux 6.17.0-1015-azure |

#### CPU Information

| Property | Value |
|----------|-------|
| Cores | 16 |
| Brand | AMD EPYC 9V45 96-Core Processor |
| Frequency | 4115 MHz |
| Usage | 0.0% |

#### Memory Information

| Property | Value |
|----------|-------|
| Total | 31.34 GB |
| Used | 2.75 GB (8.8%) |
| Available | 28.59 GB |
| Total Swap | 0 B |
| Used Swap | 0 B |

#### Disk Information

| Name | Mount Point | Type | Total | Used | Available | Usage | Removable |
|------|-------------|------|-------|------|-----------|-------|----------|
| /dev/root | / | ext4 | 28.02 GB | 17.99 GB | 10.03 GB | 64.2% | false |
| /dev/nvme0n1p16 | /boot | ext4 | 880.39 MB | 178.00 MB | 702.39 MB | 20.2% | false |
| /dev/nvme0n1p15 | /boot/efi | vfat | 104.33 MB | 6.10 MB | 98.22 MB | 5.8% | false |
| /dev/nvme0n2 | /data/rustfs | xfs | 499.76 GB | 35.49 GB | 464.26 GB | 7.1% | false |

#### Runtime Information

| Property | Value |
|----------|-------|
| Process ID | 2584271 |
| Memory Usage | 161.62 MB |
| CPU Usage | 0.00% |
| CPU Parallelism | 16 |

#### Build Information

| Property | Value |
|----------|-------|
| Version | 1.0.0-beta.8 |
| Build Time | 2026-06-15 02:47:58 +00:00 |
| Build Profile | release |
| Build OS | linux-x86_64 |
| Rust Version | rustc 1.96.0 (ac68faa20 2026-05-25) |
| Git Branch | main |
| Git Commit | 6508f88d3a5edb428a5d623f927ce384691f0cd4 |
| Git Tag |  |
| Git Status |  |

#### Configuration Information

| Property | Value |
|----------|-------|
| Server Address | :9000 |
| Console Enable | true |
| Console Address | :9001 |
| Region | us-east-1 |
| Access Key | r***n|11 |
| Secret Key | **** |
| OBS Endpoint | (not set) |
| TLS Path | (not set) |
| KMS Enabled | false |
| KMS Backend | local |
| Buffer Profile | GeneralPurpose |
| Workload Profile | (disabled) |
| FTPS | --- |
| FTPS > Build Feature | enabled |
| FTPS > Enabled (`RUSTFS_FTPS_ENABLE`) | false |
| FTPS > Address (`RUSTFS_FTPS_ADDRESS`) | 0.0.0.0:8022 |
| FTPS > TLS Enabled (`RUSTFS_FTPS_TLS_ENABLED`) | true |
| FTPS > Certs Dir (`RUSTFS_FTPS_CERTS_DIR`) | (not set) |
| FTPS > CA File (`RUSTFS_FTPS_CA_FILE`) | (not set) |
| FTPS > Passive Ports (`RUSTFS_FTPS_PASSIVE_PORTS`) | 40000-50000 |
| FTPS > External IP (`RUSTFS_FTPS_EXTERNAL_IP`) | (not set) |
| WebDAV | --- |
| WebDAV > Build Feature | enabled |
| WebDAV > Enabled (`RUSTFS_WEBDAV_ENABLE`) | false |
| WebDAV > Address (`RUSTFS_WEBDAV_ADDRESS`) | 0.0.0.0:8080 |
| WebDAV > TLS Enabled (`RUSTFS_WEBDAV_TLS_ENABLED`) | true |
| WebDAV > Certs Dir (`RUSTFS_WEBDAV_CERTS_DIR`) | (not set) |
| WebDAV > CA File (`RUSTFS_WEBDAV_CA_FILE`) | (not set) |
| WebDAV > Max Body Size (`RUSTFS_WEBDAV_MAX_BODY_SIZE`) | 5368709120 bytes |
| WebDAV > Request Timeout (`RUSTFS_WEBDAV_REQUEST_TIMEOUT`) | 300 seconds |

#### Build Features

| Property | Value |
|----------|-------|
| Enabled Features | 2/8 |

##### Feature Status

| Feature | Status | Description |
|---------|--------|-------------|
| metrics-gpu | ✗ | Metrics GPU support |
| ftps | ✓ | FTPS protocol support |
| swift | ✗ | Swift storage backend |
| webdav | ✓ | WebDAV protocol support |
| license | ✗ | License validation |
| io-scheduler-debug | ✗ | Enable debug information in I/O scheduler |
| manual-test-runners | ✗ | Enable manual test binaries |
| full | ✗ | All features enabled |

##### Default Features

| Feature | Note |
|---------|------|
| ftps | enabled by default |
| webdav | enabled by default |

##### Feature Dependencies

| Feature | Dependencies |
|---------|-------------|
| metrics-gpu | rustfs-obs/gpu |
| ftps | rustfs-protocols/ftps |
| swift | rustfs-protocols/swift |
| webdav | rustfs-protocols/webdav |
| license | (none) |
| io-scheduler-debug | (none) |
| manual-test-runners | (none) |
| full | metrics-gpu + ftps + swift + webdav |

| Config | Shard Size | Encode (GiB/s) | Reconstruct (GiB/s) |
|--------|------------|----------------|---------------------|
| 10×4 | 4 KiB | 3.56 | 3.33 |
| 10×4 | 64 KiB | 3.10 | 2.90 |
| 10×4 | 1 MiB | 3.13 | 2.23 |

Data sources:
- `benchmarks/small-file/2026-05-27-x86_64-linux-extended.csv` — archived `10x4` small-file auto results used for the cross-platform table
- `benchmarks/x86_64-simd/comprehensive-x86_64-benchmark.json` — broader x86_64 encode-only matrix for deeper drill-down

> **Note**: This table uses the archived auto-path sample from `2026-05-27`. For newer x86 backend-policy and host-specific validation context, see [GFNI results doc](benchmarks-gfni-results.md) and the newer x86_64 benchmark artifacts under `benchmarks/x86_64-simd/`.

Why the gap versus aarch64 is large:

- This is not a same-host comparison: the x86_64 numbers come from an Ubuntu 24.04 Azure VM (`rustfs-jumpbox`) with `16` visible CPUs, while the aarch64 numbers come from an Apple Silicon MacBook Pro host.
- The archived x86_64 sample used the conservative auto-selected `rust-avx2` path, while the aarch64 sample used `rust-neon`; the backend choice is part of the result, not just the ISA label.
- The CPU brand string says `AMD EPYC 9V45 96-Core Processor`, but this benchmark host only exposed `16` cores / parallelism to the runtime. That VM sizing difference alone can materially change Rayon scheduling, cache pressure, and sustained large-shard throughput.
- The gap grows with shard size instead of shrinking: versus the aarch64 artifact, x86_64 is about `+7.9% / +10.3%` at `10x4_4k`, then `-14.4% / -17.6%` at `10x4_64k`, and `-28.5% / -48.7%` at `10x4_1m` for `encode / reconstruct`.
- That pattern points to host and runtime-path differences more than a simple “x86 vs ARM” conclusion. In this report, the safest reading is: these are cross-host benchmark snapshots, not a controlled architecture-only shootout.

### x86_64 (Intel with GFNI)

**Status**: No data yet. Requires Intel Ice Lake (10th gen Xeon) or later.

Expected backends:
- `rust-gfni-avx512` (if AVX-512 + GFNI available)
- `rust-gfni-avx2` (if AVX2 + GFNI available)

---

## Methodology

### Metrics

- **Encode throughput**: `data_shards × shard_size / encode_time` (GiB/s)
- **Reconstruct throughput**: `data_shards × shard_size / reconstruct_time` (GiB/s)
- **Backend**: Auto-detected at runtime, reported in benchmark output

### Warm-up

- Criterion benchmarks: 3-second warm-up, 10 measurement iterations
- Smoke tests: No warm-up (CI validation only)

### Environment Variables

| Variable | Purpose |
|----------|---------|
| `RSE_BACKEND_OVERRIDE` | Force specific backend (`auto`, `scalar`, `rust-avx2`, etc.) |
| `RSE_WRITE_PROFILE_REPORT` | Export profiling data as JSON |
| `VALIDATION_PROFILE` | Smoke test profile (`fast`, `extended`) |
| `RS_PARALLEL_POLICY_MAX_JOBS` | Limit parallelism for reproducible results |

### Why Hardware Context Matters

The current report mixes data from different hosts and collection styles:

- Apple Silicon numbers come from small-file `extended` smoke artifacts
- AMD EPYC numbers combine archived `10x4` small-file samples and broader x86 benchmark summaries

When absolute throughput differs substantially, read the results together with:

- CPU / SoC class
- target triple
- selected SIMD backend
- profile / iteration count
- whether the source is a smoke artifact or a Criterion benchmark

Without that context, large gaps are easy to misread as regressions when they may simply reflect hardware class, runtime backend selection, or workload shape.

---

## Comparison with klauspost/reedsolomon (Go)

### Methodology Alignment

To compare Rust vs Go implementations:

1. Same hardware, same OS
2. Same (data, parity) configurations
3. Same shard sizes
4. Single-threaded comparison (set `RS_PARALLEL_POLICY_MAX_JOBS=1`)
5. Multi-threaded comparison (default parallelism)

### Go Benchmark Command

```bash
# In klauspost/reedsolomon repo:
go test -bench=BenchmarkEncode -benchtime=5s -cpu=1
go test -bench=BenchmarkReconstruct -benchtime=5s -cpu=1
```

### Key Differences

| Aspect | rustfs-erasure-codec (Rust) | klauspost/reedsolomon (Go) |
|--------|---------------------------|---------------------------|
| SIMD dispatch | Runtime (CPUID) | Runtime (CPUID) |
| GF backends | Scalar, SSSE3, AVX2, AVX-512, GFNI, NEON, VSX(feature-gated) | Scalar, SSSE3, AVX2, AVX-512, GFNI, NEON, ppc64le accel |
| Leopard codec | GF8 + GF16 | GF8 + GF16 |
| Parallelism | Rayon + configurable policy | Goroutines |
| Streaming API | Supported on `galois_8` block-based path | Supported via `NewStream()` |

---

## Action Items

1. **Extend Apple Silicon coverage** with Criterion `bandwidth` / `throughput_matrix` outputs in addition to small-file smoke artifacts
2. **Run benchmarks on Intel Ice Lake** to get GFNI numbers
3. **Run Go benchmarks on same hardware** for direct comparison
4. **Create comparison table** with normalized throughput (GiB/s per core)
