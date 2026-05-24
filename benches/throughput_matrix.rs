mod common;

use std::convert::TryInto;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use reed_solomon_erasure::galois_8::ReedSolomon;

use self::common::{
    case_name, derived_seed, make_full_shards, BenchCase, Operation, SMOKE_CASES,
};

fn bench_encode(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::Encode, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::Encode, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();

    let mut group = c.benchmark_group("throughput_matrix_encode");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    group.bench_function(name, |b| {
        let mut shards = make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
        b.iter(|| {
            rs.encode(black_box(&mut shards)).unwrap();
        });
    });
    group.finish();
}

fn bench_verify(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::Verify, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::Verify, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let mut shards = make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode(&mut shards).unwrap();

    let mut group = c.benchmark_group("throughput_matrix_verify");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    group.bench_function(name, |b| {
        b.iter(|| {
            rs.verify(black_box(&shards)).unwrap();
        });
    });
    group.finish();
}

fn bench_reconstruct(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::Reconstruct, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::Reconstruct, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let mut original = make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode(&mut original).unwrap();

    let mut group = c.benchmark_group("throughput_matrix_reconstruct");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    group.bench_function(name, |b| {
        b.iter(|| {
            let mut shards: Vec<Option<Vec<u8>>> = original.iter().cloned().map(Some).collect();
            shards[0] = None;
            shards[case.data_shards] = None;
            rs.reconstruct(black_box(&mut shards)).unwrap();
        });
    });
    group.finish();
}

fn bench_reconstruct_data(c: &mut Criterion, case: BenchCase) {
    let name = case_name(Operation::ReconstructData, case);
    let throughput = case.shard_size * case.data_shards;
    let seed = derived_seed(Operation::ReconstructData, case);
    let rs = ReedSolomon::new(case.data_shards, case.parity_shards).unwrap();
    let mut original = make_full_shards(seed, case.data_shards, case.parity_shards, case.shard_size);
    rs.encode(&mut original).unwrap();

    let mut group = c.benchmark_group("throughput_matrix_reconstruct_data");
    group.throughput(Throughput::Bytes(throughput.try_into().unwrap()));
    group.bench_function(name, |b| {
        b.iter(|| {
            let mut shards: Vec<Option<Vec<u8>>> = original.iter().cloned().map(Some).collect();
            shards[0] = None;
            shards[1] = None;
            rs.reconstruct_data(black_box(&mut shards)).unwrap();
        });
    });
    group.finish();
}

fn smoke_matrix(c: &mut Criterion) {
    for case in SMOKE_CASES {
        bench_encode(c, *case);
        bench_verify(c, *case);
        bench_reconstruct(c, *case);
        bench_reconstruct_data(c, *case);
    }
}

criterion_group!(throughput_matrix_benches, smoke_matrix);
criterion_main!(throughput_matrix_benches);

