use onnxruntime::ndarray::{Array, Array2};
use onnxruntime::session::Session;
use std::cell::RefCell;
use tokenizers::Tokenizer;

use crate::cache::CachedModel;
use crate::SemanticError;

/// Run ONNX embeddings with optional chunking support.
/// When chunking is enabled and text exceeds max_sequence_length,
/// the text is split into overlapping chunks, embedded separately,
/// and pooled using the specified strategy.
pub(crate) fn run_onnx_embeddings<T>(
    handle: &CachedModel,
    texts: &[T],
    max_sequence_length: usize,
    enable_chunking: bool,
    chunk_overlap_ratio: f32,
    pooling_strategy: &str,
) -> Result<Vec<Vec<f32>>, SemanticError>
where
    T: AsRef<str>,
{
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    // Check if any text needs chunking
    let needs_chunking = if enable_chunking {
        texts.iter().any(|text| {
            handle
                .tokenizer
                .encode(text.as_ref(), true)
                .map(|e| e.get_ids().len() > max_sequence_length)
                .unwrap_or(false)
        })
    } else {
        false
    };

    if needs_chunking && enable_chunking {
        // Process with chunking
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            let embedding = embed_with_chunking(
                handle,
                text.as_ref(),
                max_sequence_length,
                chunk_overlap_ratio,
                pooling_strategy,
            )?;
            results.push(embedding);
        }
        Ok(results)
    } else {
        // Standard processing (truncate if needed)
        let (encoded, max_len) = encode_documents(&handle.tokenizer, texts, max_sequence_length)?;
        let (input_ids, attn_mask) = build_padded_arrays(encoded, max_len)?;
        execute_session(&handle.session, input_ids, attn_mask)
    }
}

/// Embed a single text using sliding-window chunking.
fn embed_with_chunking(
    handle: &CachedModel,
    text: &str,
    max_sequence_length: usize,
    chunk_overlap_ratio: f32,
    pooling_strategy: &str,
) -> Result<Vec<f32>, SemanticError> {
    // Encode the full text to get token IDs
    let full_encoding = handle
        .tokenizer
        .encode(text, true)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;
    let token_ids: Vec<u32> = full_encoding.get_ids().to_vec();

    // If text fits in one chunk, process normally
    if token_ids.len() <= max_sequence_length {
        let (encoded, max_len) = encode_documents(&handle.tokenizer, &[text], max_sequence_length)?;
        let (input_ids, attn_mask) = build_padded_arrays(encoded, max_len)?;
        let mut vectors = execute_session(&handle.session, input_ids, attn_mask)?;
        return vectors
            .pop()
            .ok_or_else(|| SemanticError::Inference("model returned no outputs".into()));
    }

    // Create sliding window chunks
    let overlap_size = (max_sequence_length as f32 * chunk_overlap_ratio) as usize;
    let step_size = max_sequence_length - overlap_size;

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < token_ids.len() {
        let end = (start + max_sequence_length).min(token_ids.len());
        chunks.push(&token_ids[start..end]);

        if end >= token_ids.len() {
            break;
        }
        start += step_size;
    }

    // Decode each chunk back to text for processing
    let chunk_texts: Vec<String> = chunks
        .iter()
        .map(|chunk_ids| {
            handle
                .tokenizer
                .decode(chunk_ids, true)
                .unwrap_or_else(|_| String::new())
        })
        .filter(|s| !s.is_empty())
        .collect();

    if chunk_texts.is_empty() {
        return Err(SemanticError::Inference(
            "chunking produced no valid chunks".into(),
        ));
    }

    // Embed all chunks
    let (encoded, max_len) =
        encode_documents(&handle.tokenizer, &chunk_texts, max_sequence_length)?;
    let (input_ids, attn_mask) = build_padded_arrays(encoded, max_len)?;
    let chunk_embeddings = execute_session(&handle.session, input_ids, attn_mask)?;

    // Pool the embeddings
    pool_embeddings(&chunk_embeddings, pooling_strategy)
}

/// Pool multiple chunk embeddings into a single embedding.
fn pool_embeddings(embeddings: &[Vec<f32>], strategy: &str) -> Result<Vec<f32>, SemanticError> {
    if embeddings.is_empty() {
        return Err(SemanticError::Inference("no embeddings to pool".into()));
    }

    if embeddings.len() == 1 {
        return Ok(embeddings[0].clone());
    }

    let dim = embeddings[0].len();

    match strategy {
        "first" => Ok(embeddings[0].clone()),
        "mean" => {
            // Simple average
            let mut pooled = vec![0.0f32; dim];
            for embedding in embeddings {
                for (i, &val) in embedding.iter().enumerate() {
                    pooled[i] += val;
                }
            }
            let n = embeddings.len() as f32;
            for val in &mut pooled {
                *val /= n;
            }
            Ok(pooled)
        }
        "max" => {
            // Element-wise maximum
            let mut pooled = embeddings[0].clone();
            for embedding in embeddings.iter().skip(1) {
                for (i, &val) in embedding.iter().enumerate() {
                    if val > pooled[i] {
                        pooled[i] = val;
                    }
                }
            }
            Ok(pooled)
        }
        _ => {
            // Weighted mean with higher weight for center chunks
            let n = embeddings.len();
            let mut weights = Vec::with_capacity(n);

            // Compute weights based on position
            // Center chunks get higher weight, edge chunks get lower weight
            for i in 0..n {
                let normalized_pos = (i as f32 + 0.5) / n as f32; // 0.0 to 1.0
                let distance_from_center = (normalized_pos - 0.5).abs(); // 0.0 to 0.5
                let weight = 1.0 - (distance_from_center * 0.5); // 0.75 to 1.0
                weights.push(weight);
            }

            // Normalize weights
            let weight_sum: f32 = weights.iter().sum();
            for weight in &mut weights {
                *weight /= weight_sum;
            }

            // Compute weighted sum
            let mut pooled = vec![0.0f32; dim];
            for (embedding, weight) in embeddings.iter().zip(weights.iter()) {
                for (i, &val) in embedding.iter().enumerate() {
                    pooled[i] += val * weight;
                }
            }
            Ok(pooled)
        }
    }
}

