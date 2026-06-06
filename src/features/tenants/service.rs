use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{common::types::RequestContext, error::AppError};
use crate::features::permissions::repository::PermissionRepository;
use crate::features::roles::repository::RoleRepository;

use super::{
    models::{
        CreateTenantRequest, ListTenantsQuery, ListTenantsResponse, TenantResponse,
        UpdateTenantRequest,
    },
    repository::TenantRepository,
};

pub struct TenantService {
    repo: TenantRepository,
    role_repo: RoleRepository,
    perm_repo: PermissionRepository,
}

impl TenantService {
    pub fn new(repo: TenantRepository, role_repo: RoleRepository, perm_repo: PermissionRepository) -> Self {
        Self { repo, role_repo, perm_repo }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(name = %req.name))]
    pub async fn create(
        &self,
        req: CreateTenantRequest,
        ctx: RequestContext,
    ) -> Result<TenantResponse, AppError> {
        req.validate()?;
        info!("creating tenant");
        let tenant = self.repo.create(req, ctx).await?;
        info!(public_id = %tenant.public_id, "tenant created");

        // Seed default roles (owner, admin, developer, viewer) for the new tenant
        let system_perms = self.perm_repo.list_system().await?;
        if let Err(e) = self.role_repo.seed_default_roles(tenant.id, &system_perms, ctx).await {
            tracing::warn!(tenant_id = %tenant.id, error = ?e, "failed to seed default roles");
        }

        Ok(TenantResponse::from(tenant))
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_public_id(&self, public_id: String) -> Result<TenantResponse, AppError> {
        info!("fetching tenant");
        self.repo
            .get_by_public_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("tenant not found");
                AppError::NotFound(format!("Tenant not found: {public_id}"))
            })
            .map(TenantResponse::from)
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn update(
        &self,
        public_id: String,
        req: UpdateTenantRequest,
        ctx: RequestContext,
    ) -> Result<TenantResponse, AppError> {
        req.validate()?;
        info!("updating tenant");
        let tenant = self
            .repo
            .update(&public_id, req, ctx)
            .await?
            .ok_or_else(|| {
                warn!("tenant not found for update");
                AppError::NotFound(format!("Tenant not found: {public_id}"))
            })?;
        info!("tenant updated");
        Ok(TenantResponse::from(tenant))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete(&self, public_id: String, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting tenant");
        let deleted = self.repo.delete(&public_id, ctx).await?;
        if !deleted {
            warn!("tenant not found for delete");
            return Err(AppError::NotFound(format!("Tenant not found: {public_id}")));
        }
        info!("tenant deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn suspend(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<TenantResponse, AppError> {
        info!("suspending tenant");
        let tenant = self.repo.suspend(&public_id, ctx).await?.ok_or_else(|| {
            warn!("tenant not found or not active");
            AppError::NotFound(format!(
                "Tenant not found or not in active state: {public_id}"
            ))
        })?;
        info!("tenant suspended");
        Ok(TenantResponse::from(tenant))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn reactivate(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<TenantResponse, AppError> {
        info!("reactivating tenant");
        let tenant = self
            .repo
            .reactivate(&public_id, ctx)
            .await?
            .ok_or_else(|| {
                warn!("tenant not found or not suspended");
                AppError::NotFound(format!(
                    "Tenant not found or not in suspended state: {public_id}"
                ))
            })?;
        info!("tenant reactivated");
        Ok(TenantResponse::from(tenant))
    }

    #[tracing::instrument(skip(self))]
    pub async fn list(&self, query: ListTenantsQuery) -> Result<ListTenantsResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

        let (tenants, next_cursor_id) = self
            .repo
            .list(limit, cursor, query.status, query.organization_id)
            .await?;

        Ok(ListTenantsResponse {
            items: tenants.into_iter().map(TenantResponse::from).collect(),
            next_cursor: next_cursor_id.map(encode_cursor),
            limit,
        })
    }
}

fn encode_cursor(id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(id.as_bytes())
}

fn decode_cursor(cursor: &str) -> Result<Uuid, AppError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| AppError::BadRequest("invalid cursor".into()))?;
    Uuid::from_slice(&bytes).map_err(|_| AppError::BadRequest("invalid cursor".into()))
}
