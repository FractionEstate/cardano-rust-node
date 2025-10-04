//! Benchmarks for CardanoDB ring buffer implementation

use cardano_storage::cardanodb::volatile::ring_buffer::RingBuffer;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn ring_buffer_push_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_push");

    for capacity in [100, 1000, 10_000].iter() {
        group.throughput(Throughput::Elements(*capacity as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(capacity),
            capacity,
            |b, &capacity| {
                b.iter(|| {
                    let mut buffer = RingBuffer::with_capacity(capacity);
                    for i in 0..capacity {
                        buffer.push(black_box(i as u64));
                    }
                });
            },
        );
    }
    group.finish();
}

fn ring_buffer_get_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_get");

    for size in [100, 1000, 10_000].iter() {
        let mut buffer = RingBuffer::with_capacity(*size);
        for i in 0..*size {
            buffer.push(i as u64);
        }

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let idx = black_box(size / 2);
                black_box(buffer.get(idx));
            });
        });
    }
    group.finish();
}

fn ring_buffer_iter_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_iter");

    for size in [100, 1000, 10_000].iter() {
        let mut buffer = RingBuffer::with_capacity(*size);
        for i in 0..*size {
            buffer.push(i as u64);
        }

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let sum: u64 = buffer.iter().sum();
                black_box(sum);
            });
        });
    }
    group.finish();
}

fn ring_buffer_wraparound_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ring_buffer_wraparound");

    for capacity in [100, 1000].iter() {
        group.throughput(Throughput::Elements(*capacity as u64 * 2));
        group.bench_with_input(
            BenchmarkId::from_parameter(capacity),
            capacity,
            |b, &capacity| {
                b.iter(|| {
                    let mut buffer = RingBuffer::with_capacity(capacity);
                    // Push 2x capacity to force wraparound
                    for i in 0..(capacity * 2) {
                        buffer.push(black_box(i as u64));
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    ring_buffer_push_benchmark,
    ring_buffer_get_benchmark,
    ring_buffer_iter_benchmark,
    ring_buffer_wraparound_benchmark
);
criterion_main!(benches);
