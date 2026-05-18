use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mcporb_core::format::Chunk;
use mcporb_core::bm25::{build_index, search};

fn make_chunks(n: usize) -> Vec<Chunk> {
    (0..n).map(|i| Chunk {
        id: i as u32,
        document_id: 0,
        section_id: None,
        page: Some(i as u32 / 10 + 1),
        text: format!(
            "This is chunk number {}. It contains information about topic {} and related concepts. \
             The Model Driven Architecture approach provides a framework for software development \
             that separates business logic from platform-specific implementation details.",
            i, i % 50
        ),
        token_count: 40,
    }).collect()
}

fn bench_build(c: &mut Criterion) {
    let chunks = make_chunks(1000);
    c.bench_function("bm25_build_1000_chunks", |b| {
        b.iter(|| build_index(black_box(&chunks)))
    });
}

fn bench_search(c: &mut Criterion) {
    let chunks = make_chunks(10_000);
    let index = build_index(&chunks);
    c.bench_function("bm25_search_10000_chunks", |b| {
        b.iter(|| search(black_box(&index), black_box("model driven architecture framework"), 5))
    });
}

criterion_group!(benches, bench_build, bench_search);
criterion_main!(benches);
