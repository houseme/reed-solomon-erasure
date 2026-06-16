#![allow(dead_code)]

use std::process::Command;

use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

use rustfs_erasure_codec::galois_8::{active_backend_id, active_backend_kind, active_backend_name};

pub const BASE_SEED: u64 = 0x00EC_5EED_2026_0524;

#[derive(Clone, Copy, Debug)]
pub struct BenchCase {
    pub data_shards: usize,
    pub parity_shards: usize,
    pub shard_size: usize,
    pub label: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Encode,
    LeopardSetup,
    LeopardEncode,
    Update,
    Verify,
    Reconstruct,
    ReconstructData,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Operation::Encode => "encode",
            Operation::LeopardSetup => "leopard_setup",
            Operation::LeopardEncode => "leopard_encode",
            Operation::Update => "update",
            Operation::Verify => "verify",
            Operation::Reconstruct => "reconstruct",
            Operation::ReconstructData => "reconstruct_data",
        }
    }
}

pub const SMOKE_CASES: &[BenchCase] = &[
    BenchCase {
        data_shards: 4,
        parity_shards: 2,
        shard_size: 64 * 1024,
        label: "4x2_64k",
    },
    BenchCase {
        data_shards: 10,
        parity_shards: 4,
        shard_size: 1024 * 1024,
        label: "10x4_1m",
    },
    BenchCase {
        data_shards: 32,
        parity_shards: 16,
        shard_size: 1024 * 1024,
        label: "32x16_1m",
    },
];

pub const FAST_SMOKE_CASES: &[BenchCase] = &[
    BenchCase {
        data_shards: 4,
        parity_shards: 2,
        shard_size: 64 * 1024,
        label: "4x2_64k",
    },
    BenchCase {
        data_shards: 10,
        parity_shards: 4,
        shard_size: 1024 * 1024,
        label: "10x4_1m",
    },
];

pub const QUICK_SMOKE_CASES: &[BenchCase] = &[BenchCase {
    data_shards: 4,
    parity_shards: 2,
    shard_size: 64 * 1024,
    label: "4x2_64k",
}];

#[allow(dead_code)]
pub const FULL_CASES: &[BenchCase] = &[
    BenchCase {
        data_shards: 4,
        parity_shards: 2,
        shard_size: 64 * 1024,
        label: "4x2_64k",
    },
    BenchCase {
        data_shards: 4,
        parity_shards: 2,
        shard_size: 1024 * 1024,
        label: "4x2_1m",
    },
    BenchCase {
        data_shards: 4,
        parity_shards: 2,
        shard_size: 4 * 1024 * 1024,
        label: "4x2_4m",
    },
    BenchCase {
        data_shards: 8,
        parity_shards: 4,
        shard_size: 64 * 1024,
        label: "8x4_64k",
    },
    BenchCase {
        data_shards: 8,
        parity_shards: 4,
        shard_size: 1024 * 1024,
        label: "8x4_1m",
    },
    BenchCase {
        data_shards: 8,
        parity_shards: 4,
        shard_size: 4 * 1024 * 1024,
        label: "8x4_4m",
    },
    BenchCase {
        data_shards: 10,
        parity_shards: 4,
        shard_size: 64 * 1024,
        label: "10x4_64k",
    },
    BenchCase {
        data_shards: 10,
        parity_shards: 4,
        shard_size: 1024 * 1024,
        label: "10x4_1m",
    },
    BenchCase {
        data_shards: 10,
        parity_shards: 4,
        shard_size: 4 * 1024 * 1024,
        label: "10x4_4m",
    },
    BenchCase {
        data_shards: 16,
        parity_shards: 8,
        shard_size: 64 * 1024,
        label: "16x8_64k",
    },
    BenchCase {
        data_shards: 16,
        parity_shards: 8,
        shard_size: 1024 * 1024,
        label: "16x8_1m",
    },
    BenchCase {
        data_shards: 16,
        parity_shards: 8,
        shard_size: 4 * 1024 * 1024,
        label: "16x8_4m",
    },
    BenchCase {
        data_shards: 32,
        parity_shards: 16,
        shard_size: 64 * 1024,
        label: "32x16_64k",
    },
    BenchCase {
        data_shards: 32,
        parity_shards: 16,
        shard_size: 1024 * 1024,
        label: "32x16_1m",
    },
    BenchCase {
        data_shards: 32,
        parity_shards: 16,
        shard_size: 4 * 1024 * 1024,
        label: "32x16_4m",
    },
    BenchCase {
        data_shards: 64,
        parity_shards: 32,
        shard_size: 64 * 1024,
        label: "64x32_64k",
    },
    BenchCase {
        data_shards: 64,
        parity_shards: 32,
        shard_size: 1024 * 1024,
        label: "64x32_1m",
    },
    BenchCase {
        data_shards: 64,
        parity_shards: 32,
        shard_size: 4 * 1024 * 1024,
        label: "64x32_4m",
    },
    BenchCase {
        data_shards: 96,
        parity_shards: 48,
        shard_size: 1024 * 1024,
        label: "96x48_1m",
    },
    BenchCase {
        data_shards: 96,
        parity_shards: 48,
        shard_size: 4 * 1024 * 1024,
        label: "96x48_4m",
    },
    BenchCase {
        data_shards: 128,
        parity_shards: 64,
        shard_size: 1024 * 1024,
        label: "128x64_1m",
    },
    BenchCase {
        data_shards: 128,
        parity_shards: 64,
        shard_size: 4 * 1024 * 1024,
        label: "128x64_4m",
    },
];

