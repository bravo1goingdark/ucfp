//! Route handlers. Each one is `pub(super)` so the router builder in
//! `mod.rs` can register it without leaking the implementation.

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::Json,
};

use crate::core::{HitSource, Query, Record};
use crate::error::Error;
use crate::index::IndexBackend;
use crate::matcher::Matcher;

use super::apikey::ApiKeyContext;
use super::dto::{
    FingerprintDescription, HitOut, InfoResponse, QueryRequest, QueryResponse, RecordIn,
    UpsertRequest, UpsertResponse,
};
use super::error::ApiError;

// Imports only the ingest handlers need — feature-gated so a build
// with all three modality features off doesn't warn.
#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
use super::dto::IngestResponse;
#[cfg(feature = "audio")]
use super::dto::{AudioAlgorithm, AudioParams};
#[cfg(feature = "image")]
use super::dto::{ImageAlgorithm, ImageParams};
#[cfg(feature = "text")]
use super::dto::{TextAlgorithm, TextParams};
#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
use axum::body::Bytes;
#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
use axum::extract::Query as Qs;

/// Enforce that the authenticated key's tenant matches the path tenant.
///
/// A `tenant_id` of 0 in the `ApiKeyContext` is the service-bearer sentinel
/// (`StaticSingleKey` / `UCFP_TOKEN`), which is trusted to supply any
/// tenant in the path (SvelteKit→Rust hop). For every other key the path
/// tenant must match the key tenant exactly.
///
/// When no `ApiKeyContext` is present in extensions (test router path that
/// has no auth layer), the check is skipped entirely.
fn tenant_guard(ctx: Option<Extension<ApiKeyContext>>, path_tenant: u32) -> Result<(), ApiError> {
    if let Some(Extension(ctx)) = ctx
        && ctx.tenant_id != 0
        && ctx.tenant_id != path_tenant
    {
        return Err(Error::Forbidden {
            key_tenant: ctx.tenant_id,
            path_tenant,
        }
        .into());
    }
    Ok(())
}

#[cfg(feature = "audio-watermark")]
use super::dto::WatermarkReport as WatermarkReportDto;

// ── GET /healthz ───────────────────────────────────────────────────────

/// Ping the index — a 200 means "process is up AND the DB is reachable".
/// A non-200 (5xx via `ApiError`) signals the orchestrator to stop
/// routing here; the underlying error variant determines which code.
pub(super) async fn healthz<I: IndexBackend>(
    State(index): State<Arc<I>>,
) -> Result<&'static str, ApiError> {
    index.flush().await?;
    Ok("ok")
}

// ── GET /v1/info ───────────────────────────────────────────────────────

pub(super) async fn info() -> Json<InfoResponse> {
    Json(InfoResponse {
        format_version: crate::FORMAT_VERSION,
        crate_version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// ── POST /v1/records ───────────────────────────────────────────────────

pub(super) async fn upsert<I: IndexBackend>(
    State(index): State<Arc<I>>,
    Json(req): Json<UpsertRequest>,
) -> Result<Json<UpsertResponse>, ApiError> {
    let count = req.records.len();
    let records: Vec<Record> = req.records.into_iter().map(RecordIn::into).collect();
    index.upsert(&records).await?;
    Ok(Json(UpsertResponse { upserted: count }))
}

// ── DELETE /v1/records/{tenant_id}/{record_id} ─────────────────────────

pub(super) async fn delete_record<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
) -> Result<StatusCode, ApiError> {
    tenant_guard(ctx, tenant_id)?;
    index.delete(tenant_id, &[record_id]).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /v1/records/{tenant_id}/{record_id} ────────────────────────────

pub(super) async fn describe_record<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
) -> Result<Json<FingerprintDescription>, ApiError> {
    tenant_guard(ctx, tenant_id)?;
    let meta = index.get_record_metadata(tenant_id, record_id).await?;
    Ok(Json(meta.into()))
}

// ── POST /v1/query ─────────────────────────────────────────────────────

pub(super) async fn query<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    tenant_guard(ctx, req.tenant_id)?;
    let q = Query {
        tenant_id: req.tenant_id,
        modality: req.modality,
        k: req.k.max(1),
        vector: Some(req.vector),
        terms: Vec::new(),
        filter: None,
        rrf_k: 60,
    };
    let matcher = Matcher::new(index.as_ref());
    let hits = matcher.search(&q).await?;

    let hits = hits
        .into_iter()
        .map(|h| HitOut {
            tenant_id: h.tenant_id,
            record_id: h.record_id,
            score: h.score,
            source: hit_source_str(h.source),
        })
        .collect();
    Ok(Json(QueryResponse { hits }))
}

fn hit_source_str(s: HitSource) -> &'static str {
    match s {
        HitSource::Vector => "vector",
        HitSource::Bm25 => "bm25",
        HitSource::Filter => "filter",
        HitSource::Reranker => "reranker",
        HitSource::Fused => "fused",
    }
}

