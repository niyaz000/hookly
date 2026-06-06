use tracing::{info, warn};
use uuid::Uuid;

use crate::error::AppError;

use super::models::{
    CreatePermissionRequest, ListPermissionsQuery, ListPermissionsResponse, Permission,
    PermissionResponse, UpdatePermissionRequest,
};
use super::repository::PermissionRepository;

pub struct PermissionService {
    repo: PermissionRepository,
}

impl PermissionService {
    pub fn new(repo: PermissionRepository) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip(self, req), fields(tenant_id = %req.tenant_id, name = %req.name))]
    pub async fn create(&self, req: CreatePermissionRequest) -> Result<Permission, AppError> {
        info!("creating custom permission");

        let perm = self
            .repo
            .create(req.tenant_id, req.name, req.description, req.resource, req.action)
            .await?;

        info!(public_id = %perm.public_id, "permission created");
        Ok(perm)
    }

    #[tracing::instrument(skip(self), fields(public_id = %public_id))]
    pub async fn get_by_id(&self, public_id: &str) -> Result<Permission, AppError> {
        self.repo.get_by_public_id(public_id).await?.ok_or_else(|| {
            warn!(public_id = %public_id, "permission not found");
            AppError::NotFound(format!("permission not found: {public_id}"))
        })
    }

    #[tracing::instrument(skip(self, query))]
    pub async fn list(
        &self,
        tenant_id: Option<Uuid>,
        query: ListPermissionsQuery,
    ) -> Result<ListPermissionsResponse, AppError> {
        let limit = query.limit.unwrap_or(50).clamp(1, 200);
        info!(tenant_id = ?tenant_id, limit = limit, "listing permissions");

        let (perms, next_cursor) = self
            .repo
            .list(tenant_id, query.perm_type, query.resource, limit, query.cursor)
            .await?;

        let items: Vec<PermissionResponse> =
            perms.into_iter().map(PermissionResponse::from).collect();

        Ok(ListPermissionsResponse { items, next_cursor, limit })
    }

    #[tracing::instrument(skip(self, req), fields(public_id = %public_id))]
    pub async fn update(
        &self,
        public_id: &str,
        req: UpdatePermissionRequest,
    ) -> Result<Permission, AppError> {
        info!("updating permission");

        self.repo
            .update_description(public_id, req.description)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "permission not found or is a system permission");
                AppError::NotFound(format!(
                    "custom permission not found: {public_id}"
                ))
            })
    }

    #[tracing::instrument(skip(self), fields(public_id = %public_id))]
    pub async fn delete(&self, public_id: &str) -> Result<(), AppError> {
        info!("deleting permission");

        let deleted = self.repo.delete(public_id).await?;
        if !deleted {
            warn!(public_id = %public_id, "permission not found or is a system permission");
            return Err(AppError::NotFound(format!(
                "custom permission not found: {public_id}"
            )));
        }

        info!(public_id = %public_id, "permission deleted");
        Ok(())
    }
}
