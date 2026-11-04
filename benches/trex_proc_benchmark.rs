use criterion::{Criterion, criterion_group, criterion_main};

fn process_transactions() {
}

fn process_transactions_benchmark(c: &mut Criterion) {
    c.bench_function("process_transactions", |b| {
        b.iter(process_transactions);
    });
}

criterion_group!(benches, process_transactions_benchmark);
criterion_main!(benches);