// ── POST /v1/ingest/* ──────────────────────────────────────────────────
//
// Each modality-specific ingest route takes the raw bytes, dispatches on
// the requested algorithm, and upserts a fully-formed Record. Clients no
// longer need to compute fingerprints themselves.
//
// Algorithms whose feature flag is off return `Error::Unsupported` (501)
// with a message naming the missing flag, so operators can wire up the
// correct build without trial-and-error.

#[cfg(any(feature = "audio", feature = "image", feature = "text"))]
fn ingest_response(rec: &Record) -> IngestResponse {
    IngestResponse {
        tenant_id: rec.tenant_id,
        record_id: rec.record_id,
        modality: rec.modality,
        format_version: rec.format_version,
        algorithm: rec.algorithm.clone(),
        config_hash: rec.config_hash,
        fingerprint_bytes: rec.fingerprint.len(),
        fingerprint_hex: rec.fingerprint.iter().map(|b| format!("{b:02x}")).collect(),
        has_embedding: rec.embedding.is_some(),
    }
}

// ── Image ingest ───────────────────────────────────────────────────────

#[cfg(feature = "image")]
pub(super) async fn ingest_image<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    Qs(params): Qs<ImageParams>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    tenant_guard(ctx, tenant_id)?;
    let rec = match params.algorithm {
        ImageAlgorithm::Multi => crate::modality::image::fingerprint(&body, tenant_id, record_id)?,
        ImageAlgorithm::Phash => {
            #[cfg(feature = "image-perceptual")]
            {
                crate::modality::image::fingerprint_phash(
                    &body,
                    &imgfprint::PreprocessConfig::default(),
                    tenant_id,
                    record_id,
                )?
            }
            #[cfg(not(feature = "image-perceptual"))]
            return Err(Error::Unsupported(
                "image phash requires feature `image-perceptual`".into(),
            )
            .into());
        }
        ImageAlgorithm::Dhash => {
            #[cfg(feature = "image-perceptual")]
            {
                crate::modality::image::fingerprint_dhash(
                    &body,
                    &imgfprint::PreprocessConfig::default(),
                    tenant_id,
                    record_id,
                )?
            }
            #[cfg(not(feature = "image-perceptual"))]
            return Err(Error::Unsupported(
                "image dhash requires feature `image-perceptual`".into(),
            )
            .into());
        }
        ImageAlgorithm::Ahash => {
            #[cfg(feature = "image-perceptual")]
            {
                crate::modality::image::fingerprint_ahash(
                    &body,
                    &imgfprint::PreprocessConfig::default(),
                    tenant_id,
                    record_id,
                )?
            }
            #[cfg(not(feature = "image-perceptual"))]
            return Err(Error::Unsupported(
                "image ahash requires feature `image-perceptual`".into(),
            )
            .into());
        }
        ImageAlgorithm::Semantic => {
            return Err(Error::Modality(
                "use POST /v1/ingest/image/{tid}/{rid}/semantic for semantic embeddings".into(),
            )
            .into());
        }
    };
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}

