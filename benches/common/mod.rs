use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

pub const BASE_SEED: u64 = 0xEC5E_ED20_2605_24;

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
    Verify,
    Reconstruct,
    ReconstructData,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Operation::Encode => "encode",
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
];

pub fn derived_seed(operation: Operation, case: BenchCase) -> u64 {
    let op_tag = match operation {
        Operation::Encode => 0x11u64,
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

pub fn make_full_shards(seed: u64, data_shards: usize, parity_shards: usize, shard_size: usize) -> Vec<Vec<u8>> {
    let mut shards = make_data_shards(seed, data_shards, shard_size);
    shards.extend(make_empty_parity_shards(parity_shards, shard_size));
    shards
}

pub fn case_name(operation: Operation, case: BenchCase) -> String {
    format!("{}_{}", operation.as_str(), case.label)
}
