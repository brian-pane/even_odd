use std::hint::black_box;
use criterion::{Criterion, criterion_group, criterion_main};
use even_odd::{EvenOdd};

fn max(c: &mut Criterion) {
    c.bench_function("is_even(u32::MAX)", |b| b.iter(|| even_odd::is_even(black_box(u32::MAX))));
}

criterion_group!(single_threaded, max);

fn max_rayon(c: &mut Criterion) {
    c.bench_function("is_even_rayon(u32::MAX)", |b| b.iter(|| even_odd::is_even_rayon(black_box(u32::MAX))));
}

criterion_group!(rayon, max_rayon);

fn max_threadpool(c: &mut Criterion) {
    let even_odd = EvenOdd::new();
    c.bench_function("is_even_threadpool(u32::MAX)", |b| b.iter(|| even_odd.is_even(black_box(u32::MAX))));
}

criterion_group!(threadpool, max_threadpool);

criterion_main!(single_threaded, rayon, threadpool);
