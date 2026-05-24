use reed_solomon_erasure::galois_8::ReedSolomon;
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

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

fn fixed_4x2_shards() -> Vec<Vec<u8>> {
    vec![
        (0u8..16).collect(),
        (16u8..32).collect(),
        (32u8..48).collect(),
        (48u8..64).collect(),
        vec![0u8; 16],
        vec![0u8; 16],
    ]
}

fn seeded_data(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = SmallRng::seed_from_u64(seed);
    (0..len).map(|_| rng.random::<u8>()).collect()
}

fn repeated_pattern_data(len: usize, pattern: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let remaining = len - out.len();
        let take = core::cmp::min(remaining, pattern.len());
        out.extend_from_slice(&pattern[..take]);
    }
    out
}

#[test]
fn golden_encode_4x2_incrementing_input() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let mut shards = fixed_4x2_shards();
    rs.encode(&mut shards).unwrap();

    assert_eq!(
        shards[4],
        vec![64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79]
    );
    assert_eq!(
        shards[5],
        vec![80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95]
    );
    assert_eq!(checksum_shards(&shards), 0x9118ae5245b6279b);
}

#[test]
fn golden_reconstruct_data_4x2_incrementing_input() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let mut encoded = fixed_4x2_shards();
    rs.encode(&mut encoded).unwrap();

    let mut shards: Vec<Option<Vec<u8>>> = encoded.iter().cloned().map(Some).collect();
    shards[1] = None;
    shards[4] = None;

    rs.reconstruct_data(&mut shards).unwrap();

    let recovered: Vec<Vec<u8>> = shards.into_iter().map(|shard| shard.unwrap_or_default()).collect();
    assert_eq!(recovered[1], (16u8..32).collect::<Vec<_>>());
    assert!(recovered[4].is_empty());
    assert_eq!(checksum_shards(&recovered), 0x961eb966d059c4cb);
}

#[test]
fn golden_reconstruct_4x2_incrementing_input() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let mut encoded = fixed_4x2_shards();
    rs.encode(&mut encoded).unwrap();

    let mut shards: Vec<Option<Vec<u8>>> = encoded.iter().cloned().map(Some).collect();
    shards[0] = None;
    shards[5] = None;

    rs.reconstruct(&mut shards).unwrap();

    let recovered: Vec<Vec<u8>> = shards.into_iter().map(|shard| shard.unwrap_or_default()).collect();
    assert_eq!(recovered, encoded);
    assert_eq!(checksum_shards(&recovered), 0x9118ae5245b6279b);
}

#[test]
fn golden_verify_detects_corruption_4x2_incrementing_input() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let mut shards = fixed_4x2_shards();
    rs.encode(&mut shards).unwrap();

    assert!(rs.verify(&shards).unwrap());

    shards[5][3] ^= 0xff;

    assert!(!rs.verify(&shards).unwrap());
}

#[test]
fn golden_encode_4x2_seeded_random_input() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let data = seeded_data(61, 0xA5A5_1024);
    let mut shards = rs.split(&data).unwrap();
    shards.extend(vec![vec![0u8; shards[0].len()]; 2]);
    rs.encode(&mut shards).unwrap();

    assert!(rs.verify(&shards).unwrap());
    assert_eq!(checksum_shards(&shards), 0x81458d413eb7d9cc);
}

#[test]
fn golden_encode_4x2_all_zero_input() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let data = vec![0u8; 61];
    let mut shards = rs.split(&data).unwrap();
    shards.extend(vec![vec![0u8; shards[0].len()]; 2]);
    rs.encode(&mut shards).unwrap();

    assert!(rs.verify(&shards).unwrap());
    assert_eq!(checksum_shards(&shards), 0xb82e0d6749372cdb);
}

#[test]
fn golden_encode_4x2_repeated_pattern_input() {
    let rs = ReedSolomon::new(4, 2).unwrap();
    let data = repeated_pattern_data(61, &[0xDE, 0xAD, 0xBE, 0xEF]);
    let mut shards = rs.split(&data).unwrap();
    shards.extend(vec![vec![0u8; shards[0].len()]; 2]);
    rs.encode(&mut shards).unwrap();

    assert!(rs.verify(&shards).unwrap());
    assert_eq!(checksum_shards(&shards), 0x0de38c74d7643ce3);
}
