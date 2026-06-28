use axum::{
    extract::Request,
    http::header::AUTHORIZATION,
    middleware::Next,
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::common::types::RequestContext;
use crate::error::{AppError, REQUEST_ID};
use crate::state::AppState;

/// Injected into request extensions by `authenticate_admin` for all admin routes.
#[derive(Clone)]
pub struct AdminPrincipal;

pub async fn authenticate_admin(state: AppState, mut req: Request, next: Next) -> Response {
    let candidate = match extract_bearer(&req) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    if !keys_match(&state.admin_api_key, &candidate) {
        return AppError::Unauthorized("invalid admin key".into()).into_response();
    }

    let request_id = REQUEST_ID
        .try_with(|id| *id)
        .unwrap_or_else(|_| Uuid::now_v7());

    req.extensions_mut().insert(AdminPrincipal);
    req.extensions_mut().insert(RequestContext {
        request_id,
        created_by: Uuid::nil(),
        organization_id: Uuid::nil(),
        tenant_id: Uuid::nil(),
    });

    next.run(req).await
}

fn extract_bearer(req: &Request) -> Result<String, AppError> {
    let auth = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

    let token = auth
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("Authorization header must be a Bearer token".into()))?;

    if token.is_empty() {
        return Err(AppError::Unauthorized("Bearer token must not be empty".into()));
    }

    Ok(token.to_owned())
}

/// Constant-time comparison via HMAC-SHA256 to prevent timing attacks.
fn keys_match(stored: &str, candidate: &str) -> bool {
    type HmacSha256 = Hmac<Sha256>;
    const COMPARE_KEY: &[u8] = b"hookly-admin-compare";

    let mut m = HmacSha256::new_from_slice(COMPARE_KEY).expect("hmac accepts any key length");
    m.update(stored.as_bytes());
    let stored_tag = m.finalize().into_bytes();

    let mut m2 = HmacSha256::new_from_slice(COMPARE_KEY).expect("hmac accepts any key length");
    m2.update(candidate.as_bytes());

    m2.verify_slice(&stored_tag).is_ok()
}
