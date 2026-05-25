mod common;

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use reed_solomon_erasure::galois_8;

const COEFF: u8 = 173;
const XOR_COEFF: u8 = 91;
const LENGTHS: &[usize] = &[64 * 1024, 1024 * 1024, 4 * 1024 * 1024];

fn backend_label() -> String {
    let active = galois_8::active_backend_name();
    let active_id = format!("{:?}", galois_8::active_backend_id());
    let active_kind = format!("{:?}", galois_8::active_backend_kind());
    let override_value =
        std::env::var("RSE_BACKEND_OVERRIDE").unwrap_or_else(|_| "auto".to_string());
    format!("{active}_{active_id}_{active_kind}_override_{override_value}")
}

fn make_input(len: usize) -> Vec<u8> {
    common::make_data_shards(common::BASE_SEED ^ len as u64, 1, len)
        .pop()
        .unwrap()
}

fn bench_mul_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!("galois_mul_slice_{}", backend_label()));
    for &len in LENGTHS {
        let input = make_input(len);
        let mut out = vec![0u8; len];
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_function(format!("len_{len}"), |b| {
            b.iter(|| {
                galois_8::mul_slice(COEFF, black_box(&input), black_box(&mut out));
            });
        });
    }
    group.finish();
}

fn bench_mul_slice_xor(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!("galois_mul_slice_xor_{}", backend_label()));
    for &len in LENGTHS {
        let input = make_input(len);
        let mut out = vec![0x5a; len];
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_function(format!("len_{len}"), |b| {
            b.iter(|| {
                galois_8::mul_slice_xor(XOR_COEFF, black_box(&input), black_box(&mut out));
            });
        });
    }
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_mul_slice(c);
    bench_mul_slice_xor(c);
}

criterion_group!(galois_backend_benches, benches);
criterion_main!(galois_backend_benches);
