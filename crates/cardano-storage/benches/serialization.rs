//! Benchmarks for CardanoDB serialization operations

use cardano_storage::cardanodb::{
    ledger::state::LedgerState,
    types::{Blake2b256Hash, BlockLocation, BlockNo, ChunkNo, EpochNo, SlotNo},
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn ledger_state_serialization_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ledger_state_serialization");

    let state = LedgerState {
        slot: SlotNo(123456),
        block_no: BlockNo(789012),
        epoch: EpochNo(345),
    };

    group.bench_function("serialize", |b| {
        b.iter(|| {
            let json = serde_json::to_vec(black_box(&state)).unwrap();
            black_box(json);
        });
    });

    let json = serde_json::to_vec(&state).unwrap();
    group.bench_function("deserialize", |b| {
        b.iter(|| {
            let state: LedgerState = serde_json::from_slice(black_box(&json)).unwrap();
            black_box(state);
        });
    });

    group.finish();
}

fn block_location_serialization_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_location_serialization");

    let loc = BlockLocation::new(123456, 4096);

    group.bench_function("serialize", |b| {
        b.iter(|| {
            let json = serde_json::to_vec(black_box(&loc)).unwrap();
            black_box(json);
        });
    });

    let json = serde_json::to_vec(&loc).unwrap();
    group.bench_function("deserialize", |b| {
        b.iter(|| {
            let loc: BlockLocation = serde_json::from_slice(black_box(&json)).unwrap();
            black_box(loc);
        });
    });

    group.finish();
}

fn hash_serialization_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_serialization");

    let hash = Blake2b256Hash::new([0xaa; 32]);

    group.bench_function("serialize", |b| {
        b.iter(|| {
            let json = serde_json::to_vec(black_box(&hash)).unwrap();
            black_box(json);
        });
    });

    let json = serde_json::to_vec(&hash).unwrap();
    group.bench_function("deserialize", |b| {
        b.iter(|| {
            let hash: Blake2b256Hash = serde_json::from_slice(black_box(&json)).unwrap();
            black_box(hash);
        });
    });

    group.finish();
}

fn chunk_no_operations_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_no_ops");

    group.bench_function("create", |b| {
        b.iter(|| {
            let chunk = ChunkNo::new(black_box(42));
            black_box(chunk);
        });
    });

    let chunk = ChunkNo::new(42);
    group.bench_function("to_u64", |b| {
        b.iter(|| {
            let n = black_box(chunk).to_u64();
            black_box(n);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    ledger_state_serialization_benchmark,
    block_location_serialization_benchmark,
    hash_serialization_benchmark,
    chunk_no_operations_benchmark
);
criterion_main!(benches);
