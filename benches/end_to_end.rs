//! Mixed-modality end-to-end fingerprint bench.
//!
//! Three sub-benches — text MinHash, image multi-hash, audio Wang —
//! run a single fingerprint per iteration through the same modality
//! wrappers `/v1/ingest/*` uses. Designed to give one number per
//! modality for tracking the perf delta of mimalloc + CPU-baseline
//! changes (and any future hot-path tweaks) across releases.
//!
//! Run with:
//!   cargo bench --bench end_to_end                       # current baseline
//!   RUSTFLAGS="-C target-cpu=native" cargo bench --bench end_to_end
//!
//! The bench is local-only. CI doesn't run it because criterion takes
//! minutes per pass and benchmark numbers aren't deterministic enough
//! across runners to gate PRs on.
//!
//! Numbers from the inaugural run are documented in CHANGELOG.md when
//! the perf-relevant change lands.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

fn bench_text_minhash(c: &mut Criterion) {
    // ~5.6 KiB pangram repeated — large enough to dominate canonicalize
    // + tokenize cost and exercise the shingle window.
    let text: String = "the quick brown fox jumps over the lazy dog. ".repeat(128);
    let mut g = c.benchmark_group("text/minhash-h128");
    g.throughput(Throughput::Bytes(text.len() as u64));
    g.bench_function("default-opts", |b| {
        b.iter(|| {
            let rec = ucfp::text::fingerprint_minhash(black_box(&text), 0, 1)
                .expect("text fingerprint");
            black_box(rec.fingerprint.len())
        });
    });
    g.finish();
}

fn bench_image_multihash(c: &mut Criterion) {
    // 256×256 colour ramp — within the per-image SDK cap and big enough
    // that resize + DCT dominate the inner loop.
    let png = synth_png(256, 256);
    let mut g = c.benchmark_group("image/multihash");
    g.throughput(Throughput::Bytes(png.len() as u64));
    g.bench_function("default-preprocess", |b| {
        b.iter(|| {
            let rec = ucfp::image::fingerprint(black_box(&png), 0, 1)
                .expect("image fingerprint");
            black_box(rec.fingerprint.len())
        });
    });
    g.finish();
}

fn bench_audio_wang(c: &mut Criterion) {
    // 4 s of 440 Hz mono sine at 8 kHz — comfortably above Wang's
    // ~2 s minimum and short enough to keep iteration time reasonable.
    let sr: u32 = 8_000;
    let n = (sr as usize) * 4;
    let mut samples = vec![0f32; n];
    for (i, s) in samples.iter_mut().enumerate() {
        let t = i as f32 / sr as f32;
        *s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
    }
    let mut g = c.benchmark_group("audio/wang");
    g.throughput(Throughput::Bytes((samples.len() * 4) as u64));
    g.bench_function("4s-440hz-8khz", |b| {
        b.iter(|| {
            let rec = ucfp::audio::fingerprint_wang(black_box(&samples), sr, 0, 1)
                .expect("wang fingerprint");
            black_box(rec.fingerprint.len())
        });
    });
    g.finish();
}

fn synth_png(w: u32, h: u32) -> Vec<u8> {
    let img = image::ImageBuffer::from_fn(w, h, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128u8])
    });
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encode synthetic png");
    buf
}

criterion_group!(
    benches,
    bench_text_minhash,
    bench_image_multihash,
    bench_audio_wang
);
criterion_main!(benches);