pub fn derived_seed(operation: Operation, case: BenchCase) -> u64 {
    let op_tag = match operation {
        Operation::Encode => 0x11u64,
        Operation::LeopardSetup => 0x66u64,
        Operation::LeopardEncode => 0x77u64,
        Operation::Update => 0x55u64,
        Operation::Verify => 0x22u64,
        Operation::Reconstruct => 0x33u64,
        Operation::ReconstructData => 0x44u64,
    };

    BASE_SEED
        ^ op_tag
        ^ ((case.data_shards as u64) << 48)
        ^ ((case.parity_shards as u64) << 32)
        ^ case.shard_size as u64
}

pub fn make_data_shards(seed: u64, data_shards: usize, shard_size: usize) -> Vec<Vec<u8>> {
    let mut rng = SmallRng::seed_from_u64(seed);
    (0..data_shards)
        .map(|_| (0..shard_size).map(|_| rng.random::<u8>()).collect())
        .collect()
}

pub fn make_empty_parity_shards(parity_shards: usize, shard_size: usize) -> Vec<Vec<u8>> {
    (0..parity_shards).map(|_| vec![0u8; shard_size]).collect()
}

pub fn make_full_shards(
    seed: u64,
    data_shards: usize,
    parity_shards: usize,
    shard_size: usize,
) -> Vec<Vec<u8>> {
    let mut shards = make_data_shards(seed, data_shards, shard_size);
    shards.extend(make_empty_parity_shards(parity_shards, shard_size));
    shards
}

pub fn case_name(operation: Operation, case: BenchCase) -> String {
    format!("{}_{}", operation.as_str(), case.label)
}

pub const ARTIFACT_SCHEMA_VERSION: u32 = 1;

pub fn git_revision() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn features() -> String {
    let mut enabled = Vec::new();
    if cfg!(feature = "std") {
        enabled.push("std");
    }
    if cfg!(feature = "simd-accel") {
        enabled.push("simd-accel");
    } else {
        if cfg!(feature = "simd-neon") {
            enabled.push("simd-neon");
        }
        if cfg!(feature = "simd-ssse3") {
            enabled.push("simd-ssse3");
        }
        if cfg!(feature = "simd-avx2") {
            enabled.push("simd-avx2");
        }
        if cfg!(feature = "simd-avx512") {
            enabled.push("simd-avx512");
        }
        if cfg!(feature = "simd-gfni") {
            enabled.push("simd-gfni");
        }
    }
    if cfg!(feature = "benchmark-metrics") {
        enabled.push("benchmark-metrics");
    }
    if enabled.is_empty() {
        "none".to_string()
    } else {
        enabled.join("|")
    }
}

pub fn backend() -> &'static str {
    active_backend_name()
}

pub fn backend_id() -> String {
    format!("{:?}", active_backend_id())
}

pub fn backend_kind() -> String {
    format!("{:?}", active_backend_kind())
}

pub fn backend_override() -> String {
    std::env::var("RSE_BACKEND_OVERRIDE").unwrap_or_else(|_| "auto".to_string())
}

pub fn benchmark_metrics_enabled() -> bool {
    cfg!(feature = "benchmark-metrics")
}

pub fn target_triple() -> String {
    format!(
        "{}-{}-{}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        option_env!("CARGO_CFG_TARGET_ENV").unwrap_or("unknown"),
    )
}
