//! Axum [`FromRequestParts`] extractor that turns an `Authorization:
//! Bearer …` header into an [`ApiKeyContext`].
//!
//! Wired by `router_with_state` (R3 owns that hookup): the application
//! state must carry an `Arc<dyn ApiKeyLookup>` reachable via [`FromRef`]
//! so this extractor can resolve the presented bearer through whichever
//! lookup impl the bin selected.

#![allow(dead_code)]

use std::sync::Arc;

use axum::extract::{FromRef, FromRequestParts};
use axum::http::{StatusCode, request::Parts};

use super::apikey::{ApiKeyContext, ApiKeyLookup};

impl<S> FromRequestParts<S> for ApiKeyContext
where
    S: Send + Sync,
    Arc<dyn ApiKeyLookup>: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .ok_or((StatusCode::UNAUTHORIZED, "missing Authorization header"))?
            .to_str()
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid Authorization header"))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or((StatusCode::UNAUTHORIZED, "expected Bearer scheme"))?
            .trim();
        if token.is_empty() {
            return Err((StatusCode::UNAUTHORIZED, "empty bearer token"));
        }
        let lookup = Arc::<dyn ApiKeyLookup>::from_ref(state);
        match lookup.lookup(token).await {
            Ok(Some(ctx)) => Ok(ctx),
            Ok(None) => Err((StatusCode::UNAUTHORIZED, "unknown api key")),
            Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "key lookup failed")),
        }
    }
}