struct EncodedDoc {
    ids: Vec<i64>,
    mask: Vec<i64>,
}

fn encode_documents<T>(
    tokenizer: &Tokenizer,
    texts: &[T],
    max_sequence_length: usize,
) -> Result<(Vec<EncodedDoc>, usize), SemanticError>
where
    T: AsRef<str>,
{
    let mut encoded = Vec::with_capacity(texts.len());
    let mut max_len = 0usize;

    for text in texts {
        let encoding = tokenizer
            .encode(text.as_ref(), true)
            .map_err(|e| SemanticError::Inference(e.to_string()))?;
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        max_len = max_len.max(ids.len());
        encoded.push(EncodedDoc { ids, mask });
    }

    // Cap max_len at max_sequence_length to prevent exceeding model limits
    max_len = max_len.min(max_sequence_length);

    // Truncate sequences that exceed max_sequence_length
    for doc in &mut encoded {
        if doc.ids.len() > max_sequence_length {
            doc.ids.truncate(max_sequence_length);
            doc.mask.truncate(max_sequence_length);
        }
    }

    Ok((encoded, max_len))
}

fn build_padded_arrays(
    encoded: Vec<EncodedDoc>,
    max_len: usize,
) -> Result<(Array2<i64>, Array2<i64>), SemanticError> {
    let seq_len = max_len.max(1);
    let batch = encoded.len();
    let mut id_storage = Vec::with_capacity(batch * seq_len);
    let mut mask_storage = Vec::with_capacity(batch * seq_len);

    for EncodedDoc { ids, mask } in encoded {
        if ids.len() != mask.len() {
            return Err(SemanticError::Inference(
                "tokenizer produced mismatched id/mask lengths".into(),
            ));
        }
        let len = ids.len();
        let pad = seq_len.saturating_sub(len);
        id_storage.extend(ids);
        mask_storage.extend(mask);
        if pad > 0 {
            id_storage.extend(std::iter::repeat_n(0, pad));
            mask_storage.extend(std::iter::repeat_n(0, pad));
        }
    }

    let input_ids = Array::from_shape_vec((batch, seq_len), id_storage)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;
    let attn_mask = Array::from_shape_vec((batch, seq_len), mask_storage)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;
    Ok((input_ids, attn_mask))
}

fn execute_session(
    session: &RefCell<Session<'static>>,
    input_ids: Array2<i64>,
    attn_mask: Array2<i64>,
) -> Result<Vec<Vec<f32>>, SemanticError> {
    let (batch, seq_len) = input_ids.dim();
    let mut guard = session.borrow_mut();
    let session_ref = &mut *guard;
    let mut runtime_inputs = Vec::with_capacity(session_ref.inputs.len());
    let mut input_ids_tensor = Some(input_ids);
    let mut attn_mask_tensor = Some(attn_mask);

    for input in &session_ref.inputs {
        match input.name.as_str() {
            "input_ids" => {
                let tensor = input_ids_tensor.take().ok_or_else(|| {
                    SemanticError::InvalidConfig(
                        "model requested `input_ids` multiple times".into(),
                    )
                })?;
                runtime_inputs.push(tensor.into_dyn());
            }
            "attention_mask" => {
                let tensor = attn_mask_tensor.take().ok_or_else(|| {
                    SemanticError::InvalidConfig(
                        "model requested `attention_mask` multiple times".into(),
                    )
                })?;
                runtime_inputs.push(tensor.into_dyn());
            }
            "token_type_ids" => {
                let tensor = Array::from_elem((batch, seq_len), 0_i64);
                runtime_inputs.push(tensor.into_dyn());
            }
            other => {
                return Err(SemanticError::Inference(format!(
                    "unsupported model input '{other}'"
                )))
            }
        }
    }

    if runtime_inputs.is_empty() {
        return Err(SemanticError::Inference(
            "model did not declare any inputs".into(),
        ));
    }

    let outputs = session_ref
        .run::<i64, f32, _>(runtime_inputs)
        .map_err(|e| SemanticError::Inference(e.to_string()))?;
    let output_tensor = outputs
        .into_iter()
        .next()
        .ok_or_else(|| SemanticError::Inference("model returned no outputs".into()))?;

    let flat: Vec<f32> = output_tensor.iter().copied().collect();
    if batch == 0 {
        return Ok(Vec::new());
    }
    if flat.is_empty() {
        return Ok(vec![Vec::new(); batch]);
    }
    if !flat.len().is_multiple_of(batch) {
        return Err(SemanticError::Inference(format!(
            "model output shape {}/{} is not divisible",
            flat.len(),
            batch
        )));
    }

    let chunk = flat.len() / batch;
    let mut vectors = Vec::with_capacity(batch);
    for slice in flat.chunks(chunk) {
        vectors.push(slice.to_vec());
    }
    Ok(vectors)
}