/// `POST /v1/ingest/image/{tid}/{rid}/semantic` — CLIP-style ONNX vector.
#[cfg(feature = "image-semantic")]
pub(super) async fn ingest_image_semantic<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    Qs(params): Qs<ImageParams>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    tenant_guard(ctx, tenant_id)?;
    let model = params
        .model_id
        .as_deref()
        .ok_or_else(|| Error::Modality("image semantic requires `model_id`".into()))?;
    let rec = crate::modality::image::fingerprint_semantic(
        &body,
        &imgfprint::PreprocessConfig::default(),
        model,
        tenant_id,
        record_id,
    )?;
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}

// ── Text ingest ────────────────────────────────────────────────────────

#[cfg(feature = "text")]
pub(super) async fn ingest_text<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    Qs(params): Qs<TextParams>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    tenant_guard(ctx, tenant_id)?;
    let text = std::str::from_utf8(&body)
        .map_err(|e| Error::Modality(format!("body is not valid UTF-8: {e}")))?;
    let opts = build_text_opts(&params);
    let rec =
        match params.algorithm {
            TextAlgorithm::Minhash => crate::modality::text::fingerprint_minhash_with::<
                { crate::modality::text::DEFAULT_H },
            >(text, &opts, tenant_id, record_id)?,
            TextAlgorithm::SimhashTf => {
                #[cfg(feature = "text-simhash")]
                {
                    crate::modality::text::fingerprint_simhash_tf(
                        text, &opts, tenant_id, record_id,
                    )?
                }
                #[cfg(not(feature = "text-simhash"))]
                return Err(Error::Unsupported(
                    "simhash-tf requires feature `text-simhash`".into(),
                )
                .into());
            }
            TextAlgorithm::SimhashIdf => {
                #[cfg(feature = "text-simhash")]
                {
                    let idf = txtfp::IdfTable::default();
                    crate::modality::text::fingerprint_simhash_idf(
                        text, &opts, &idf, tenant_id, record_id,
                    )?
                }
                #[cfg(not(feature = "text-simhash"))]
                return Err(Error::Unsupported(
                    "simhash-idf requires feature `text-simhash`".into(),
                )
                .into());
            }
            TextAlgorithm::Lsh => {
                #[cfg(feature = "text-lsh")]
                {
                    crate::modality::text::fingerprint_lsh(text, &opts, tenant_id, record_id)?
                }
                #[cfg(not(feature = "text-lsh"))]
                return Err(Error::Unsupported("lsh requires feature `text-lsh`".into()).into());
            }
            TextAlgorithm::Tlsh => {
                #[cfg(feature = "text-tlsh")]
                {
                    crate::modality::text::fingerprint_tlsh(text, &opts, tenant_id, record_id)?
                }
                #[cfg(not(feature = "text-tlsh"))]
                return Err(Error::Unsupported("tlsh requires feature `text-tlsh`".into()).into());
            }
            TextAlgorithm::SemanticLocal => {
                #[cfg(feature = "text-semantic-local")]
                {
                    let model = params.model_id.as_deref().ok_or_else(|| {
                        Error::Modality("semantic-local requires `model_id`".into())
                    })?;
                    crate::modality::text::fingerprint_semantic_local(
                        text, model, tenant_id, record_id,
                    )?
                }
                #[cfg(not(feature = "text-semantic-local"))]
                return Err(Error::Unsupported(
                    "semantic-local requires feature `text-semantic-local`".into(),
                )
                .into());
            }
            TextAlgorithm::SemanticOpenai => {
                #[cfg(feature = "text-semantic-openai")]
                {
                    let model = params.model_id.as_deref().ok_or_else(|| {
                        Error::Modality("semantic-openai requires `model_id`".into())
                    })?;
                    let api_key = params.api_key.as_deref().ok_or_else(|| {
                        Error::Modality("semantic-openai requires `api_key`".into())
                    })?;
                    crate::modality::text::fingerprint_semantic_openai(
                        text, model, api_key, tenant_id, record_id,
                    )?
                }
                #[cfg(not(feature = "text-semantic-openai"))]
                return Err(Error::Unsupported(
                    "semantic-openai requires feature `text-semantic-openai`".into(),
                )
                .into());
            }
            TextAlgorithm::SemanticVoyage => {
                #[cfg(feature = "text-semantic-voyage")]
                {
                    let model = params.model_id.as_deref().ok_or_else(|| {
                        Error::Modality("semantic-voyage requires `model_id`".into())
                    })?;
                    let api_key = params.api_key.as_deref().ok_or_else(|| {
                        Error::Modality("semantic-voyage requires `api_key`".into())
                    })?;
                    crate::modality::text::fingerprint_semantic_voyage(
                        text, model, api_key, tenant_id, record_id,
                    )?
                }
                #[cfg(not(feature = "text-semantic-voyage"))]
                return Err(Error::Unsupported(
                    "semantic-voyage requires feature `text-semantic-voyage`".into(),
                )
                .into());
            }
            TextAlgorithm::SemanticCohere => {
                #[cfg(feature = "text-semantic-cohere")]
                {
                    let model = params.model_id.as_deref().ok_or_else(|| {
                        Error::Modality("semantic-cohere requires `model_id`".into())
                    })?;
                    let api_key = params.api_key.as_deref().ok_or_else(|| {
                        Error::Modality("semantic-cohere requires `api_key`".into())
                    })?;
                    crate::modality::text::fingerprint_semantic_cohere(
                        text, model, api_key, tenant_id, record_id,
                    )?
                }
                #[cfg(not(feature = "text-semantic-cohere"))]
                return Err(Error::Unsupported(
                    "semantic-cohere requires feature `text-semantic-cohere`".into(),
                )
                .into());
            }
        };
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}

