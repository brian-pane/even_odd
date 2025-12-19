use std::hint::black_box;
use criterion::{Criterion, criterion_group, criterion_main};

fn zero(c: &mut Criterion) {
    c.bench_function("is_even(0)", |b| b.iter(|| even_odd::is_even(black_box(0))));
}

fn one(c: &mut Criterion) {
    c.bench_function("is_even(1)", |b| b.iter(|| even_odd::is_even(black_box(1))));
}

fn max(c: &mut Criterion) {
    c.bench_function("is_even(u32::MAX)", |b| b.iter(|| even_odd::is_even(black_box(u32::MAX))));
}

criterion_group!(single_threaded, zero, one, max);

fn zero_parallel(c: &mut Criterion) {
    c.bench_function("is_even_parallel(0)", |b| b.iter(|| even_odd::is_even_parallel(black_box(0))));
}

fn one_parallel(c: &mut Criterion) {
    c.bench_function("is_even_parallel(1)", |b| b.iter(|| even_odd::is_even_parallel(black_box(1))));
}

fn max_parallel(c: &mut Criterion) {
    c.bench_function("is_even_parallel(u32::MAX)", |b| b.iter(|| even_odd::is_even_parallel(black_box(u32::MAX))));
}

criterion_group!(multi_threaded, zero_parallel, one_parallel, max_parallel);

criterion_main!(single_threaded, multi_threaded);
