#[path = "../benches/common/mod.rs"]
mod bench_common;

use reed_solomon_erasure::galois_8::ReedSolomon;

use self::bench_common::{Operation, SMOKE_CASES, derived_seed, make_full_shards};

fn checksum_bytes(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325u64, |acc, &b| {
        (acc ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

fn checksum_shards(shards: &[Vec<u8>]) -> u64 {
    let mut acc = 0xcbf29ce484222325u64;
    for shard in shards {
        acc ^= checksum_bytes(shard);
        acc = acc.wrapping_mul(0x100000001b3);
    }
    acc
}

#[test]
fn selftest_golden_and_reconstruction_path() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let mut shards = vec![
        (0u8..16).collect::<Vec<_>>(),
        (16u8..32).collect::<Vec<_>>(),
        (32u8..48).collect::<Vec<_>>(),
        (48u8..64).collect::<Vec<_>>(),
        vec![0u8; 16],
        vec![0u8; 16],
    ];
    rs.encode(&mut shards).unwrap();

    assert_eq!(
        shards[4],
        vec![
            64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79
        ]
    );
    assert_eq!(
        shards[5],
        vec![
            80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95
        ]
    );
    assert_eq!(checksum_shards(&shards), 0x9118ae5245b6279b);

    let mut missing: Vec<Option<Vec<u8>>> = shards.iter().cloned().map(Some).collect();
    missing[0] = None;
    missing[5] = None;
    rs.reconstruct(&mut missing).unwrap();

    let rebuilt: Vec<Vec<u8>> = missing.into_iter().map(|shard| shard.unwrap()).collect();
    assert_eq!(rebuilt, shards);
}

#[test]
fn selftest_smoke_cases_encode_verify_reconstruct() {
    for case in SMOKE_CASES {
        let seed = derived_seed(Operation::Encode, *case);
        let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
        let mut shards =
            make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);

        rs.encode(&mut shards).unwrap();
        assert!(rs.verify(&shards).unwrap());

        let mut corrupted = shards.clone();
        corrupted[case.data_shards][0] ^= 0xff;
        assert!(!rs.verify(&corrupted).unwrap());

        let mut missing: Vec<Option<Vec<u8>>> = shards.iter().cloned().map(Some).collect();
        missing[0] = None;
        missing[case.data_shards] = None;
        rs.reconstruct(&mut missing).unwrap();

        let rebuilt: Vec<Vec<u8>> = missing.into_iter().map(|shard| shard.unwrap()).collect();
        assert_eq!(rebuilt, shards);
    }
}