#[cfg(feature = "text")]
fn build_text_opts(params: &TextParams) -> crate::modality::text::TextOpts {
    use super::dto::{PreprocessKind as DtoPre, TokenizerKind as DtoTok};
    use crate::modality::text::{TextOpts, TokenizerKind as ModTok};
    let mut opts = TextOpts::default();
    if let Some(k) = params.k {
        opts.k = k;
    }
    if let Some(h) = params.h {
        opts.h = h;
    }
    if let Some(t) = params.tokenizer {
        opts.tokenizer = match t {
            DtoTok::Word => ModTok::Word,
            DtoTok::Grapheme => ModTok::Grapheme,
            DtoTok::CjkJp => ModTok::CjkJp,
            DtoTok::CjkKo => ModTok::CjkKo,
        };
    }
    if let Some(p) = params.preprocess {
        opts.preprocess = Some(match p {
            DtoPre::Html => crate::modality::text::PreprocessKind::Html,
            DtoPre::Markdown => crate::modality::text::PreprocessKind::Markdown,
            DtoPre::Pdf => crate::modality::text::PreprocessKind::Pdf,
        });
    }
    opts
}

/// `POST /v1/ingest/text/{tid}/{rid}/stream` — NDJSON push streaming.
#[cfg(feature = "text-streaming")]
pub(super) async fn ingest_text_stream<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    Qs(params): Qs<TextParams>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    tenant_guard(ctx, tenant_id)?;
    let opts = build_text_opts(&params);
    let mut session =
        crate::modality::text::StreamingMinHashSession::new(&opts, tenant_id, record_id);
    // NDJSON shape: each line is a JSON string carrying a UTF-8 chunk.
    // Empty lines are skipped. Non-string payloads are 400.
    for line in body.split(|b| *b == b'\n') {
        let trimmed: &[u8] = line.strip_suffix(b"\r").unwrap_or(line);
        if trimmed.is_empty() {
            continue;
        }
        let chunk: String = serde_json::from_slice(trimmed)
            .map_err(|e| Error::Modality(format!("NDJSON line: {e}")))?;
        session.push(chunk.as_bytes())?;
    }
    let mut records = session.finalize()?;
    let rec = records
        .pop()
        .ok_or_else(|| Error::Modality("streaming session produced no record".into()))?;
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}

