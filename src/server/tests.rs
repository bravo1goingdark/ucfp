//! End-to-end HTTP integration tests against an in-memory `EmbeddedBackend`.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, Response, StatusCode},
};
use http_body_util::BodyExt;
use serde::Deserialize;
use tower::util::ServiceExt;

use crate::index::embedded::EmbeddedBackend;

use super::{
    ApiKeyLookup, InMemoryTokenBucket, NoopUsageSink, ServerState, StaticMapKey, router,
    router_with_state,
};

async fn fixture() -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let backend = EmbeddedBackend::open(dir.path().join("ucfp.redb")).unwrap();
    let app = router(Arc::new(backend));
    (app, dir)
}

fn json_body(v: serde_json::Value) -> Body {
    Body::from(serde_json::to_vec(&v).unwrap())
}

async fn read_json<T: for<'de> Deserialize<'de>>(resp: Response<Body>) -> T {
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn healthz_returns_ok() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn upsert_then_query_round_trips() {
    let (app, _dir) = fixture().await;

    let upsert_req = serde_json::json!({
        "records": [
            {
                "tenant_id": 1, "record_id": 100,
                "modality": "Image",
                "format_version": 1, "algorithm": "test", "config_hash": 0,
                "fingerprint": [1, 2, 3],
                "embedding": [1.0, 0.0, 0.0]
            },
            {
                "tenant_id": 1, "record_id": 200,
                "modality": "Image",
                "format_version": 1, "algorithm": "test", "config_hash": 0,
                "fingerprint": [4, 5, 6],
                "embedding": [0.7, 0.7, 0.0]
            }
        ]
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/records")
                .header("content-type", "application/json")
                .body(json_body(upsert_req))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["upserted"], 2);

    let query_req = serde_json::json!({
        "tenant_id": 1,
        "modality": "Image",
        "k": 5,
        "vector": [0.6, 0.6, 0.0],
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/query")
                .header("content-type", "application/json")
                .body(json_body(query_req))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(resp).await;
    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0]["record_id"], 200);
    assert_eq!(hits[0]["source"], "vector");
}

#[tokio::test]
async fn delete_returns_204_and_removes_record() {
    let (app, _dir) = fixture().await;

    let upsert_req = serde_json::json!({
        "records": [{
            "tenant_id": 7, "record_id": 42,
            "modality": "Text",
            "format_version": 1, "algorithm": "minhash-h128", "config_hash": 0,
            "fingerprint": [9],
            "embedding": [1.0]
        }]
    });
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/records")
                .header("content-type", "application/json")
                .body(json_body(upsert_req))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/records/7/42")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let query_req = serde_json::json!({
        "tenant_id": 7,
        "modality": "Text",
        "k": 5,
        "vector": [1.0],
    });
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/query")
                .header("content-type", "application/json")
                .body(json_body(query_req))
                .unwrap(),
        )
        .await
        .unwrap();
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["hits"].as_array().unwrap().len(), 0);
}

// ── Modality-specific ingest routes ────────────────────────────────────

#[cfg(feature = "text")]
#[tokio::test]
async fn ingest_text_round_trip() {
    let (app, _dir) = fixture().await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/3/77")
                .header("content-type", "text/plain; charset=utf-8")
                .body(Body::from("the quick brown fox jumps over the lazy dog"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["tenant_id"], 3);
    assert_eq!(body["record_id"], 77);
    assert_eq!(body["modality"], "Text");
    assert_eq!(body["algorithm"], "minhash-h128");
    assert!(body["fingerprint_bytes"].as_u64().unwrap() > 0);
    assert_eq!(body["has_embedding"], false);
}

#[cfg(feature = "text")]
#[tokio::test]
async fn ingest_text_rejects_invalid_utf8() {
    let (app, _dir) = fixture().await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/1/1")
                // Lone continuation byte — invalid UTF-8.
                .body(Body::from(vec![0x80u8]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["error"], "modality");
}

#[cfg(feature = "image")]
fn synthetic_png(w: u32, h: u32) -> Vec<u8> {
    let img = image::ImageBuffer::from_fn(w, h, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128u8])
    });
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

