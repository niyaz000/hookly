use tracing::info;
use uuid::Uuid;

use crate::common::types::RequestContext;
use crate::error::AppError;
use crate::features::permissions::repository::PermissionRepository;
use crate::features::roles::repository::RoleRepository;

use super::models::{
    AssignPermissionsRequest, AssignRolesRequest, BulkAssignResponse,
    EffectivePermission, EffectivePermissionsResponse, ListAssignedPermissionsResponse,
    ListAssignedRolesResponse,
};
use super::repository::AssignmentRepository;

pub struct AssignmentService {
    repo: AssignmentRepository,
    role_repo: RoleRepository,
    perm_repo: PermissionRepository,
}

impl AssignmentService {
    pub fn new(
        repo: AssignmentRepository,
        role_repo: RoleRepository,
        perm_repo: PermissionRepository,
    ) -> Self {
        Self { repo, role_repo, perm_repo }
    }

    // ── User roles ─────────────────────────────────────────────────────────────

    pub async fn list_user_roles(
        &self,
        user_public_id: &str,
    ) -> Result<ListAssignedRolesResponse, AppError> {
        let rows = self.repo.list_user_roles(user_public_id).await?;
        let items = rows.into_iter().map(Into::into).collect();
        Ok(ListAssignedRolesResponse { items })
    }

    pub async fn assign_user_roles(
        &self,
        user_public_id: &str,
        tenant_id: Uuid,
        req: AssignRolesRequest,
        ctx: RequestContext,
    ) -> Result<BulkAssignResponse, AppError> {
        info!(user_public_id = %user_public_id, "assigning roles to user");

        let mut role_ids = Vec::new();
        for role_public_id in &req.role_ids {
            let (id, public_id) = self
                .role_repo
                .get_id_by_public_id(role_public_id)
                .await?
                .map(|id| (id, role_public_id.clone()))
                .ok_or_else(|| {
                    AppError::NotFound(format!("role not found: {role_public_id}"))
                })?;
            role_ids.push((id, public_id));
        }

        let (assigned, already_present) = self
            .repo
            .assign_user_roles(user_public_id, role_ids, tenant_id, req.expires_at, ctx.created_by)
            .await?;

        Ok(BulkAssignResponse { assigned, already_present })
    }

    pub async fn remove_user_role(
        &self,
        user_public_id: &str,
        role_public_id: &str,
    ) -> Result<(), AppError> {
        let removed = self.repo.remove_user_role(user_public_id, role_public_id).await?;
        if !removed {
            return Err(AppError::NotFound(format!(
                "role {role_public_id} not assigned to user {user_public_id}"
            )));
        }
        Ok(())
    }

    // ── User permissions ───────────────────────────────────────────────────────

    pub async fn list_user_permissions(
        &self,
        user_public_id: &str,
    ) -> Result<ListAssignedPermissionsResponse, AppError> {
        let rows = self.repo.list_user_permissions(user_public_id).await?;
        let items = rows.into_iter().map(Into::into).collect();
        Ok(ListAssignedPermissionsResponse { items })
    }

