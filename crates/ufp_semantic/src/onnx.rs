use onnxruntime::ndarray::{Array, Array2};
use onnxruntime::session::Session;
use std::cell::RefCell;
use tokenizers::Tokenizer;

use crate::cache::CachedModel;
use crate::SemanticError;

pub(crate) fn run_onnx_embeddings<T>(
    handle: &CachedModel,
    texts: &[T],
) -> Result<Vec<Vec<f32>>, SemanticError>
where
    T: AsRef<str>,
{
    if texts.is_empty() {
        return Ok(Vec::new());
    }

    let (encoded, max_len) = encode_documents(&handle.tokenizer, texts)?;
    let (input_ids, attn_mask) = build_padded_arrays(encoded, max_len)?;
    execute_session(&handle.session, input_ids, attn_mask)
}

struct EncodedDoc {
    ids: Vec<i64>,
    mask: Vec<i64>,
}

fn encode_documents<T>(
    tokenizer: &Tokenizer,
    texts: &[T],
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
