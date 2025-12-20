use std::hint::black_box;
use criterion::{Criterion, criterion_group, criterion_main};

fn max(c: &mut Criterion) {
    c.bench_function("is_even(u32::MAX)", |b| b.iter(|| even_odd::is_even(black_box(u32::MAX))));
}

criterion_group!(single_threaded, max);

fn max_parallel(c: &mut Criterion) {
    c.bench_function("is_even_parallel(u32::MAX)", |b| b.iter(|| even_odd::is_even_parallel(black_box(u32::MAX))));
}

criterion_group!(multi_threaded, max_parallel);

criterion_main!(single_threaded, multi_threaded);
