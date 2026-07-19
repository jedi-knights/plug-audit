//! Perf benchmark harness — real rules land in PA-5, this stub exists so
//! `cargo bench` is wired from the scaffold onward and regressions are caught
//! from the first rule that ships.

use criterion::{Criterion, criterion_group, criterion_main};

fn scaffold_placeholder(c: &mut Criterion) {
    c.bench_function("scaffold_placeholder", |b| {
        b.iter(|| std::hint::black_box(1 + 1));
    });
}

criterion_group!(benches, scaffold_placeholder);
criterion_main!(benches);
