//! Comprehensive x86_64 benchmark for leopard_gf8 encoder.
//!
//! Runs leopard encode across many (data_shards, parity_shards, shard_size)
//! configurations and records throughput to JSON for cross-platform comparison.

#[path = "../benches/common/mod.rs"]
mod bench_common;

use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use rustfs_erasure_codec::galois_8::ReedSolomon;
use rustfs_erasure_codec::{CodecFamily, CodecOptions};

use self::bench_common::make_full_shards;

const BASE_SEED: u64 = 0x00EC_5EED_2026_0524;

/// All configurations to benchmark: (data_shards, parity_shards, label).
const CONFIGS: &[(usize, usize, &str)] = &[
    (4, 2, "4x2"),
    (10, 4, "10x4"),
    (32, 16, "32x16"),
    (64, 32, "64x32"),
    (96, 48, "96x48"),
    (128, 64, "128x64"),
];

/// Shard sizes to test: (bytes, label).
const SHARD_SIZES: &[(usize, &str)] = &[
    (1024, "1K"),
    (4 * 1024, "4K"),
    (16 * 1024, "16K"),
    (64 * 1024, "64K"),
    (128 * 1024, "128K"),
    (256 * 1024, "256K"),
    (512 * 1024, "512K"),
    (1024 * 1024, "1M"),
    (4 * 1024 * 1024, "4M"),
];

fn get_cpu_name() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn measure_encode(
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> (f64, f64, usize) {
    let codec = ReedSolomon::with_options(
        data_shards,
        parity_shards,
        CodecOptions::builder()
            .codec_family(CodecFamily::LeopardGF8)
            .build(),
    )
    .unwrap();

    let total_bytes = shard_size * data_shards;
    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);

    // Warmup
    {
        let mut shards = make_full_shards(BASE_SEED, data_shards, parity_shards, shard_size);
        #[cfg(feature = "std")]
        codec.encode_opt(black_box(&mut shards)).unwrap();
        #[cfg(not(feature = "std"))]
        codec.encode(black_box(&mut shards)).unwrap();
        black_box(());
    }

    // Scale iterations to data size
    let iterations = if total_mb < 1.0 {
        200
    } else if total_mb < 10.0 {
        100
    } else if total_mb < 100.0 {
        50
    } else {
        20
    };

    let start = Instant::now();
    for _ in 0..iterations {
        let mut shards = make_full_shards(BASE_SEED, data_shards, parity_shards, shard_size);
        #[cfg(feature = "std")]
        codec.encode_opt(black_box(&mut shards)).unwrap();
        #[cfg(not(feature = "std"))]
        codec.encode(black_box(&mut shards)).unwrap();
        black_box(());
    }
    let elapsed = start.elapsed();

    let ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
    let throughput_mb_s = total_mb / (ns_per_iter / 1_000_000_000.0);

    (ns_per_iter, throughput_mb_s, iterations)
}

#[test]
#[ignore]
fn comprehensive_leopard_encode_benchmark() {
    let cpu = get_cpu_name();
    let mut json_lines = Vec::new();

    eprintln!("=== Comprehensive Leopard GF8 Encode Benchmark ===");
    eprintln!("CPU: {}", cpu);
    eprintln!();

    for &(data_shards, parity_shards, config_label) in CONFIGS {
        for &(shard_size, shard_label) in SHARD_SIZES {
            let case_label = format!("{}_{}", config_label, shard_label);
            let total_data_mb = (shard_size * data_shards) as f64 / (1024.0 * 1024.0);

            eprint!("  {:<12} ({:7.2} MB) ... ", case_label, total_data_mb);

            let (ns_per_iter, throughput_mb_s, iterations) =
                measure_encode(data_shards, parity_shards, shard_size);

            eprintln!("{:>8.2} MB/s ({} iters)", throughput_mb_s, iterations);

            json_lines.push(format!(
                r#"    {{"case":"{}","data_shards":{},"parity_shards":{},"shard_size":{},"total_data_mb":{:.4},"iterations":{},"ns_per_iter":{:.2},"throughput_mb_s":{:.2}}}"#,
                case_label, data_shards, parity_shards, shard_size, total_data_mb,
                iterations, ns_per_iter, throughput_mb_s
            ));
        }
    }

    // Build JSON report
    let json = format!(
        r#"{{
  "schema_version": 1,
  "artifact_kind": "comprehensive-x86_64-benchmark",
  "timestamp": "2026-05-30",
  "platform": "{}",
  "arch": "{}",
  "cpu": "{}",
  "results": [
{}
  ]
}}"#,
        std::env::consts::OS,
        std::env::consts::ARCH,
        cpu,
        json_lines.join(",\n"),
    );

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/benchmark-smoke");
    fs::create_dir_all(&out_dir).unwrap();
    let json_path = out_dir.join("comprehensive-x86_64-benchmark.json");
    fs::write(&json_path, &json).unwrap();

    eprintln!("\nReport written to: {}", json_path.display());
    eprintln!("Total configurations: {}", json_lines.len());
}
