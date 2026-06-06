use tracing::{info, warn};

use crate::common::{types::RequestContext, TenantCrypto};
use crate::error::AppError;

use super::{
    models::{
        CreatePlatformWebhookRequest, ListPlatformWebhooksQuery, ListPlatformWebhooksResponse,
        PlatformWebhookResponse, PlatformWebhookStatus, UpdatePlatformWebhookRequest,
    },
    repository::PlatformWebhookRepository,
};

pub struct PlatformWebhookService {
    repo: PlatformWebhookRepository,
    crypto: TenantCrypto,
}

impl PlatformWebhookService {
    pub fn new(repo: PlatformWebhookRepository, crypto: TenantCrypto) -> Self {
        Self { repo, crypto }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(tenant_id = %req.tenant_id, name = %req.name))]
    pub async fn create(
        &self,
        req: CreatePlatformWebhookRequest,
        ctx: RequestContext,
    ) -> Result<PlatformWebhookResponse, AppError> {
        info!("creating platform webhook");

        let count = self.repo.count_active_for_tenant(req.tenant_id).await?;
        if count >= PlatformWebhookRepository::max_per_tenant() {
            return Err(AppError::BadRequest(format!(
                "maximum of {} webhook endpoints per tenant reached",
                PlatformWebhookRepository::max_per_tenant()
            )));
        }

        let raw_secret = TenantCrypto::generate_webhook_secret();
        let signing_secret_enc = self.crypto.encrypt(req.tenant_id, &raw_secret)?;

        let webhook = self
            .repo
            .create(
                req.tenant_id,
                req.name,
                req.description,
                req.url,
                signing_secret_enc,
                req.metadata,
                ctx,
            )
            .await?;

        info!(public_id = %webhook.public_id, "platform webhook created");

        Ok(PlatformWebhookResponse::from_webhook(webhook).with_signing_secret(raw_secret))
    }

    #[tracing::instrument(skip(self), fields(public_id = %public_id))]
    pub async fn get_by_id(&self, public_id: &str) -> Result<PlatformWebhookResponse, AppError> {
        let webhook = self
            .repo
            .get_by_public_id(public_id)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "platform webhook not found");
                AppError::NotFound(format!("platform webhook not found: {public_id}"))
            })?;
        Ok(PlatformWebhookResponse::from_webhook(webhook))
    }

    #[tracing::instrument(skip(self, query))]
    pub async fn list(&self, query: ListPlatformWebhooksQuery) -> Result<ListPlatformWebhooksResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let (webhooks, next_cursor) = self
            .repo
            .list(query.tenant_id, query.status, limit, query.cursor)
            .await?;
        Ok(ListPlatformWebhooksResponse {
            items: webhooks.into_iter().map(PlatformWebhookResponse::from_webhook).collect(),
            next_cursor,
            limit,
        })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(public_id = %public_id))]
    pub async fn update(
        &self,
        public_id: &str,
        req: UpdatePlatformWebhookRequest,
        ctx: RequestContext,
    ) -> Result<PlatformWebhookResponse, AppError> {
        info!("updating platform webhook");
        let webhook = self
            .repo
            .update(public_id, req.name, req.description, req.url, req.metadata, ctx)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "platform webhook not found for update");
                AppError::NotFound(format!("platform webhook not found: {public_id}"))
            })?;
        info!("platform webhook updated");
        Ok(PlatformWebhookResponse::from_webhook(webhook))
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn suspend(&self, public_id: &str, ctx: RequestContext) -> Result<PlatformWebhookResponse, AppError> {
        info!("suspending platform webhook");
        self.repo
            .set_status(public_id, PlatformWebhookStatus::Suspended, ctx)
            .await?
            .map(PlatformWebhookResponse::from_webhook)
            .ok_or_else(|| AppError::NotFound(format!("platform webhook not found: {public_id}")))
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn activate(&self, public_id: &str, ctx: RequestContext) -> Result<PlatformWebhookResponse, AppError> {
        info!("activating platform webhook");
        self.repo
            .set_status(public_id, PlatformWebhookStatus::Active, ctx)
            .await?
            .map(PlatformWebhookResponse::from_webhook)
            .ok_or_else(|| AppError::NotFound(format!("platform webhook not found: {public_id}")))
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn rotate_secret(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<PlatformWebhookResponse, AppError> {
        info!("rotating platform webhook signing secret");

        // Load existing webhook to get tenant_id for encryption
        let existing = self
            .repo
            .get_by_public_id(public_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("platform webhook not found: {public_id}")))?;

        let raw_secret = TenantCrypto::generate_webhook_secret();
        let signing_secret_enc = self.crypto.encrypt(existing.tenant_id, &raw_secret)?;

        let webhook = self
            .repo
            .rotate_secret(public_id, signing_secret_enc, ctx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("platform webhook not found: {public_id}")))?;

        info!("platform webhook signing secret rotated");
        Ok(PlatformWebhookResponse::from_webhook(webhook).with_signing_secret(raw_secret))
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting platform webhook");
        let deleted = self.repo.soft_delete(public_id, ctx).await?;
        if !deleted {
            return Err(AppError::NotFound(format!("platform webhook not found: {public_id}")));
        }
        info!("platform webhook deleted");
        Ok(())
    }
}