#[cfg(feature = "image")]
#[tokio::test]
async fn ingest_image_round_trip() {
    let (app, _dir) = fixture().await;

    let png = synthetic_png(64, 64);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/image/9/1234")
                .header("content-type", "image/png")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["tenant_id"], 9);
    assert_eq!(body["record_id"], 1234);
    assert_eq!(body["modality"], "Image");
    assert_eq!(body["algorithm"], "imgfprint-multihash-v1");
    assert!(body["fingerprint_bytes"].as_u64().unwrap() > 0);
    assert_eq!(body["has_embedding"], false);
}

#[cfg(feature = "image")]
#[tokio::test]
async fn ingest_image_rejects_garbage_bytes() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/image/1/1")
                .body(Body::from(b"not an image".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[cfg(feature = "audio")]
#[tokio::test]
async fn ingest_audio_rejects_misaligned_body() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/audio/1/1?sample_rate=8000")
                .body(Body::from(vec![0u8, 0, 0])) // 3 bytes — not a multiple of 4
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn info_returns_format_version() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["format_version"], crate::FORMAT_VERSION);
    assert_eq!(body["crate_version"], env!("CARGO_PKG_VERSION"));
}

// ── R3 additions: describe + per-algorithm round trips ─────────────────

/// Synthesize a ~1-second 8 kHz mono sine-wave buffer as raw f32 LE
/// bytes — minimum input that the audio fingerprinters accept.
#[cfg(feature = "audio")]
fn synthetic_audio_bytes(seconds: usize, sample_rate: u32, freq_hz: f32) -> Vec<u8> {
    let n = sample_rate as usize * seconds;
    let mut out = Vec::with_capacity(n * 4);
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * freq_hz * t).sin() * 0.5;
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(feature = "text")]
#[tokio::test]
async fn describe_record_round_trip() {
    let (app, _dir) = fixture().await;

    // Ingest a text record first.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/12/345")
                .body(Body::from("describe me please"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // GET the metadata view.
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/records/12/345")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["tenant_id"], 12);
    assert_eq!(body["record_id"], 345);
    assert_eq!(body["modality"], "Text");
    assert_eq!(body["algorithm"], "minhash-h128");
    assert!(body["fingerprint_bytes"].as_u64().unwrap() > 0);
    assert_eq!(body["has_embedding"], false);
}

#[tokio::test]
async fn describe_record_returns_404_for_missing() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/records/9999/8888")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[cfg(feature = "audio-panako")]
#[tokio::test]
async fn ingest_audio_panako_round_trip() {
    let (app, _dir) = fixture().await;
    // Panako requires exactly 8 kHz input (PANAKO_SR constant in audiofp).
    let body = synthetic_audio_bytes(2, 8_000, 440.0);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/audio/1/1?sample_rate=8000&algorithm=panako")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "audiofp-panako-v1");
}

#[cfg(feature = "audio-haitsma")]
#[tokio::test]
async fn ingest_audio_haitsma_round_trip() {
    let (app, _dir) = fixture().await;
    // Haitsma requires exactly 5 kHz input (HAITSMA_SR constant in audiofp).
    let body = synthetic_audio_bytes(2, 5_000, 440.0);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/audio/1/2?sample_rate=5000&algorithm=haitsma")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "audiofp-haitsma-v1");
}

#[cfg(all(feature = "audio", not(feature = "audio-neural")))]
#[tokio::test]
async fn ingest_audio_neural_returns_unsupported_without_feature() {
    let (app, _dir) = fixture().await;
    let body = synthetic_audio_bytes(1, 8_000, 440.0);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/audio/1/3?sample_rate=8000&algorithm=neural")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["error"], "unsupported");
    let msg = body["message"].as_str().unwrap();
    assert!(
        msg.contains("audio-neural"),
        "message names the feature: {msg}"
    );
}

#[cfg(feature = "image-perceptual")]
#[tokio::test]
async fn ingest_image_phash_round_trip() {
    let (app, _dir) = fixture().await;
    let png = synthetic_png(64, 64);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/image/2/1?algorithm=phash")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "imgfprint-phash-v1");
}

#[cfg(feature = "image-perceptual")]
#[tokio::test]
async fn ingest_image_dhash_round_trip() {
    let (app, _dir) = fixture().await;
    let png = synthetic_png(64, 64);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/image/2/2?algorithm=dhash")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "imgfprint-dhash-v1");
}

#[cfg(feature = "image-perceptual")]
#[tokio::test]
async fn ingest_image_ahash_round_trip() {
    let (app, _dir) = fixture().await;
    let png = synthetic_png(64, 64);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/image/2/3?algorithm=ahash")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "imgfprint-ahash-v1");
}

#[cfg(feature = "image-perceptual")]
#[tokio::test]
async fn ingest_image_multi_explicit_round_trip() {
    let (app, _dir) = fixture().await;
    let png = synthetic_png(64, 64);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/image/2/4?algorithm=multi")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "imgfprint-multihash-v1");
}

#[cfg(all(feature = "image", not(feature = "image-semantic")))]
#[tokio::test]
async fn ingest_image_semantic_returns_clean_error_without_feature() {
    let (app, _dir) = fixture().await;
    let png = synthetic_png(32, 32);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/image/2/5?algorithm=semantic")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    // The main /image route refuses semantic and points at /semantic
    // (which only exists when image-semantic is on).
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[cfg(feature = "text-simhash")]
#[tokio::test]
async fn ingest_text_simhash_tf_round_trip() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/3/1?algorithm=simhash-tf")
                .body(Body::from("hello world"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "simhash-b64-tf");
}

#[cfg(feature = "text-simhash")]
#[tokio::test]
async fn ingest_text_simhash_idf_round_trip() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/3/2?algorithm=simhash-idf")
                .body(Body::from("hello world"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "simhash-b64-idf");
}

#[cfg(feature = "text-lsh")]
#[tokio::test]
async fn ingest_text_lsh_round_trip() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/3/3?algorithm=lsh")
                .body(Body::from("hello world"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["algorithm"], "minhash-lsh-h128");
}

#[cfg(all(feature = "text", not(feature = "text-semantic-local")))]
#[tokio::test]
async fn ingest_text_semantic_local_returns_unsupported_without_feature() {
    let (app, _dir) = fixture().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/3/9?algorithm=semantic-local&model_id=ignored")
                .body(Body::from("hi"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    let body: serde_json::Value = read_json(resp).await;
    assert_eq!(body["error"], "unsupported");
    let msg = body["message"].as_str().unwrap();
    assert!(
        msg.contains("text-semantic-local"),
        "message names the feature: {msg}"
    );
}

// ── R4 additions: ServerState fixture + auth / isolation / quota / log ─

#[cfg(feature = "text")]
mod r4 {
    use super::*;
    use crate::server::{
        ApiKeyContext, ApiKeyLookup, NoopRateLimiter, NoopUsageSink, RateDecision, ServerState,
        StaticMapKey, StaticSingleKey, TenantRateLimiter, UsageSink, router_with_state,
    };

    /// Build a `ServerState<EmbeddedBackend>` with caller-chosen
    /// trait-object impls and return the auth-wrapped router. Holds onto
    /// the `TempDir` so the redb file outlives the test.
    fn fixture_with_state(
        api_keys: Arc<dyn ApiKeyLookup>,
        rate_limit: Arc<dyn TenantRateLimiter>,
        usage: Arc<dyn UsageSink>,
    ) -> (Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(EmbeddedBackend::open(dir.path().join("ucfp.redb")).unwrap());
        let state = ServerState {
            index: backend,
            api_keys,
            rate_limit,
            usage,
        };
        let app = router_with_state(state);
        (app, dir)
    }

    #[tokio::test]
    async fn protected_text_ingest_with_bearer_returns_201() {
        let token = "secret-bearer-aaa";
        let api_keys: Arc<dyn ApiKeyLookup> = Arc::new(StaticSingleKey {
            expected: token.as_bytes().to_vec(),
            tenant_id: 0,
        });
        let (app, _dir) =
            fixture_with_state(api_keys, Arc::new(NoopRateLimiter), Arc::new(NoopUsageSink));

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/ingest/text/0/1")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::from("the quick brown fox"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body: serde_json::Value = read_json(resp).await;
        assert_eq!(body["tenant_id"], 0);
        assert_eq!(body["record_id"], 1);
        assert_eq!(body["algorithm"], "minhash-h128");
    }

    #[tokio::test]
    async fn protected_text_ingest_without_bearer_returns_401() {
        let api_keys: Arc<dyn ApiKeyLookup> = Arc::new(StaticSingleKey {
            expected: b"unused-secret".to_vec(),
            tenant_id: 0,
        });
        let (app, _dir) =
            fixture_with_state(api_keys, Arc::new(NoopRateLimiter), Arc::new(NoopUsageSink));

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/ingest/text/0/1")
                    .body(Body::from("hello"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn static_map_two_tenants_are_isolated() {
        let toml = r#"
[[key]]
token = "tok-tenant-1"
tenant_id = 1
key_id = "k1"
scopes = ["ingest", "query"]

[[key]]
token = "tok-tenant-2"
tenant_id = 2
key_id = "k2"
scopes = ["ingest", "query"]
"#;
        let map = StaticMapKey::from_toml(toml).expect("toml parses");
        let api_keys: Arc<dyn ApiKeyLookup> = Arc::new(map);
        let (app, _dir) =
            fixture_with_state(api_keys, Arc::new(NoopRateLimiter), Arc::new(NoopUsageSink));

        // Tenant 1 ingests under their own URL prefix.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/ingest/text/1/100")
                    .header("authorization", "Bearer tok-tenant-1")
                    .body(Body::from("alpha bravo charlie delta"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Tenant 2 queries their own (empty) tenant partition. Storage is
        // partitioned by `tenant_id` so they should see zero hits — even
        // though tenant 1 just ingested.
        let query_req = serde_json::json!({
            "tenant_id": 2,
            "modality": "Text",
            "k": 5,
            "vector": [0.0]
        });
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/query")
                    .header("authorization", "Bearer tok-tenant-2")
                    .header("content-type", "application/json")
                    .body(json_body(query_req))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = read_json(resp).await;
        assert_eq!(
            body["hits"].as_array().unwrap().len(),
            0,
            "tenant 2 must not see tenant 1's records"
        );
    }

    /// Test-only rate limiter that allows the first N calls and denies
    /// the rest. `AtomicI64` keeps the underflow path natural.
    #[derive(Debug)]
    struct CountingRateLimiter {
        remaining: std::sync::atomic::AtomicI64,
    }

    impl CountingRateLimiter {
        fn new(allow: u32) -> Self {
            Self {
                remaining: std::sync::atomic::AtomicI64::new(allow as i64),
            }
        }
    }

    #[async_trait::async_trait]
    impl TenantRateLimiter for CountingRateLimiter {
        async fn check(
            &self,
            _ctx: &ApiKeyContext,
            _cost: u32,
        ) -> crate::error::Result<RateDecision> {
            let before = self
                .remaining
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            if before > 0 {
                Ok(RateDecision::Allow {
                    remaining: (before - 1).max(0) as u64,
                    reset_ms: 1_000,
                })
            } else {
                Ok(RateDecision::Deny {
                    retry_after_ms: 1_000,
                })
            }
        }
    }

    #[tokio::test]
    async fn rate_limiter_blocks_after_n_calls() {
        const N: u32 = 3;
        let token = "rate-limit-token";
        let api_keys: Arc<dyn ApiKeyLookup> = Arc::new(StaticSingleKey {
            expected: token.as_bytes().to_vec(),
            tenant_id: 0,
        });
        let (app, _dir) = fixture_with_state(
            api_keys,
            Arc::new(CountingRateLimiter::new(N)),
            Arc::new(NoopUsageSink),
        );

        let make_req = || {
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/0/9")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from("ratelimited"))
                .unwrap()
        };

        // First N requests must succeed.
        for i in 0..N {
            let resp = app.clone().oneshot(make_req()).await.unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::CREATED,
                "request {i} should succeed within the budget"
            );
        }
        // Request N+1 must be denied.
        let resp = app.oneshot(make_req()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry = resp
            .headers()
            .get("retry-after")
            .expect("retry-after header present");
        assert_eq!(retry.to_str().unwrap(), "1");
    }

    #[tokio::test]
    async fn log_usage_sink_writes_ndjson_line() {
        use crate::server::LogUsageSink;
        use std::time::Duration;

        let token = "log-sink-token";
        let api_keys: Arc<dyn ApiKeyLookup> = Arc::new(StaticSingleKey {
            expected: token.as_bytes().to_vec(),
            tenant_id: 0,
        });

        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("usage.ndjson");
        let sink: Arc<dyn UsageSink> =
            Arc::new(LogUsageSink::open(&log_path).expect("open log sink"));

        let (app, _backend_dir) = fixture_with_state(api_keys, Arc::new(NoopRateLimiter), sink);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/ingest/text/0/77")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::from("usage event please"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        // The middleware records via `tokio::spawn`, so the file write is
        // queued but may not have happened by the time `oneshot` returns.
        // Poll for up to ~500ms with short sleeps.
        let mut contents = String::new();
        for _ in 0..50 {
            contents = std::fs::read_to_string(&log_path).unwrap_or_default();
            if !contents.trim().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            !contents.trim().is_empty(),
            "log sink should have written at least one NDJSON line"
        );

        // First line must be valid JSON with the fields the middleware
        // populates. R3 hard-codes `op = "ingest"` for every protected
        // request — modality / algorithm classification lands in a later
        // change set, so we only assert what the middleware actually
        // emits today.
        let line = contents.lines().next().expect("one line present");
        let v: serde_json::Value = serde_json::from_str(line).expect("first line parses as JSON");
        assert_eq!(v["tenant_id"], 0);
        assert_eq!(v["key_id"], "static-single");
        assert_eq!(v["op"], "ingest");
        assert_eq!(v["status"], 201);
        assert!(v["ts"].is_number(), "ts is unix-ms u64");
    }
}

// ── Cross-tenant isolation ─────────────────────────────────────────────

/// Build a `router_with_state` wired with a two-entry `StaticMapKey` so
/// we can test the tenant-isolation guard introduced alongside this test.
async fn multi_tenant_fixture() -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(EmbeddedBackend::open(dir.path().join("ucfp.redb")).unwrap());
    let keys = StaticMapKey::from_toml(
        r#"
[[key]]
token = "key-t10"
tenant_id = 10
key_id = "k10"

[[key]]
token = "key-t20"
tenant_id = 20
key_id = "k20"
"#,
    )
    .unwrap();
    let state = ServerState {
        index: backend,
        api_keys: Arc::new(keys) as Arc<dyn ApiKeyLookup>,
        rate_limit: Arc::new(InMemoryTokenBucket::with_limits(1000, 2000)),
        usage: Arc::new(NoopUsageSink),
    };
    let app = router_with_state(state);
    (app, dir)
}

#[cfg(feature = "text")]
#[tokio::test]
async fn cross_tenant_read_is_forbidden() {
    let (app, _dir) = multi_tenant_fixture().await;

    // Tenant 10 ingests a record.
    let ingest = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/10/1")
                .header("Authorization", "Bearer key-t10")
                .header("Content-Type", "text/plain")
                .body(Body::from("hello from tenant 10"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ingest.status(), StatusCode::CREATED, "ingest must succeed");

    // Tenant 20's key must NOT be able to describe tenant 10's record.
    let cross = app
        .oneshot(
            Request::builder()
                .uri("/v1/records/10/1")
                .header("Authorization", "Bearer key-t20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        cross.status(),
        StatusCode::FORBIDDEN,
        "cross-tenant describe must return 403"
    );
    let body: serde_json::Value = read_json(cross).await;
    assert_eq!(body["error"], "forbidden");
}

#[cfg(feature = "inspect")]
#[tokio::test]
async fn input_cache_roundtrip_then_text_ingest_via_input_id() {
    let (app, _dir) = fixture().await;

    // Cache a payload for tenant 9.
    let put = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/inputs?tenant_id=9&modality=text")
                .header("content-type", "text/plain; charset=utf-8")
                .body(Body::from("the quick brown fox jumps over the lazy dog"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put.status(), StatusCode::OK);
    let put_body: serde_json::Value = read_json(put).await;
    let input_id = put_body["input_id"].as_u64().expect("input_id");
    assert_eq!(put_body["tenant_id"], 9);
    assert_eq!(put_body["size_bytes"], 43);

    // Ingest using input_id — empty body, all data comes from cache.
    let ing = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/ingest/text/9/77?input_id={input_id}"))
                .header("content-type", "text/plain; charset=utf-8")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ing.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(ing).await;
    assert_eq!(body["tenant_id"], 9);
    assert_eq!(body["record_id"], 77);
    assert_eq!(body["algorithm"], "minhash-h128");

    // DELETE removes the entry; second delete is 404.
    let del = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/inputs/9/{input_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);
    let del2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/inputs/9/{input_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del2.status(), StatusCode::NOT_FOUND);

    // Ingest using a non-existent input_id is a 4xx.
    let bad = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/9/78?input_id=999999999")
                .header("content-type", "text/plain; charset=utf-8")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(bad.status().is_client_error());
}


#[cfg(feature = "inspect")]
#[tokio::test]
async fn pipeline_inspect_text_returns_each_stage() {
    let (app, _dir) = fixture().await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pipeline/inspect/text/0?k=3&tokenizer=word")
                .header("content-type", "text/plain; charset=utf-8")
                .body(Body::from("Hello, World! Hello again."))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(resp).await;

    // Stable algorithm tag.
    assert_eq!(body["algorithm"], "minhash-h128");
    // Canonicalization lowercases the input.
    let canon = body["canonicalized"].as_str().unwrap();
    assert!(canon.starts_with("hello"), "expected lowercase canonicalized: {canon}");
    // Token list is non-empty and contains expected tokens.
    let tokens = body["tokens"].as_array().unwrap();
    assert!(!tokens.is_empty());
    assert!(tokens.iter().any(|t| t == "hello"));
    // Shingle list is non-empty for k=3.
    let shingles = body["shingles"].as_array().unwrap();
    assert!(!shingles.is_empty());
    // MinHash<128> is repr(C) { schema:u16, _pad:[u8;6], hashes:[u64;128] }
    // → 8 + 8*128 = 1032 bytes (= 2064 hex chars).
    let fp = body["fingerprint_hex"].as_str().unwrap();
    assert_eq!(fp.len(), 2064);
    assert_eq!(body["fingerprint_bytes"], 1032);
}


// Regression contract: a no-opts MinHash<128> fingerprint of a known
// fixed input must produce the same bytes after refactors. Catches
// silent canonicalizer / shingle / hasher drift that the looser
// "fingerprint_bytes > 0" assertions in `ingest_text_round_trip` miss.
//
// Golden values were captured from the v0.2 hash family (Xxh3_64) +
// txtfp 0.2.0 NFKC canonicalizer + shingle k=5 / Word tokenizer. Bump
// these intentionally if the upstream contract genuinely changes; an
// accidental change here means a regression worth investigating.
#[cfg(feature = "text")]
#[tokio::test]
async fn golden_text_minhash_no_opts_is_stable() {
    let (app, _dir) = fixture().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/text/0/1")
                .header("content-type", "text/plain; charset=utf-8")
                .body(Body::from("the quick brown fox jumps over the lazy dog"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: serde_json::Value = read_json(resp).await;
    let hex = body["fingerprint_hex"].as_str().expect("fingerprint_hex");
    // Schema header (8 B) + first slot (8 B) — the rest of the 1024-byte
    // signature is implicitly covered through the assertion that this
    // prefix is stable: any drift in canonicalizer / tokenizer / hasher
    // shifts the very first hash slot.
    assert_eq!(
        &hex[..32],
        "0100000000000000a26accc88c8a8106",
        "MinHash regression — first 16 bytes (schema + first slot) drifted",
    );
    assert_eq!(
        body["config_hash"], 2_212_816_233_060_047_056_u64,
        "MinHash regression — config_hash drifted (canonicalizer or tokenizer changed?)",
    );
    assert_eq!(body["fingerprint_bytes"], 1032);
}


#[cfg(all(feature = "inspect", feature = "image"))]
#[tokio::test]
async fn pipeline_inspect_image_returns_each_stage() {
    let (app, _dir) = fixture().await;

    let png = synthetic_png(64, 64);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/pipeline/inspect/image/0")
                .header("content-type", "image/png")
                .body(Body::from(png))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = read_json(resp).await;

    assert_eq!(body["algorithm"], "imgfprint-multihash-v1");
    assert_eq!(body["width"], 64);
    assert_eq!(body["height"], 64);
    // Every base64 stage is non-empty and looks PNG-shaped (starts with the
    // base64 of the PNG magic byte 0x89).
    let original = body["original_png_b64"].as_str().unwrap();
    assert!(original.starts_with("iVBORw0KGgo"), "original_png_b64 missing PNG header: {}", &original[..16]);
    let g32 = body["gray32_png_b64"].as_str().unwrap();
    assert!(g32.starts_with("iVBORw0KGgo"));
    let g8 = body["gray8_png_b64"].as_str().unwrap();
    assert!(g8.starts_with("iVBORw0KGgo"));
    // AHash mean is in valid u8 range and non-zero for the synthetic
    // colour-ramp test image.
    let mean = body["ahash_mean"].as_u64().unwrap();
    assert!(mean > 0 && mean < 256);
    // Final fingerprint matches the multi-hash bundle shape (536 bytes).
    assert_eq!(body["fingerprint_bytes"], 536);
    assert_eq!(body["fingerprint_hex"].as_str().unwrap().len(), 1072);
}

