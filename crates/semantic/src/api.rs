use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

use crate::normalize::l2_normalize_in_place;
use crate::resilience::{
    execute_with_retry_async, is_retryable_error, CircuitBreakerManager, CircuitState,
    RateLimitManager, RateLimitStats, RetryConfig, RetryResult, TokenBucket,
};
use crate::{SemanticConfig, SemanticEmbedding, SemanticError};

// Global managers for resilience (lazy-initialized)
static CIRCUIT_BREAKER_MANAGER: Lazy<CircuitBreakerManager> =
    Lazy::new(CircuitBreakerManager::default);
static RATE_LIMIT_MANAGER: Lazy<RateLimitManager> = Lazy::new(RateLimitManager::default);

// Global HTTP client with connection pooling
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(32)
        .build()
        .expect("Failed to build HTTP client")
});

#[derive(Clone, Copy)]
enum ApiProviderKind {
    HuggingFace,
    OpenAI,
    Custom,
}

/// Get the provider name string for tracking.
fn provider_name(cfg: &SemanticConfig) -> String {
    cfg.api_provider
        .as_deref()
        .unwrap_or("custom")
        .to_ascii_lowercase()
}

/// Handles the API-based embedding generation with resilience.
pub(crate) async fn semanticize_via_api(
    doc_id: &str,
    text: &str,
    cfg: &SemanticConfig,
) -> Result<SemanticEmbedding, SemanticError> {
    let url = cfg
        .api_url
        .as_deref()
        .ok_or_else(|| SemanticError::InvalidConfig("api_url is required for api mode".into()))?;
    let provider = api_provider_kind(cfg);
    let provider_name_str = provider_name(cfg);

    // Check resilience features are enabled
    if cfg.enable_resilience {
        // 1. Check circuit breaker
        let cb = CIRCUIT_BREAKER_MANAGER.get_or_create(&provider_name_str);
        if !cb.allow_request() {
            return Err(SemanticError::Inference(format!(
                "Circuit breaker is OPEN for provider '{provider_name_str}'. Service temporarily unavailable."
            )));
        }

        // 2. Check rate limit
        let rate_limiter = get_rate_limiter(cfg, &provider_name_str);
        if !rate_limiter.acquire() {
            return Err(SemanticError::Inference(format!(
                "Rate limit exceeded for provider '{provider_name_str}'. Please try again later."
            )));
        }
    }

    let payload_text = vec![text.to_string()];
    let payload = build_api_payload(provider, &payload_text, cfg, false);

    // Execute with retry logic if resilience is enabled
    let response_result = if cfg.enable_resilience {
        let retry_cfg = cfg.retry_config.unwrap_or_default();
        execute_api_request_with_retry(url, cfg, payload, &retry_cfg, &provider_name_str).await
    } else {
        // Direct request without retry
        match send_api_request(url, cfg, payload).await {
            Ok(r) => Ok(RetryResult {
                result: Ok(r),
                attempts: 1,
                total_duration: Duration::from_millis(0),
                succeeded: true,
            }),
            Err(e) => Err(e),
        }
    };

    // Handle response and record metrics
    match response_result {
        Ok(retry_result) => {
            if cfg.enable_resilience {
                // Record success for circuit breaker
                let cb = CIRCUIT_BREAKER_MANAGER.get_or_create(&provider_name_str);
                cb.record_success();
            }

            let response = retry_result
                .into_result()
                .map_err(SemanticError::Inference)?;

            process_response(doc_id, cfg, response)
        }
        Err(e) => {
            if cfg.enable_resilience {
                // Record failure for circuit breaker if it's a non-retryable or final failure
                let cb = CIRCUIT_BREAKER_MANAGER.get_or_create(&provider_name_str);
                cb.record_failure();
            }
            Err(e)
        }
    }
}