/// `POST /v1/ingest/text/{tid}/{rid}/preprocess/{kind}` — preprocess
/// HTML/Markdown/PDF inputs to plain text and ingest as MinHash.
#[cfg(any(feature = "text-markup", feature = "text-pdf"))]
pub(super) async fn ingest_text_preprocess<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id, kind)): Path<(u32, u64, String)>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    tenant_guard(ctx, tenant_id)?;
    use crate::modality::text::{PreprocessKind, TextOpts};
    let preprocess_kind = match kind.as_str() {
        "html" => {
            #[cfg(not(feature = "text-markup"))]
            return Err(Error::Unsupported(
                "html preprocess requires feature `text-markup`".into(),
            )
            .into());
            #[cfg(feature = "text-markup")]
            PreprocessKind::Html
        }
        "markdown" => {
            #[cfg(not(feature = "text-markup"))]
            return Err(Error::Unsupported(
                "markdown preprocess requires feature `text-markup`".into(),
            )
            .into());
            #[cfg(feature = "text-markup")]
            PreprocessKind::Markdown
        }
        "pdf" => {
            #[cfg(not(feature = "text-pdf"))]
            return Err(
                Error::Unsupported("pdf preprocess requires feature `text-pdf`".into()).into(),
            );
            #[cfg(feature = "text-pdf")]
            PreprocessKind::Pdf
        }
        other => {
            return Err(Error::Modality(format!(
                "unknown preprocess kind `{other}` (want html|markdown|pdf)"
            ))
            .into());
        }
    };
    let opts = TextOpts {
        preprocess: Some(preprocess_kind),
        ..TextOpts::default()
    };
    // PDF inputs are binary; the text fingerprint helpers accept `&str`,
    // so we pass an empty wrapper and let the preprocess pass run on the
    // raw bytes via `text.as_bytes()` in `preprocess_pdf`. For HTML and
    // Markdown the body is already UTF-8.
    let text = if matches!(preprocess_kind, PreprocessKind::Pdf) {
        // Safety: the PDF preprocess re-reads `text.as_bytes()` and the
        // SDK doesn't require valid UTF-8 from the caller — but `&str`
        // requires it. Smuggle bytes via `from_utf8_lossy`; the lossy
        // replacement only matters when the SDK actually reads `text`,
        // which it doesn't for the PDF path.
        std::borrow::Cow::Owned(String::from_utf8_lossy(&body).into_owned())
    } else {
        std::borrow::Cow::Borrowed(
            std::str::from_utf8(&body)
                .map_err(|e| Error::Modality(format!("body is not valid UTF-8: {e}")))?,
        )
    };
    let rec = crate::modality::text::fingerprint_minhash_with::<
        { crate::modality::text::DEFAULT_H },
    >(text.as_ref(), &opts, tenant_id, record_id)?;
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}

// ── Audio ingest ───────────────────────────────────────────────────────

#[cfg(feature = "audio")]
pub(super) async fn ingest_audio<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    Qs(params): Qs<AudioParams>,
    body: Bytes,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    tenant_guard(ctx, tenant_id)?;
    if !body.len().is_multiple_of(4) {
        return Err(Error::Modality(format!(
            "audio body must be a multiple of 4 bytes (raw f32 LE samples), got {}",
            body.len()
        ))
        .into());
    }
    // Decode body bytes → Vec<f32> via explicit little-endian conversion.
    // Avoids alignment concerns from a direct `bytemuck::cast_slice` on
    // arbitrary heap buffers across platforms.
    let samples: Vec<f32> = body
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    let rec = match params.algorithm {
        AudioAlgorithm::Wang => crate::modality::audio::fingerprint_wang(
            &samples,
            params.sample_rate,
            tenant_id,
            record_id,
        )?,
        AudioAlgorithm::Panako => {
            #[cfg(feature = "audio-panako")]
            {
                crate::modality::audio::fingerprint_panako(
                    &samples,
                    params.sample_rate,
                    tenant_id,
                    record_id,
                )?
            }
            #[cfg(not(feature = "audio-panako"))]
            return Err(Error::Unsupported("panako requires feature `audio-panako`".into()).into());
        }
        AudioAlgorithm::Haitsma => {
            #[cfg(feature = "audio-haitsma")]
            {
                crate::modality::audio::fingerprint_haitsma(
                    &samples,
                    params.sample_rate,
                    tenant_id,
                    record_id,
                )?
            }
            #[cfg(not(feature = "audio-haitsma"))]
            return Err(
                Error::Unsupported("haitsma requires feature `audio-haitsma`".into()).into(),
            );
        }
        AudioAlgorithm::Neural => {
            #[cfg(feature = "audio-neural")]
            {
                let model = params
                    .model_id
                    .as_deref()
                    .ok_or_else(|| Error::Modality("neural requires `model_id`".into()))?;
                crate::modality::audio::fingerprint_neural(
                    &samples,
                    params.sample_rate,
                    model,
                    tenant_id,
                    record_id,
                )?
            }
            #[cfg(not(feature = "audio-neural"))]
            return Err(Error::Unsupported("neural requires feature `audio-neural`".into()).into());
        }
        AudioAlgorithm::Watermark => {
            return Err(Error::Modality(
                "use POST /v1/ingest/audio/{tid}/{rid}/watermark for detection".into(),
            )
            .into());
        }
    };
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}

