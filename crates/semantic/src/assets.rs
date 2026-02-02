use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{SemanticConfig, SemanticError};

#[derive(Debug)]
pub(crate) struct ModelAssets {
    pub(crate) model_path: PathBuf,
    pub(crate) tokenizer_path: PathBuf,
}

pub(crate) fn should_fallback_to_stub(err: &SemanticError) -> bool {
    matches!(
        err,
        SemanticError::ModelNotFound(_)
            | SemanticError::TokenizerMissing(_)
            | SemanticError::Download(_)
    )
}

/// Ensures that the model and tokenizer exist locally, downloading them when URLs are provided.
pub(crate) async fn resolve_model_assets(
    cfg: &SemanticConfig,
) -> Result<ModelAssets, SemanticError> {
    let model_path = ensure_local_file(&cfg.model_path, cfg.model_url.as_deref(), || {
        SemanticError::ModelNotFound(cfg.model_path.display().to_string())
    })
    .await?;

    let tokenizer_target = tokenizer_storage_path(cfg)?;
    let tokenizer_path = ensure_local_file(&tokenizer_target, cfg.tokenizer_url.as_deref(), || {
        SemanticError::TokenizerMissing(cfg.model_name.clone())
    })
    .await?;

    Ok(ModelAssets {
        model_path,
        tokenizer_path,
    })
}

/// Determines where the tokenizer should be stored. When no explicit path is supplied we infer a
/// filename from the remote URL and place it next to the model file.
fn tokenizer_storage_path(cfg: &SemanticConfig) -> Result<PathBuf, SemanticError> {
    if let Some(path) = &cfg.tokenizer_path {
        return Ok(path.clone());
    }

    if let Some(url) = &cfg.tokenizer_url {
        let inferred_name = infer_filename_from_url(url).unwrap_or_else(|| "tokenizer.json".into());
        let base_dir = cfg
            .model_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(base_dir.join(inferred_name));
    }

    Err(SemanticError::TokenizerMissing(cfg.model_name.clone()))
}

/// Returns `target` if it already exists, otherwise attempts to download `remote_url`.
async fn ensure_local_file<F>(
    target: &Path,
    remote_url: Option<&str>,
    on_missing: F,
) -> Result<PathBuf, SemanticError>
where
    F: FnOnce() -> SemanticError,
{
    if target.exists() {
        return Ok(target.to_path_buf());
    }

    if let Some(url) = remote_url {
        download_to_path(target, url).await?;
        return Ok(target.to_path_buf());
    }

    Err(on_missing())
}

/// Downloads `url` into `target`, creating parent directories as needed.
async fn download_to_path(target: &Path, url: &str) -> Result<(), SemanticError> {
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let response = reqwest::get(url)
        .await
        .map_err(|e| SemanticError::Download(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(SemanticError::Download(format!(
            "unexpected status {} while fetching {}",
            status, url
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| SemanticError::Download(e.to_string()))?;

    fs::write(target, &bytes)?;
    Ok(())
}

/// Extracts a filename from the provided URL, stripping query/fragment parts.
fn infer_filename_from_url(url: &str) -> Option<String> {
    url.split('/')
        .rev()
        .find(|segment| !segment.is_empty())
        .map(|segment| segment.split(['?', '#']).next().unwrap_or(segment))
        .map(|segment| segment.to_string())
}