/// Handles batch API-based embedding generation with resilience.
pub(crate) async fn semanticize_batch_via_api<D, T>(
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
    let provider_name_str = provider_name(cfg);

    // Check resilience features are enabled
    if cfg.enable_resilience {
        // 1. Check circuit breaker
        let cb = CIRCUIT_BREAKER_MANAGER.get_or_create(&provider_name_str);
        if !cb.allow_request() {
            return Err(SemanticError::Inference(format!(
                "Circuit breaker is OPEN for provider '{provider_name_str}'. Service temporarily unavailable."
            )));
        }

        // 2. Check rate limit (batch counts as one request for rate limiting)
        let rate_limiter = get_rate_limiter(cfg, &provider_name_str);
        if !rate_limiter.acquire() {
            return Err(SemanticError::Inference(format!(
                "Rate limit exceeded for provider '{provider_name_str}'. Please try again later."
            )));
        }
    }

    let doc_ids: Vec<String> = docs
        .iter()
        .map(|(doc_id, _)| doc_id.as_ref().to_owned())
        .collect();
    let texts: Vec<String> = docs
        .iter()
        .map(|(_, text)| text.as_ref().to_owned())
        .collect();

    let payload = build_api_payload(provider, &texts, cfg, true);

    // Execute with retry logic if resilience is enabled
    let response_result = if cfg.enable_resilience {
        let retry_cfg = cfg.retry_config.unwrap_or_default();
        execute_api_request_with_retry(url, cfg, payload, &retry_cfg, &provider_name_str).await
    } else {
        match send_api_request(url, cfg, payload).await {
            Ok(r) => Ok(RetryResult {
                result: Ok(r),
                attempts: 1,
                total_duration: Duration::from_millis(0),
                succeeded: true,
            }),
            Err(e) => Err(e),
        }
    };

    // Handle response and record metrics
    match response_result {
        Ok(retry_result) => {
            if cfg.enable_resilience {
                let cb = CIRCUIT_BREAKER_MANAGER.get_or_create(&provider_name_str);
                cb.record_success();
            }

            let response = retry_result
                .into_result()
                .map_err(SemanticError::Inference)?;

            let vectors = parse_embeddings_from_value(response)?;

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
        Err(e) => {
            if cfg.enable_resilience {
                let cb = CIRCUIT_BREAKER_MANAGER.get_or_create(&provider_name_str);
                cb.record_failure();
            }
            Err(e)
        }
    }
}

/// Get or create rate limiter for provider.
fn get_rate_limiter(cfg: &SemanticConfig, provider: &str) -> Arc<TokenBucket> {
    if let Some(ref config) = cfg.rate_limit_config {
        RATE_LIMIT_MANAGER.get_or_create_with_config(provider, *config)
    } else {
        RATE_LIMIT_MANAGER.get_or_create(provider)
    }
}

/// Execute API request with retry logic.
async fn execute_api_request_with_retry(
    url: &str,
    cfg: &SemanticConfig,
    payload: Value,
    retry_cfg: &RetryConfig,
    provider: &str,
) -> Result<RetryResult<Value>, SemanticError> {
    let url = url.to_string();
    let cfg = cfg.clone();

    let result = execute_with_retry_async(retry_cfg, |attempt| {
        let url = url.clone();
        let cfg = cfg.clone();
        let payload = payload.clone();

        async move {
            // Log retry attempt
            if attempt > 0 {
                eprintln!("[semantic] Retry attempt {attempt} for provider '{provider}'");
            }

            match send_api_request(&url, &cfg, payload).await {
                Ok(response) => Ok(response),
                Err(e) => {
                    let error_str = e.to_string();
                    // Only retry on retryable errors
                    if is_retryable_error(&error_str) {
                        Err(error_str)
                    } else {
                        // Non-retryable error - fail immediately
                        Err(format!("Non-retryable error: {error_str}"))
                    }
                }
            }
        }
    })
    .await;

    if result.succeeded {
        Ok(result)
    } else {
        Err(SemanticError::Inference(
            result
                .result
                .err()
                .unwrap_or_else(|| "Request failed after retries".to_string()),
        ))
    }
}

/// Process single response into SemanticEmbedding.
fn process_response(
    doc_id: &str,
    cfg: &SemanticConfig,
    response: Value,
) -> Result<SemanticEmbedding, SemanticError> {
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

async fn send_api_request(
    url: &str,
    cfg: &SemanticConfig,
    payload: Value,
) -> Result<Value, SemanticError> {
    let mut request = HTTP_CLIENT.post(url);
    request = request.header("Content-Type", "application/json");
    if let Some(header) = cfg.api_auth_header.as_deref() {
        request = request.header("Authorization", header);
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .map_err(|e| SemanticError::Download(format!("HTTP request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(SemanticError::Download(format!(
            "HTTP error {status}: {body}"
        )));
    }

    response
        .json::<Value>()
        .await
        .map_err(|e| SemanticError::Inference(format!("Invalid JSON response: {e}")))
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

/// Public API for getting circuit breaker stats.
#[allow(dead_code)]
pub fn get_circuit_breaker_stats() -> Vec<(String, CircuitState, u64)> {
    CIRCUIT_BREAKER_MANAGER.get_all_stats()
}

/// Public API for getting rate limit stats.
#[allow(dead_code)]
pub fn get_rate_limit_stats() -> Vec<(String, RateLimitStats)> {
    RATE_LIMIT_MANAGER.get_all_stats()
}

/// Reset all circuit breakers (useful for testing or admin operations).
#[allow(dead_code)]
pub fn reset_circuit_breakers() {
    CIRCUIT_BREAKER_MANAGER.reset_all();
}

/// Reset all rate limiters (useful for testing or admin operations).
#[allow(dead_code)]
pub fn reset_rate_limiters() {
    RATE_LIMIT_MANAGER.reset_all();
}

/// Check if a provider's circuit breaker is healthy.
#[allow(dead_code)]
pub fn is_provider_healthy(provider: &str) -> bool {
    CIRCUIT_BREAKER_MANAGER.is_healthy(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resilience::{CircuitBreaker, CircuitBreakerConfig, RateLimitConfig, RetryConfig};
    use std::time::Duration;

    fn test_config() -> SemanticConfig {
        SemanticConfig {
            mode: "api".into(),
            api_url: Some("https://api.example.com/embed".into()),
            api_provider: Some("test".into()),
            enable_resilience: true,
            retry_config: Some(
                RetryConfig::default()
                    .with_max_retries(2)
                    .with_base_delay(Duration::from_millis(10)),
            ),
            circuit_breaker_config: Some(
                CircuitBreakerConfig::default()
                    .with_failure_threshold(2)
                    .with_reset_timeout(Duration::from_millis(50)),
            ),
            rate_limit_config: Some(
                RateLimitConfig::default()
                    .with_requests_per_second(100.0)
                    .with_burst_size(10),
            ),
            ..SemanticConfig::default()
        }
    }

    #[test]
    fn test_provider_name_extraction() {
        let cfg = SemanticConfig {
            api_provider: Some("openai".into()),
            ..SemanticConfig::default()
        };
        assert_eq!(provider_name(&cfg), "openai");

        let cfg2 = SemanticConfig {
            api_provider: None,
            ..SemanticConfig::default()
        };
        assert_eq!(provider_name(&cfg2), "custom");
    }

    #[test]
    fn test_is_retryable_error_detection() {
        assert!(is_retryable_error("timeout"));
        assert!(is_retryable_error("connection reset"));
        assert!(is_retryable_error("HTTP 503"));
        assert!(is_retryable_error("HTTP 429"));
        assert!(!is_retryable_error("HTTP 400"));
        assert!(!is_retryable_error("HTTP 404"));
    }

    #[test]
    fn test_get_rate_limiter_uses_config() {
        let cfg = test_config();
        let limiter = get_rate_limiter(&cfg, "test");

        // Should be able to acquire burst_size tokens
        for _ in 0..10 {
            assert!(limiter.try_acquire());
        }

        // Should fail now
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_circuit_breaker_tracks_failures() {
        // Create a circuit breaker with custom config (low threshold for testing)
        let cb = Arc::new(CircuitBreaker::new(
            CircuitBreakerConfig::default()
                .with_failure_threshold(2)
                .with_reset_timeout(Duration::from_millis(50)),
        ));

        // First failure
        cb.record_failure();
        assert!(cb.allow_request());

        // Second failure - should open circuit
        cb.record_failure();
        assert!(!cb.allow_request());
        assert_eq!(cb.current_state(), CircuitState::Open);
    }

    #[test]
    fn test_is_provider_healthy() {
        reset_circuit_breakers();

        assert!(is_provider_healthy("new_provider"));

        // Simulate failure to open circuit
        let cb = CIRCUIT_BREAKER_MANAGER.get_or_create("failing_provider");
        for _ in 0..5 {
            cb.record_failure();
        }

        assert!(!is_provider_healthy("failing_provider"));

        reset_circuit_breakers();
    }

    #[test]
    fn test_parse_embedding_collection_various_formats() {
        // Direct array format
        let result = parse_embedding_collection(json!([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]));
        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0], vec![1.0, 2.0, 3.0]);

        // Single embedding format
        let result2 = parse_embedding_collection(json!([1.0, 2.0, 3.0]));
        assert!(result2.is_ok());
        let embeddings2 = result2.unwrap();
        assert_eq!(embeddings2.len(), 1);
        assert_eq!(embeddings2[0], vec![1.0, 2.0, 3.0]);

        // Empty array
        let result3 = parse_embedding_collection(json!([]));
        assert!(result3.is_ok());
        assert!(result3.unwrap().is_empty());
    }
}
