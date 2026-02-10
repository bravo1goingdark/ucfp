use once_cell::sync::OnceCell;
use onnxruntime::{environment::Environment, session::Session};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tokenizers::Tokenizer;

use crate::assets::ModelAssets;
use crate::SemanticError;

static ORT_ENV: OnceCell<Environment> = OnceCell::new();

thread_local! {
    static MODEL_CACHE: RefCell<std::collections::HashMap<ModelCacheKey, Rc<CachedModel>>> =
        RefCell::new(std::collections::HashMap::new());
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct ModelCacheKey {
    model_path: PathBuf,
    tokenizer_path: PathBuf,
}

pub(crate) struct CachedModel {
    pub(crate) tokenizer: Tokenizer,
    pub(crate) session: RefCell<Session<'static>>,
}

impl CachedModel {
    pub(crate) fn load(assets: &ModelAssets) -> Result<Self, SemanticError> {
        let tokenizer = Tokenizer::from_file(&assets.tokenizer_path)
            .map_err(|e| SemanticError::Inference(e.to_string()))?;

        let env = ort_environment()?;
        let session = env
            .new_session_builder()
            .map_err(|e| SemanticError::Inference(e.to_string()))?
            .with_model_from_file(assets.model_path.clone())
            .map_err(|e| SemanticError::Inference(e.to_string()))?;

        Ok(Self {
            tokenizer,
            session: RefCell::new(session),
        })
    }
}

pub(crate) fn get_or_load_model_handle(
    assets: &ModelAssets,
) -> Result<Rc<CachedModel>, SemanticError> {
    let key = ModelCacheKey {
        model_path: assets.model_path.clone(),
        tokenizer_path: assets.tokenizer_path.clone(),
    };

    MODEL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(handle) = cache.get(&key) {
            return Ok(handle.clone());
        }

        let handle = Rc::new(CachedModel::load(assets)?);
        cache.insert(key, handle.clone());
        Ok(handle)
    })
}

fn ort_environment() -> Result<&'static Environment, SemanticError> {
    ORT_ENV.get_or_try_init(|| {
        Environment::builder()
            .with_name("semantic")
            .build()
            .map_err(|e| SemanticError::Inference(e.to_string()))
    })
}