    pub async fn assign_user_permissions(
        &self,
        user_public_id: &str,
        tenant_id: Uuid,
        req: AssignPermissionsRequest,
        ctx: RequestContext,
    ) -> Result<BulkAssignResponse, AppError> {
        info!(user_public_id = %user_public_id, "assigning permissions to user");

        let mut perm_ids = Vec::new();
        for perm_public_id in &req.permission_ids {
            let perm = self
                .perm_repo
                .get_by_public_id(perm_public_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("permission not found: {perm_public_id}"))
                })?;
            perm_ids.push((perm.id, perm.public_id));
        }

        let (assigned, already_present) = self
            .repo
            .assign_user_permissions(
                user_public_id,
                perm_ids,
                tenant_id,
                req.expires_at,
                ctx.created_by,
            )
            .await?;

        Ok(BulkAssignResponse { assigned, already_present })
    }

    pub async fn remove_user_permission(
        &self,
        user_public_id: &str,
        perm_public_id: &str,
    ) -> Result<(), AppError> {
        let removed =
            self.repo.remove_user_permission(user_public_id, perm_public_id).await?;
        if !removed {
            return Err(AppError::NotFound(format!(
                "permission {perm_public_id} not assigned to user {user_public_id}"
            )));
        }
        Ok(())
    }

    pub async fn get_user_effective_permissions(
        &self,
        user_public_id: &str,
        tenant_id: Uuid,
    ) -> Result<EffectivePermissionsResponse, AppError> {
        let rows = self
            .repo
            .get_user_effective_permissions(user_public_id, tenant_id)
            .await?;

        let permissions: Vec<EffectivePermission> = rows.into_iter().map(Into::into).collect();

        Ok(EffectivePermissionsResponse {
            subject_id: user_public_id.to_owned(),
            tenant_id,
            permissions,
        })
    }

    // ── API key roles ──────────────────────────────────────────────────────────

    pub async fn list_api_key_roles(
        &self,
        api_key_public_id: &str,
    ) -> Result<ListAssignedRolesResponse, AppError> {
        let rows = self.repo.list_api_key_roles(api_key_public_id).await?;
        let items = rows.into_iter().map(Into::into).collect();
        Ok(ListAssignedRolesResponse { items })
    }

    pub async fn assign_api_key_roles(
        &self,
        api_key_public_id: &str,
        req: AssignRolesRequest,
        ctx: RequestContext,
    ) -> Result<BulkAssignResponse, AppError> {
        info!(api_key_public_id = %api_key_public_id, "assigning roles to api key");

        let mut role_ids = Vec::new();
        for role_public_id in &req.role_ids {
            let (id, public_id) = self
                .role_repo
                .get_id_by_public_id(role_public_id)
                .await?
                .map(|id| (id, role_public_id.clone()))
                .ok_or_else(|| {
                    AppError::NotFound(format!("role not found: {role_public_id}"))
                })?;
            role_ids.push((id, public_id));
        }

        let (assigned, already_present) = self
            .repo
            .assign_api_key_roles(api_key_public_id, role_ids, ctx.created_by)
            .await?;

        Ok(BulkAssignResponse { assigned, already_present })
    }

    pub async fn remove_api_key_role(
        &self,
        api_key_public_id: &str,
        role_public_id: &str,
    ) -> Result<(), AppError> {
        let removed =
            self.repo.remove_api_key_role(api_key_public_id, role_public_id).await?;
        if !removed {
            return Err(AppError::NotFound(format!(
                "role {role_public_id} not assigned to api key {api_key_public_id}"
            )));
        }
        Ok(())
    }

    // ── API key permissions ────────────────────────────────────────────────────

    pub async fn list_api_key_permissions(
        &self,
        api_key_public_id: &str,
    ) -> Result<ListAssignedPermissionsResponse, AppError> {
        let rows = self.repo.list_api_key_permissions(api_key_public_id).await?;
        let items = rows.into_iter().map(Into::into).collect();
        Ok(ListAssignedPermissionsResponse { items })
    }

    pub async fn assign_api_key_permissions(
        &self,
        api_key_public_id: &str,
        req: AssignPermissionsRequest,
        ctx: RequestContext,
    ) -> Result<BulkAssignResponse, AppError> {
        info!(api_key_public_id = %api_key_public_id, "assigning permissions to api key");

        let mut perm_ids = Vec::new();
        for perm_public_id in &req.permission_ids {
            let perm = self
                .perm_repo
                .get_by_public_id(perm_public_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("permission not found: {perm_public_id}"))
                })?;
            perm_ids.push((perm.id, perm.public_id));
        }

        let (assigned, already_present) = self
            .repo
            .assign_api_key_permissions(
                api_key_public_id,
                perm_ids,
                req.expires_at,
                ctx.created_by,
            )
            .await?;

        Ok(BulkAssignResponse { assigned, already_present })
    }

    pub async fn remove_api_key_permission(
        &self,
        api_key_public_id: &str,
        perm_public_id: &str,
    ) -> Result<(), AppError> {
        let removed =
            self.repo.remove_api_key_permission(api_key_public_id, perm_public_id).await?;
        if !removed {
            return Err(AppError::NotFound(format!(
                "permission {perm_public_id} not assigned to api key {api_key_public_id}"
            )));
        }
        Ok(())
    }
}
