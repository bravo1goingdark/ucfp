use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use ufp_perceptual::{perceptualize_tokens, PerceptualConfig};

fn bench_perceptual(c: &mut Criterion) {
    let config = PerceptualConfig::default();
    let mut group = c.benchmark_group("perceptual");

    for size in [100, 500, 2000].iter() {
        let tokens: Vec<String> = (0..*size).map(|i| format!("word{i}")).collect();
        let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_function(format!("tokens_{size}"), |b| {
            b.iter(|| {
                perceptualize_tokens(black_box(&token_refs), black_box(&config))
                    .expect("perceptualize")
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_perceptual);
criterion_main!(benches);
