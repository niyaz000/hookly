use std::sync::Arc;

use chrono::Utc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::common::{types::RequestContext, KeyProvider};
use crate::error::AppError;
use crate::features::environments::repository::EnvironmentRepository;

use super::crypto;
use super::models::{
    ApiKey, ApiKeyResponse, ApiKeySettings, CreateApiKeyRequest,
    InsertAuditParams, ListApiKeysQuery, ListApiKeysResponse, RevealApiKeyResponse,
    UpdateApiKeyRequest, UpdateApiKeySettingsRequest, UpsertApiKeySettingsRequest,
};
use super::repository::ApiKeyRepository;

pub struct ApiKeyService {
    repo: ApiKeyRepository,
    env_repo: EnvironmentRepository,
    key_provider: Arc<dyn KeyProvider>,
}

impl ApiKeyService {
    pub fn new(repo: ApiKeyRepository, env_repo: EnvironmentRepository, key_provider: Arc<dyn KeyProvider>) -> Self {
        Self { repo, env_repo, key_provider }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(
        tenant_id = %req.tenant_id,
        user_id = %req.user_id,
        environment_id = %req.environment_id,
        name = %req.name
    ))]
    pub async fn create(
        &self,
        req: CreateApiKeyRequest,
        ctx: RequestContext,
    ) -> Result<(ApiKey, String), AppError> {
        info!("creating api key");

        let env = self
            .env_repo
            .get_by_public_id(&req.environment_id)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("environment not found: {}", req.environment_id))
            })?;

        if env.tenant_id != req.tenant_id {
            return Err(AppError::BadRequest("environment does not belong to this tenant".into()));
        }

        if env.status != crate::features::environments::models::EnvironmentStatus::Active {
            return Err(AppError::BadRequest("environment is not active".into()));
        }

        let settings = self
            .repo
            .get_settings_by_tenant(req.tenant_id)
            .await?;

        let key_length = settings.as_ref().map(|s| s.key_length).unwrap_or(32);
        let allow_view_later = settings.as_ref().map(|s| s.allow_view_later).unwrap_or(false);
        let default_ttl = settings.as_ref().and_then(|s| s.default_ttl_seconds);
        let max_keys = settings.as_ref().and_then(|s| s.max_keys_per_user);

        if let Some(max) = max_keys {
            let count = self
                .repo
                .count_active_for_user(req.tenant_id, req.user_id)
                .await?;
            if count >= max as i64 {
                warn!(
                    user_id = %req.user_id,
                    tenant_id = %req.tenant_id,
                    current_count = count,
                    max_keys = max,
                    "user has reached max api key limit"
                );
                return Err(AppError::Conflict(format!(
                    "user has reached the maximum of {} api key(s) for this tenant",
                    max
                )));
            }
        }

        let expires_at = req.expires_at.or_else(|| {
            default_ttl.map(|ttl| Utc::now() + chrono::Duration::seconds(ttl as i64))
        });

        let (full_key, key_prefix) = crypto::generate_api_key(&env.name, key_length);
        let key_hash = crypto::hash_key(&full_key);

        let key_encrypted = if allow_view_later {
            let raw_key = self.key_provider.get_encryption_key().await?;
            Some(crypto::encrypt_key(raw_key, &full_key)?)
        } else {
            None
        };

        let key = self
            .repo
            .create(
                req.organization_id,
                req.tenant_id,
                req.user_id,
                req.name,
                req.description,
                key_hash,
                key_encrypted,
                key_prefix,
                req.environment_id,
                expires_at,
                ctx,
            )
            .await?;

        let audit = InsertAuditParams {
            api_key_id: key.id,
            api_key_public_id: key.public_id.clone(),
            organization_id: key.organization_id,
            tenant_id: key.tenant_id,
            user_id: key.user_id,
            action: "created",
            actor_id: Some(ctx.created_by),
            request_id: ctx.request_id,
            changes: None,
        };
        if let Err(e) = self.repo.insert_audit(audit).await {
            warn!(error = ?e, api_key_public_id = %key.public_id, "failed to write creation audit record");
        }

        info!(public_id = %key.public_id, "api key created");
        Ok((key, full_key))
    }

    #[tracing::instrument(skip(self), fields(public_id = %public_id))]
    pub async fn get_by_id(&self, public_id: &str) -> Result<ApiKey, AppError> {
        info!("fetching api key");

        self.repo
            .get_by_public_id(public_id)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "api key not found");
                AppError::NotFound(format!("api key not found: {}", public_id))
            })
    }

    #[tracing::instrument(skip(self, query), fields(
        tenant_id = ?query.tenant_id,
        limit = ?query.limit
    ))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        query: ListApiKeysQuery,
    ) -> Result<ListApiKeysResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        info!(tenant_id = %tenant_id, limit = limit, "listing api keys");

        let tags_val = query.tags.as_ref()
            .filter(|t| !t.is_empty())
            .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null));

        let (keys, next_cursor_id) = self
            .repo
            .list(
                tenant_id,
                query.user_id,
                query.environment_id,
                query.status,
                limit,
                query.cursor,
                tags_val,
            )
            .await?;

        let next_cursor = next_cursor_id.map(|id| id.to_string());
        let items: Vec<ApiKeyResponse> = keys.into_iter().map(ApiKeyResponse::from).collect();

        info!(count = items.len(), "api keys listed");
        Ok(ListApiKeysResponse { items, next_cursor, limit })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(public_id = %public_id))]
    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateApiKeyRequest,
        ctx: RequestContext,
    ) -> Result<ApiKey, AppError> {
        info!("updating api key");

        let existing = self
            .repo
            .get_by_public_id(public_id)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "api key not found for update");
                AppError::NotFound(format!("api key not found: {}", public_id))
            })?;

        let mut changes = serde_json::Map::new();
        if let Some(ref new_desc) = req.description {
            changes.insert(
                "description".into(),
                serde_json::json!({ "from": existing.description, "to": new_desc }),
            );
        }
        if let Some(new_expires) = req.expires_at {
            changes.insert(
                "expires_at".into(),
                serde_json::json!({ "from": existing.expires_at, "to": new_expires }),
            );
        }
        let changes_val: Option<serde_json::Value> = if changes.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(changes))
        };

        let updated = self
            .repo
            .update(public_id, req.description, req.expires_at, ctx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("api key not found: {}", public_id)))?;

        let audit = InsertAuditParams {
            api_key_id: updated.id,
            api_key_public_id: updated.public_id.clone(),
            organization_id: updated.organization_id,
            tenant_id: updated.tenant_id,
            user_id: updated.user_id,
            action: "updated",
            actor_id: Some(ctx.created_by),
            request_id: ctx.request_id,
            changes: changes_val,
        };
        if let Err(e) = self.repo.insert_audit(audit).await {
            warn!(error = ?e, public_id = %public_id, "failed to write update audit record");
        }

        info!(public_id = %updated.public_id, "api key updated");
        Ok(updated)
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting api key");

        let existing = self
            .repo
            .get_by_public_id(public_id)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "api key not found for deletion");
                AppError::NotFound(format!("api key not found: {}", public_id))
            })?;

        let deleted = self.repo.delete(public_id, ctx).await?;
        if !deleted {
            return Err(AppError::NotFound(format!("api key not found: {}", public_id)));
        }

        let audit = InsertAuditParams {
            api_key_id: existing.id,
            api_key_public_id: existing.public_id.clone(),
            organization_id: existing.organization_id,
            tenant_id: existing.tenant_id,
            user_id: existing.user_id,
            action: "deleted",
            actor_id: Some(ctx.created_by),
            request_id: ctx.request_id,
            changes: None,
        };
        if let Err(e) = self.repo.insert_audit(audit).await {
            warn!(error = ?e, public_id = %public_id, "failed to write deletion audit record");
        }

        info!(public_id = %public_id, "api key deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn reveal(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<RevealApiKeyResponse, AppError> {
        info!("revealing api key");

        let key = self
            .repo
            .get_by_public_id(public_id)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "api key not found for reveal");
                AppError::NotFound(format!("api key not found: {}", public_id))
            })?;

        let settings = self.repo.get_settings_by_tenant(key.tenant_id).await?;
        let allow_view_later = settings.map(|s| s.allow_view_later).unwrap_or(false);

        if !allow_view_later {
            warn!(
                public_id = %public_id,
                tenant_id = %key.tenant_id,
                "reveal attempted on tenant without view-later enabled"
            );
            return Err(AppError::BadRequest(
                "view-later is not enabled for this tenant".into(),
            ));
        }

        let envelope = key.key_encrypted.as_deref().ok_or_else(|| {
            warn!(public_id = %public_id, "reveal attempted on key created without view-later");
            AppError::BadRequest(
                "this key was created before view-later was enabled and cannot be revealed".into(),
            )
        })?;

        let raw_key = self.key_provider.get_encryption_key().await?;
        let plaintext = crypto::decrypt_key(raw_key, envelope)?;

        let audit = InsertAuditParams {
            api_key_id: key.id,
            api_key_public_id: key.public_id.clone(),
            organization_id: key.organization_id,
            tenant_id: key.tenant_id,
            user_id: key.user_id,
            action: "revealed",
            actor_id: Some(ctx.created_by),
            request_id: ctx.request_id,
            changes: None,
        };
        if let Err(e) = self.repo.insert_audit(audit).await {
            warn!(error = ?e, public_id = %public_id, "failed to write reveal audit record");
        }

        info!(public_id = %public_id, "api key revealed");
        Ok(RevealApiKeyResponse {
            id: key.public_id,
            key: plaintext,
        })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(
        organization_id = %req.organization_id,
        tenant_id = %req.tenant_id
    ))]
    pub async fn upsert_settings(
        &self,
        req: UpsertApiKeySettingsRequest,
        ctx: RequestContext,
    ) -> Result<ApiKeySettings, AppError> {
        info!("upserting api key settings");

        let settings = self.repo.upsert_settings(&req, ctx).await?;

        info!(
            public_id = %settings.public_id,
            version = settings.version,
            "api key settings upserted"
        );
        Ok(settings)
    }

    #[tracing::instrument(skip(self), fields(public_id = %public_id))]
    pub async fn get_settings_by_id(&self, public_id: &str) -> Result<ApiKeySettings, AppError> {
        info!("fetching api key settings");

        self.repo
            .get_settings_by_public_id(public_id)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "api key settings not found");
                AppError::NotFound(format!("api key settings not found: {}", public_id))
            })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(public_id = %public_id))]
    pub async fn update_settings(
        &self,
        public_id: &str,
        req: UpdateApiKeySettingsRequest,
        ctx: RequestContext,
    ) -> Result<ApiKeySettings, AppError> {
        info!("updating api key settings");

        let settings = self
            .repo
            .update_settings_by_public_id(public_id, &req, ctx)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "api key settings not found for update");
                AppError::NotFound(format!("api key settings not found: {}", public_id))
            })?;

        info!(public_id = %settings.public_id, version = settings.version, "api key settings updated");
        Ok(settings)
    }
}
