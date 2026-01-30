use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use canonical::{canonicalize, CanonicalizeConfig};

fn bench_canonicalize(c: &mut Criterion) {
    let config = CanonicalizeConfig::default();
    let mut group = c.benchmark_group("canonicalize");

    for size in [64, 512, 4096, 32768].iter() {
        let text = "word ".repeat(*size / 5);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_function(format!("bytes_{size}"), |b| {
            b.iter(|| {
                canonicalize(black_box("doc-1"), black_box(&text), black_box(&config))
                    .expect("canonicalize")
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_canonicalize);
criterion_main!(benches);
