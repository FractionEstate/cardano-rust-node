//! Benchmarks for CardanoDB hash operations

use cardano_storage::cardanodb::types::Blake2b256Hash;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn blake2b_hash_computation_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("blake2b_hash");

    for size in [32, 256, 1024, 4096, 32768].iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| {
                let hash = Blake2b256Hash::hash(black_box(data));
                black_box(hash);
            });
        });
    }
    group.finish();
}

fn blake2b_hash_display_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("blake2b_display");

    let hash = Blake2b256Hash::new([0xaa; 32]);

    group.bench_function("format", |b| {
        b.iter(|| {
            let s = format!("{}", black_box(&hash));
            black_box(s);
        });
    });

    group.finish();
}

fn blake2b_hash_from_slice_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("blake2b_from_slice");

    let bytes = [0xbb; 32];

    group.bench_function("valid", |b| {
        b.iter(|| {
            let hash = Blake2b256Hash::from_slice(black_box(&bytes));
            black_box(hash);
        });
    });

    let invalid_bytes = [0xcc; 31];
    group.bench_function("invalid", |b| {
        b.iter(|| {
            let hash = Blake2b256Hash::from_slice(black_box(&invalid_bytes));
            black_box(hash);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    blake2b_hash_computation_benchmark,
    blake2b_hash_display_benchmark,
    blake2b_hash_from_slice_benchmark
);
criterion_main!(benches);
