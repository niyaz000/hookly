use tracing::{info, warn};
use uuid::Uuid;

use crate::common::{types::RequestContext, validators};
use crate::error::AppError;

use super::models::{
    CreateEnvironmentRequest, Environment, EnvironmentStatus, ListEnvironmentsQuery,
    ListEnvironmentsResponse, EnvironmentResponse, UpdateEnvironmentRequest,
};
use super::repository::EnvironmentRepository;

pub struct EnvironmentService {
    repo: EnvironmentRepository,
}

impl EnvironmentService {
    pub fn new(repo: EnvironmentRepository) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(tenant_id = %req.tenant_id, name = %req.name))]
    pub async fn create(
        &self,
        req: CreateEnvironmentRequest,
        ctx: RequestContext,
    ) -> Result<Environment, AppError> {
        info!("creating environment");
        if let Some(t) = &req.tags { validators::validate_tags(t)?; }

        let tenant_id = self
            .repo
            .resolve_tenant(&req.tenant_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tenant not found: {}", req.tenant_id)))?;

        let tags = serde_json::to_value(req.tags.unwrap_or_default())
            .unwrap_or(serde_json::Value::Object(Default::default()));

        let env = self.repo.create(tenant_id, req.name, req.description, tags, ctx).await?;

        info!(public_id = %env.public_id, "environment created");
        Ok(env)
    }

    #[tracing::instrument(skip(self), fields(public_id = %public_id))]
    pub async fn get_by_id(&self, public_id: &str) -> Result<Environment, AppError> {
        self.repo
            .get_by_public_id(public_id)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "environment not found");
                AppError::NotFound(format!("environment not found: {}", public_id))
            })
    }

    #[tracing::instrument(skip(self, query), fields(tenant_id = ?query.tenant_id))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        query: ListEnvironmentsQuery,
    ) -> Result<ListEnvironmentsResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        info!(tenant_id = %tenant_id, limit = limit, "listing environments");

        let tags_val = query.tags.as_ref()
            .filter(|t| !t.is_empty())
            .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null));

        let (envs, next_cursor) = self
            .repo
            .list(tenant_id, query.status, limit, query.cursor, tags_val)
            .await?;

        let items: Vec<EnvironmentResponse> = envs.into_iter().map(EnvironmentResponse::from).collect();
        info!(count = items.len(), "environments listed");

        Ok(ListEnvironmentsResponse { items, next_cursor, limit })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(public_id = %public_id))]
    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateEnvironmentRequest,
        ctx: RequestContext,
    ) -> Result<Environment, AppError> {
        info!("updating environment");
        if let Some(t) = &req.tags { validators::validate_tags(t)?; }

        let tags = serde_json::to_value(req.tags.unwrap_or_default())
            .unwrap_or(serde_json::Value::Object(Default::default()));

        self.repo
            .update_tags(public_id, tags, ctx)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "environment not found for update");
                AppError::NotFound(format!("environment not found: {}", public_id))
            })
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn enable(&self, public_id: &str, ctx: RequestContext) -> Result<Environment, AppError> {
        info!("enabling environment");

        self.repo
            .set_status(public_id, EnvironmentStatus::Active, ctx)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "environment not found for enable");
                AppError::NotFound(format!("environment not found: {}", public_id))
            })
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn disable(&self, public_id: &str, ctx: RequestContext) -> Result<Environment, AppError> {
        info!("disabling environment");

        self.repo
            .set_status(public_id, EnvironmentStatus::Disabled, ctx)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "environment not found for disable");
                AppError::NotFound(format!("environment not found: {}", public_id))
            })
    }
}
