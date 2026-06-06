use tracing::{info, warn};
use uuid::Uuid;

use crate::common::types::RequestContext;
use crate::error::AppError;
use crate::features::permissions::repository::PermissionRepository;

use super::models::{
    AssignPermissionsRequest, AssignPermissionsResponse, CreateRoleRequest, ListRolePermissionsResponse,
    ListRolesQuery, ListRolesResponse, Role, RoleResponse, UpdateRoleRequest,
};
use super::repository::RoleRepository;

pub struct RoleService {
    repo: RoleRepository,
    perm_repo: PermissionRepository,
}

impl RoleService {
    pub fn new(repo: RoleRepository, perm_repo: PermissionRepository) -> Self {
        Self { repo, perm_repo }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(tenant_id = %req.tenant_id, name = %req.name))]
    pub async fn create(
        &self,
        req: CreateRoleRequest,
        ctx: RequestContext,
    ) -> Result<Role, AppError> {
        info!("creating role");

        let role = self
            .repo
            .create(req.tenant_id, req.name, req.description, false, ctx)
            .await?;

        info!(public_id = %role.public_id, "role created");
        Ok(role)
    }

    #[tracing::instrument(skip(self), fields(public_id = %public_id))]
    pub async fn get_by_id(&self, public_id: &str) -> Result<Role, AppError> {
        self.repo.get_by_public_id(public_id).await?.ok_or_else(|| {
            warn!(public_id = %public_id, "role not found");
            AppError::NotFound(format!("role not found: {public_id}"))
        })
    }

    #[tracing::instrument(skip(self, query))]
    pub async fn list(
        &self,
        tenant_id: Option<Uuid>,
        query: ListRolesQuery,
    ) -> Result<ListRolesResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        info!(tenant_id = ?tenant_id, limit = limit, "listing roles");

        let (roles, next_cursor) = self
            .repo
            .list(tenant_id, query.is_system, limit, query.cursor)
            .await?;

        let items: Vec<RoleResponse> = roles.into_iter().map(RoleResponse::from).collect();
        Ok(ListRolesResponse { items, next_cursor, limit })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(public_id = %public_id))]
    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateRoleRequest,
        ctx: RequestContext,
    ) -> Result<Role, AppError> {
        info!("updating role");

        self.repo
            .update(public_id, req.name, req.description, ctx)
            .await?
            .ok_or_else(|| {
                warn!(public_id = %public_id, "role not found or is a system role");
                AppError::NotFound(format!("custom role not found: {public_id}"))
            })
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting role");

        let deleted = self.repo.delete(public_id, ctx).await?;
        if !deleted {
            warn!(public_id = %public_id, "role not found or is a system role");
            return Err(AppError::NotFound(format!("custom role not found: {public_id}")));
        }

        info!(public_id = %public_id, "role deleted");
        Ok(())
    }

    // ── Permissions ───────────────────────────────────────────────────────────

    #[tracing::instrument(skip(self), fields(role_id = %role_public_id))]
    pub async fn list_permissions(
        &self,
        role_public_id: &str,
    ) -> Result<ListRolePermissionsResponse, AppError> {
        // Verify role exists
        self.get_by_id(role_public_id).await?;

        let items = self.repo.list_permissions(role_public_id).await?;
        Ok(ListRolePermissionsResponse { role_id: role_public_id.to_owned(), items })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(role_id = %role_public_id))]
    pub async fn assign_permissions(
        &self,
        role_public_id: &str,
        req: AssignPermissionsRequest,
        ctx: RequestContext,
    ) -> Result<AssignPermissionsResponse, AppError> {
        info!("assigning permissions to role");

        let role_id = self
            .repo
            .get_id_by_public_id(role_public_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("role not found: {role_public_id}")))?;

        // Resolve permission public_ids to internal UUIDs
        let mut permission_ids: Vec<(Uuid, String)> = Vec::new();
        for perm_public_id in &req.permission_ids {
            let perm = self
                .perm_repo
                .get_by_public_id(perm_public_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("permission not found: {perm_public_id}"))
                })?;
            permission_ids.push((perm.id, perm.public_id));
        }

        let (assigned, already_present) =
            self.repo.assign_permissions(role_id, permission_ids, ctx).await?;

        Ok(AssignPermissionsResponse { assigned, already_present })
    }

    #[tracing::instrument(skip(self), fields(role_id = %role_public_id, perm_id = %perm_public_id))]
    pub async fn remove_permission(
        &self,
        role_public_id: &str,
        perm_public_id: &str,
    ) -> Result<(), AppError> {
        info!("removing permission from role");

        let removed = self.repo.remove_permission(role_public_id, perm_public_id).await?;
        if !removed {
            return Err(AppError::NotFound(format!(
                "permission {perm_public_id} not found on role {role_public_id}"
            )));
        }

        Ok(())
    }
}
