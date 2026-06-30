use axum::{
    extract::Request,
    http::header::AUTHORIZATION,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{call_counter, types::RequestContext};
use crate::error::{AppError, REQUEST_ID};
use crate::state::AppState;

use super::crypto::hash_key;
use super::extractor::ApiKeyPrincipal;

const AUTH_CACHE_TTL_SECS: u64 = 60;
const JWT_KEY_CACHE_TTL_SECS: u64 = 300;

/// Serialized into Redis. All fields needed to validate and build the principal without a DB hit.
#[derive(Serialize, Deserialize)]
struct AuthCacheEntry {
    api_key_public_id: String,
    organization_id: Uuid,
    tenant_id: Uuid,
    user_id: Uuid,
    expires_at: Option<DateTime<Utc>>,
    api_key_deleted: bool,
    api_key_status: String,
    org_status: String,
    org_deleted: bool,
    tenant_status: String,
    tenant_deleted: bool,
}

/// Flat result of the single JOIN query on cache miss.
#[derive(sqlx::FromRow)]
struct AuthRow {
    api_key_public_id: String,
    organization_id: Uuid,
    tenant_id: Uuid,
    user_id: Uuid,
    api_key_status: String,
    expires_at: Option<DateTime<Utc>>,
    api_key_deleted: bool,
    org_status: String,
    org_deleted: bool,
    tenant_status: String,
    tenant_deleted: bool,
}

pub async fn authenticate(state: AppState, mut req: Request, next: Next) -> Response {
    // Extract the bearer token synchronously before entering any async context.
    // This avoids holding a &Request reference across await points (Body is !Sync).
    let token = match extract_token(&req) {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

    let result = if is_jwt(&token) {
        resolve_jwt(state, token).await
    } else {
        resolve_api_key(state, hash_key(&token)).await
    };

    match result {
        Ok((principal, ctx)) => {
            req.extensions_mut().insert(principal);
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
        Err(e) => e.into_response(),
    }
}

/// Synchronous extraction — no async, no borrows carried across await points.
fn extract_token(req: &Request) -> Result<String, AppError> {
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

/// JWTs have exactly three dot-separated segments. API keys never contain dots.
fn is_jwt(token: &str) -> bool {
    token.splitn(4, '.').count() == 3
}

// ── API key path ──────────────────────────────────────────────────────────────

/// All async work operates on fully owned data so the resulting future is Send.
async fn resolve_api_key(
    state: AppState,
    key_hash: String,
) -> Result<(ApiKeyPrincipal, RequestContext), AppError> {
    let cache_key = format!("auth:{}", key_hash);

    // ── Redis cache read ──────────────────────────────────────────────────────
    call_counter::inc_redis();
    if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
        if let Ok(Some(raw)) = conn.get::<_, Option<String>>(&cache_key).await {
            if let Ok(entry) = serde_json::from_str::<AuthCacheEntry>(&raw) {
                let principal = validate(&entry)?;
                let ctx = build_ctx(&principal);
                return Ok((principal, ctx));
            }
        }
    }

    // ── Cache miss: single JOIN across api_keys, organizations, tenants ───────
    let row = sqlx::query_as::<_, AuthRow>(
        r#"
        SELECT
            ak.public_id                    AS api_key_public_id,
            ak.organization_id,
            ak.tenant_id,
            ak.user_id,
            ak.status::TEXT                 AS api_key_status,
            ak.expires_at,
            (ak.deleted_at IS NOT NULL)     AS api_key_deleted,
            o.status::TEXT                  AS org_status,
            (o.deleted_at IS NOT NULL)      AS org_deleted,
            t.status::TEXT                  AS tenant_status,
            (t.deleted_at IS NOT NULL)      AS tenant_deleted
        FROM api_keys ak
        JOIN organizations o ON o.id = ak.organization_id
        JOIN tenants t       ON t.id = ak.tenant_id
        WHERE ak.key_hash = $1
        "#,
    )
    .bind(&key_hash)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid or revoked api key".into()))?;

    let entry = AuthCacheEntry {
        api_key_public_id: row.api_key_public_id.clone(),
        organization_id: row.organization_id,
        tenant_id: row.tenant_id,
        user_id: row.user_id,
        expires_at: row.expires_at,
        api_key_deleted: row.api_key_deleted,
        api_key_status: row.api_key_status.clone(),
        org_status: row.org_status.clone(),
        org_deleted: row.org_deleted,
        tenant_status: row.tenant_status.clone(),
        tenant_deleted: row.tenant_deleted,
    };

    // Write to cache (best-effort — Redis failure is non-fatal)
    if let Ok(json) = serde_json::to_string(&entry) {
        call_counter::inc_redis();
        if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
            let _: Result<(), _> = conn.set_ex(&cache_key, json, AUTH_CACHE_TTL_SECS).await;
        }
    }

    let principal = validate(&entry)?;

    // Update last_used_at asynchronously — only on cache miss (~every 60 s per key).
    let pool = state.db.clone();
    let pub_id = principal.api_key_public_id.clone();
    tokio::spawn(async move {
        let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE public_id = $1")
            .bind(&pub_id)
            .execute(&pool)
            .await;
    });

    let ctx = build_ctx(&principal);
    Ok((principal, ctx))
}

/// Validates the cache entry against runtime conditions and returns the principal.
/// Order: key validity → org status → tenant status.
fn validate(entry: &AuthCacheEntry) -> Result<ApiKeyPrincipal, AppError> {
    if entry.api_key_deleted {
        return Err(AppError::Unauthorized("invalid or revoked api key".into()));
    }
    if entry.api_key_status == "expired" {
        return Err(AppError::Unauthorized("api key has expired".into()));
    }
    if let Some(expires_at) = entry.expires_at {
        if expires_at < Utc::now() {
            return Err(AppError::Unauthorized("api key has expired".into()));
        }
    }
    if entry.org_deleted {
        return Err(AppError::Forbidden("Organization has been deactivated".into()));
    }
    if entry.org_status == "suspended" {
        return Err(AppError::Forbidden("Organization has been suspended".into()));
    }
    if entry.tenant_deleted {
        return Err(AppError::Forbidden("Tenant has been deactivated".into()));
    }
    if entry.tenant_status == "suspended" {
        return Err(AppError::Forbidden("Tenant has been suspended".into()));
    }
    Ok(ApiKeyPrincipal {
        api_key_public_id: entry.api_key_public_id.clone(),
        organization_id: entry.organization_id,
        tenant_id: entry.tenant_id,
        user_id: entry.user_id,
    })
}

fn build_ctx(principal: &ApiKeyPrincipal) -> RequestContext {
    let request_id = REQUEST_ID
        .try_with(|id| *id)
        .unwrap_or_else(|_| Uuid::now_v7());
    RequestContext {
        request_id,
        created_by: principal.user_id,
        organization_id: principal.organization_id,
        tenant_id: principal.tenant_id,
    }
}

// ── JWT path ───────────────────────────────────────────────────────────────────

/// Cache entry for a JWT public key. Includes org/tenant status so we can gate
/// without an extra DB round-trip on every request.
#[derive(Serialize, Deserialize)]
struct JwtKeyCacheEntry {
    public_key_pem: String,
    algorithm: String,
    tenant_id: Uuid,
    organization_id: Uuid,
    org_status: String,
    org_deleted: bool,
    tenant_status: String,
    tenant_deleted: bool,
}

#[derive(sqlx::FromRow)]
struct JwtKeyRow {
    public_key: Option<String>,
    algorithm: String,
    tenant_id: Uuid,
    organization_id: Uuid,
    org_status: String,
    org_deleted: bool,
    tenant_status: String,
    tenant_deleted: bool,
}

/// Custom claims carried in every Hookly service-issued JWT.
#[derive(Deserialize)]
struct JwtClaims {
    sub: String,
    org_id: Uuid,
    tenant_id: Uuid,
}

async fn resolve_jwt(
    state: AppState,
    token: String,
) -> Result<(ApiKeyPrincipal, RequestContext), AppError> {
    let header = jsonwebtoken::decode_header(&token)
        .map_err(|_| AppError::Unauthorized("invalid JWT header".into()))?;

    let kid = header
        .kid
        .ok_or_else(|| AppError::Unauthorized("JWT missing kid header".into()))?;

    let entry = load_jwt_key(&state, &kid).await?;

    let algorithm = parse_jwt_algorithm(&entry.algorithm)?;
    let decoding_key = make_decoding_key(&entry.public_key_pem, algorithm)?;

    let mut validation = Validation::new(algorithm);
    validation.validate_aud = false;

    let token_data = jsonwebtoken::decode::<JwtClaims>(&token, &decoding_key, &validation)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                AppError::Unauthorized("JWT has expired".into())
            }
            _ => AppError::Unauthorized("invalid JWT signature or claims".into()),
        })?;

    let claims = token_data.claims;

    // Validate claims match the key's DB-side ownership — prevents cross-tenant token reuse.
    if claims.tenant_id != entry.tenant_id {
        return Err(AppError::Unauthorized(
            "JWT tenant_id does not match key ownership".into(),
        ));
    }
    if claims.org_id != entry.organization_id {
        return Err(AppError::Unauthorized(
            "JWT org_id does not match key ownership".into(),
        ));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("JWT sub is not a valid UUID".into()))?;

    if entry.org_deleted {
        return Err(AppError::Forbidden("Organization has been deactivated".into()));
    }
    if entry.org_status == "suspended" {
        return Err(AppError::Forbidden("Organization has been suspended".into()));
    }
    if entry.tenant_deleted {
        return Err(AppError::Forbidden("Tenant has been deactivated".into()));
    }
    if entry.tenant_status == "suspended" {
        return Err(AppError::Forbidden("Tenant has been suspended".into()));
    }

    let principal = ApiKeyPrincipal {
        api_key_public_id: kid,
        organization_id: entry.organization_id,
        tenant_id: entry.tenant_id,
        user_id,
    };
    let ctx = build_ctx(&principal);
    Ok((principal, ctx))
}

