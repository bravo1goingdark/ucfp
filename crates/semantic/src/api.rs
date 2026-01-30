use serde_json::{json, Value};
use std::time::Duration;
use ureq::AgentBuilder;

use crate::normalize::l2_normalize_in_place;
use crate::{SemanticConfig, SemanticEmbedding, SemanticError};

#[derive(Clone, Copy)]
enum ApiProviderKind {
    HuggingFace,
    OpenAI,
    Custom,
}

/// Handles the API-based embedding generation.
pub(crate) fn semanticize_via_api(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> Result<SemanticEmbedding, SemanticError> {
    let url = cfg
        .api_url
        .as_deref()
        .ok_or_else(|| SemanticError::InvalidConfig("api_url is required for api mode".into()))?;
    let provider = api_provider_kind(cfg);
    let payload_text = vec![text.to_string()];
    // Build the API payload according to the provider's expected format.
    let payload = build_api_payload(provider, &payload_text, cfg, false);
    // Send the request and parse the response.
    let response = send_api_request(url, cfg, payload)?;
    let mut vectors = parse_embeddings_from_value(response)?;
    let mut embedding = vectors
        .pop()
        .or_else(|| vectors.into_iter().next())
        .ok_or_else(|| {
            SemanticError::Inference("API response did not contain embeddings".into())
        })?;

    if cfg.normalize {
        l2_normalize_in_place(&mut embedding);
    }

    let embedding_dim = embedding.len();

    Ok(SemanticEmbedding {
        doc_id: doc_id.to_string(),
        vector: embedding,
        model_name: cfg.model_name.clone(),
        tier: cfg.tier.clone(),
        embedding_dim,
        normalized: cfg.normalize,
    })
}

/// Handles batch API-based embedding generation.
pub(crate) fn semanticize_batch_via_api<D, T>(
    docs: &[(D, T)],
    cfg: &SemanticConfig,
) -> Result<Vec<SemanticEmbedding>, SemanticError>
where
    D: AsRef<str>,
    T: AsRef<str>,
{
    if docs.is_empty() {
        return Ok(Vec::new());
    }

    let url = cfg
        .api_url
        .as_deref()
        .ok_or_else(|| SemanticError::InvalidConfig("api_url is required for api mode".into()))?;
    let provider = api_provider_kind(cfg);

    let doc_ids: Vec<String> = docs
        .iter()
        .map(|(doc_id, _)| doc_id.as_ref().to_owned())
        .collect();
    let texts: Vec<String> = docs
        .iter()
        .map(|(_, text)| text.as_ref().to_owned())
        .collect();

    let payload = build_api_payload(provider, &texts, cfg, true);
    let vectors = parse_embeddings_from_value(send_api_request(url, cfg, payload)?)?;

    if vectors.len() != doc_ids.len() {
        return Err(SemanticError::Inference(format!(
            "API returned {} embeddings for {} inputs",
            vectors.len(),
            doc_ids.len()
        )));
    }

    let mut results = Vec::with_capacity(doc_ids.len());
    for (doc_id, mut vector) in doc_ids.into_iter().zip(vectors.into_iter()) {
        if cfg.normalize {
            l2_normalize_in_place(&mut vector);
        }
        let embedding_dim = vector.len();
        results.push(SemanticEmbedding {
            doc_id,
            vector,
            model_name: cfg.model_name.clone(),
            tier: cfg.tier.clone(),
            embedding_dim,
            normalized: cfg.normalize,
        });
    }

    Ok(results)
}

fn api_provider_kind(cfg: &SemanticConfig) -> ApiProviderKind {
    let provider = cfg
        .api_provider
        .as_deref()
        .unwrap_or("custom")
        .to_ascii_lowercase();
    match provider.as_str() {
        "hf" | "huggingface" => ApiProviderKind::HuggingFace,
        "openai" | "gpt" => ApiProviderKind::OpenAI,
        _ => ApiProviderKind::Custom,
    }
}

fn build_api_payload(
    provider: ApiProviderKind,
    texts: &[String],
    cfg: &SemanticConfig,
    batch: bool,
) -> Value {
    match provider {
        ApiProviderKind::HuggingFace => {
            if batch {
                json!({ "inputs": texts })
            } else if let Some(first) = texts.first() {
                json!({ "inputs": first })
            } else {
                json!({ "inputs": "" })
            }
        }
        ApiProviderKind::OpenAI => {
            if batch {
                json!({ "input": texts, "model": cfg.model_name })
            } else if let Some(first) = texts.first() {
                json!({ "input": first, "model": cfg.model_name })
            } else {
                json!({ "input": "", "model": cfg.model_name })
            }
        }
        ApiProviderKind::Custom => {
            if batch {
                json!({ "texts": texts })
            } else if let Some(first) = texts.first() {
                json!({ "text": first })
            } else {
                json!({ "text": "" })
            }
        }
    }
}

fn send_api_request(
    url: &str,
    cfg: &SemanticConfig,
    payload: Value,
) -> Result<Value, SemanticError> {
    let agent = api_agent(cfg);
    let mut request = agent.post(url);
    request = request.set("Content-Type", "application/json");
    if let Some(header) = cfg.api_auth_header.as_deref() {
        request = request.set("Authorization", header);
    }

    let payload_body = payload.to_string();
    let response = request
        .send_string(&payload_body)
        .map_err(|e| SemanticError::Download(e.to_string()))?;

    let body = response
        .into_string()
        .map_err(|e| SemanticError::Download(e.to_string()))?;
    serde_json::from_str(&body).map_err(|e| SemanticError::Inference(e.to_string()))
}

fn api_agent(cfg: &SemanticConfig) -> ureq::Agent {
    let timeout = Duration::from_secs(cfg.api_timeout_secs.unwrap_or(30));
    AgentBuilder::new().timeout(timeout).build()
}

fn parse_embeddings_from_value(value: Value) -> Result<Vec<Vec<f32>>, SemanticError> {
    match value {
        Value::Object(mut map) => {
            if let Some(embeddings) = map.remove("embeddings") {
                return parse_embedding_collection(embeddings);
            }

            if let Some(Value::Array(items)) = map.remove("data") {
                let mut vectors = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        Value::Object(mut obj) => {
                            if let Some(embedding) = obj.remove("embedding") {
                                vectors.push(parse_embedding_vector(embedding)?);
                            } else {
                                return Err(SemanticError::Inference(
                                    "missing `embedding` field in data item".into(),
                                ));
                            }
                        }
                        _ => {
                            return Err(SemanticError::Inference(
                                "unexpected entry inside `data` array".into(),
                            ))
                        }
                    }
                }
                return Ok(vectors);
            }

            Err(SemanticError::Inference(
                "unsupported API response shape".into(),
            ))
        }
        other => parse_embedding_collection(other),
    }
}

fn parse_embedding_collection(value: Value) -> Result<Vec<Vec<f32>>, SemanticError> {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                Ok(Vec::new())
            } else if items.iter().all(|item| matches!(item, Value::Array(_))) {
                items.into_iter().map(parse_embedding_vector).collect()
            } else {
                parse_embedding_vector(Value::Array(items)).map(|vec| vec![vec])
            }
        }
        other => parse_embedding_vector(other).map(|vec| vec![vec]),
    }
}

fn parse_embedding_vector(value: Value) -> Result<Vec<f32>, SemanticError> {
    match value {
        Value::Array(values) => values
            .into_iter()
            .map(|entry| match entry {
                Value::Number(num) => num
                    .as_f64()
                    .map(|f| f as f32)
                    .ok_or_else(|| SemanticError::Inference("non-finite embedding value".into())),
                other => Err(SemanticError::Inference(format!(
                    "embedding entries must be numbers, got {other:?}"
                ))),
            })
            .collect(),
        other => Err(SemanticError::Inference(format!(
            "embedding vector must be an array, got {other:?}"
        ))),
    }
}