/// `POST /v1/ingest/audio/{tid}/{rid}/watermark` — runs the AudioSeal
/// detector and returns its report. Does not upsert.
#[cfg(feature = "audio-watermark")]
pub(super) async fn ingest_audio_watermark<I: IndexBackend>(
    State(_index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, _record_id)): Path<(u32, u64)>,
    Qs(params): Qs<AudioParams>,
    body: Bytes,
) -> Result<(StatusCode, Json<WatermarkReportDto>), ApiError> {
    tenant_guard(ctx, tenant_id)?;
    if !body.len().is_multiple_of(4) {
        return Err(Error::Modality(format!(
            "audio body must be a multiple of 4 bytes (raw f32 LE samples), got {}",
            body.len()
        ))
        .into());
    }
    let samples: Vec<f32> = body
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let model = params
        .model_id
        .as_deref()
        .ok_or_else(|| Error::Modality("watermark requires `model_id`".into()))?;
    let report = crate::modality::audio::detect_watermark(&samples, params.sample_rate, model)?;
    Ok((StatusCode::OK, Json(WatermarkReportDto::from(report))))
}

/// `POST /v1/ingest/audio/{tid}/{rid}/stream` — multipart form streaming.
///
/// Accepts a single `audio` part containing raw f32 LE samples. The
/// part is consumed in chunks; each chunk is pushed into a
/// [`crate::modality::audio::StreamingWangSession`] and the records
/// emitted across all chunks are merged into a single upsert.
#[cfg(all(feature = "audio-streaming", feature = "multipart"))]
pub(super) async fn ingest_audio_stream<I: IndexBackend>(
    State(index): State<Arc<I>>,
    ctx: Option<Extension<ApiKeyContext>>,
    Path((tenant_id, record_id)): Path<(u32, u64)>,
    Qs(params): Qs<AudioParams>,
    mut multipart: axum::extract::Multipart,
) -> Result<(StatusCode, Json<IngestResponse>), ApiError> {
    tenant_guard(ctx, tenant_id)?;
    let mut session = crate::modality::audio::StreamingWangSession::new(
        params.sample_rate,
        tenant_id,
        record_id,
    )?;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::Modality(format!("multipart: {e}")))?
    {
        let bytes = field
            .bytes()
            .await
            .map_err(|e| Error::Modality(format!("multipart read: {e}")))?;
        if !bytes.len().is_multiple_of(4) {
            return Err(Error::Modality(format!(
                "audio chunk must be a multiple of 4 bytes (raw f32 LE samples), got {}",
                bytes.len()
            ))
            .into());
        }
        let samples: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        session.push(&samples)?;
    }
    let mut records = session.finalize()?;
    let rec = records
        .pop()
        .ok_or_else(|| Error::Modality("streaming session produced no record".into()))?;
    index.upsert(std::slice::from_ref(&rec)).await?;
    Ok((StatusCode::CREATED, Json(ingest_response(&rec))))
}