/// Load the JWT public key from Redis cache (5 min TTL) or DB on miss.
async fn load_jwt_key(state: &AppState, kid: &str) -> Result<JwtKeyCacheEntry, AppError> {
    let cache_key = format!("jwt_pk:{}", kid);

    call_counter::inc_redis();
    if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
        if let Ok(Some(raw)) = conn.get::<_, Option<String>>(&cache_key).await {
            if let Ok(entry) = serde_json::from_str::<JwtKeyCacheEntry>(&raw) {
                return Ok(entry);
            }
        }
    }

    let row = sqlx::query_as::<_, JwtKeyRow>(
        r#"
        SELECT
            jk.public_key,
            jk.algorithm::TEXT         AS algorithm,
            jk.tenant_id,
            t.organization_id,
            o.status::TEXT             AS org_status,
            (o.deleted_at IS NOT NULL) AS org_deleted,
            t.status::TEXT             AS tenant_status,
            (t.deleted_at IS NOT NULL) AS tenant_deleted
        FROM jwt_keys jk
        JOIN tenants t       ON t.id  = jk.tenant_id
        JOIN organizations o ON o.id  = t.organization_id
        WHERE jk.key_id    = $1
          AND jk.key_use   = 'authentication'
          AND jk.status    = 'active'
          AND jk.deleted_at IS NULL
          AND (jk.expires_at IS NULL OR jk.expires_at > NOW())
        "#,
    )
    .bind(kid)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("unknown or inactive JWT signing key".into()))?;

    let public_key_pem = row.public_key.ok_or_else(|| {
        AppError::Unauthorized("HMAC keys cannot be used for service authentication".into())
    })?;

    let entry = JwtKeyCacheEntry {
        public_key_pem,
        algorithm: row.algorithm,
        tenant_id: row.tenant_id,
        organization_id: row.organization_id,
        org_status: row.org_status,
        org_deleted: row.org_deleted,
        tenant_status: row.tenant_status,
        tenant_deleted: row.tenant_deleted,
    };

    if let Ok(json) = serde_json::to_string(&entry) {
        call_counter::inc_redis();
        if let Ok(mut conn) = state.redis.get_multiplexed_async_connection().await {
            let _: Result<(), _> = conn
                .set_ex(&cache_key, json, JWT_KEY_CACHE_TTL_SECS)
                .await;
        }
    }

    Ok(entry)
}

fn parse_jwt_algorithm(alg: &str) -> Result<Algorithm, AppError> {
    match alg {
        "RS256" => Ok(Algorithm::RS256),
        "RS384" => Ok(Algorithm::RS384),
        "RS512" => Ok(Algorithm::RS512),
        "ES256" => Ok(Algorithm::ES256),
        "ES384" => Ok(Algorithm::ES384),
        _ => Err(AppError::Unauthorized(format!(
            "unsupported JWT algorithm: {alg}"
        ))),
    }
}

fn make_decoding_key(pem: &str, algorithm: Algorithm) -> Result<DecodingKey, AppError> {
    match algorithm {
        Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => {
            DecodingKey::from_rsa_pem(pem.as_bytes())
                .map_err(|_| AppError::Internal("invalid RSA public key in key record".into()))
        }
        Algorithm::ES256 | Algorithm::ES384 => {
            DecodingKey::from_ec_pem(pem.as_bytes())
                .map_err(|_| AppError::Internal("invalid EC public key in key record".into()))
        }
        _ => Err(AppError::Unauthorized("unsupported algorithm".into())),
    }
}
