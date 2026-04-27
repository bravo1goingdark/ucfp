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

use super::router;

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
