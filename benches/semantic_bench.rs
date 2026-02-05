use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use semantic::{semanticize, SemanticConfig};

mod common;
use common::{
    generate_text_word_count, get_model_path, get_sample_text, get_tokenizer_path,
    models_available, TextLength,
};

/// Benchmark semantic embedding with different text lengths
fn bench_semanticize(c: &mut Criterion) {
    if !models_available() {
        println!("Skipping semantic benchmarks - ONNX model files not found");
        println!("Expected at: models/bge-small-en-v1.5/onnx/model.onnx");
        return;
    }

    let model_path = get_model_path().unwrap();
    let tokenizer_path = get_tokenizer_path().unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("semanticize");

    // Benchmark different text lengths
    for &length in &[TextLength::Short, TextLength::Medium, TextLength::Long] {
        let text = get_sample_text(length);
        let text_len = text.len();

        let config = SemanticConfig {
            tier: "balanced".into(),
            mode: "onnx".into(),
            model_name: "bge-small-en-v1.5".into(),
            model_path: model_path.clone(),
            tokenizer_path: Some(tokenizer_path.clone()),
            max_sequence_length: 512,
            ..Default::default()
        };

        group.throughput(Throughput::Bytes(text_len as u64));
        group.bench_function(format!("onnx_{:?}", length), |b| {
            b.to_async(&runtime).iter(|| async {
                let _ = semanticize(black_box("bench-doc"), black_box(text), black_box(&config))
                    .await
                    .expect("semanticize should succeed");
            });
        });
    }

    group.finish();
}

/// Benchmark semantic embedding with different tiers
fn bench_semanticize_tiers(c: &mut Criterion) {
    if !models_available() {
        return;
    }

    let model_path = get_model_path().unwrap();
    let tokenizer_path = get_tokenizer_path().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("semanticize_tiers");
    let text = get_sample_text(TextLength::Medium);

    for tier in &["fast", "balanced", "accurate"] {
        let config = SemanticConfig {
            tier: tier.to_string(),
            mode: "onnx".into(),
            model_name: "bge-small-en-v1.5".into(),
            model_path: model_path.clone(),
            tokenizer_path: Some(tokenizer_path.clone()),
            ..Default::default()
        };

        group.bench_function(*tier, |b| {
            b.to_async(&runtime).iter(|| async {
                let _ = semanticize(black_box("bench-doc"), black_box(text), black_box(&config))
                    .await
                    .expect("semanticize should succeed");
            });
        });
    }

    group.finish();
}

/// Benchmark batch processing
fn bench_semanticize_batch(c: &mut Criterion) {
    if !models_available() {
        return;
    }

    let model_path = get_model_path().unwrap();
    let tokenizer_path = get_tokenizer_path().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("semanticize_batch");
    let texts: Vec<String> = (0..100)
        .map(|i| {
            format!(
                "Document number {} with some content for batch processing benchmark.",
                i
            )
        })
        .collect();

    let config = SemanticConfig {
        tier: "balanced".into(),
        mode: "onnx".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: model_path.clone(),
        tokenizer_path: Some(tokenizer_path.clone()),
        ..Default::default()
    };

    // Benchmark different batch sizes
    for batch_size in [1, 10, 50, 100].iter() {
        let batch: Vec<&str> = texts.iter().take(*batch_size).map(|s| s.as_str()).collect();
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_function(format!("batch_{}", batch_size), |b| {
            b.to_async(&runtime).iter(|| async {
                for text in &batch {
                    let _ =
                        semanticize(black_box("bench-doc"), black_box(*text), black_box(&config))
                            .await
                            .expect("semanticize should succeed");
                }
            });
        });
    }

    group.finish();
}

/// Benchmark with chunking enabled (long documents)
fn bench_semanticize_chunking(c: &mut Criterion) {
    if !models_available() {
        return;
    }

    let model_path = get_model_path().unwrap();
    let tokenizer_path = get_tokenizer_path().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("semanticize_chunking");

    // Generate very long text that will require chunking
    let long_text = generate_text_word_count(2000); // ~2000 words, exceeds 512 tokens

    let config_no_chunking = SemanticConfig {
        tier: "balanced".into(),
        mode: "onnx".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path: model_path.clone(),
        tokenizer_path: Some(tokenizer_path.clone()),
        enable_chunking: false,
        max_sequence_length: 512,
        ..Default::default()
    };

    let config_with_chunking = SemanticConfig {
        tier: "balanced".into(),
        mode: "onnx".into(),
        model_name: "bge-small-en-v1.5".into(),
        model_path,
        tokenizer_path: Some(tokenizer_path),
        enable_chunking: true,
        max_sequence_length: 512,
        chunk_overlap_ratio: 0.5,
        ..Default::default()
    };

    group.bench_function("no_chunking", |b| {
        b.to_async(&runtime).iter(|| async {
            let _ = semanticize(
                black_box("bench-doc"),
                black_box(&long_text),
                black_box(&config_no_chunking),
            )
            .await
            .expect("semanticize should succeed");
        });
    });

    group.bench_function("with_chunking", |b| {
        b.to_async(&runtime).iter(|| async {
            let _ = semanticize(
                black_box("bench-doc"),
                black_box(&long_text),
                black_box(&config_with_chunking),
            )
            .await
            .expect("semanticize should succeed");
        });
    });

    group.finish();
}

/// Benchmark stub mode (no model files needed)
fn bench_semanticize_stub(c: &mut Criterion) {
    let mut group = c.benchmark_group("semanticize_stub");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let config = SemanticConfig {
        tier: "fast".into(), // Stub mode used for fast tier
        mode: "stub".into(),
        ..Default::default()
    };

    for &length in &[TextLength::Short, TextLength::Medium, TextLength::Long] {
        let text = get_sample_text(length);
        group.bench_function(format!("stub_{:?}", length), |b| {
            b.to_async(&runtime).iter(|| async {
                let _ = semanticize(black_box("bench-doc"), black_box(text), black_box(&config))
                    .await
                    .expect("semanticize should succeed");
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_semanticize,
    bench_semanticize_tiers,
    bench_semanticize_batch,
    bench_semanticize_chunking,
    bench_semanticize_stub
);
criterion_main!(benches);
