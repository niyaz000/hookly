use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use tracing::{info, warn};
use uuid::Uuid;

use crate::common::{types::RequestContext, validators};
use crate::error::AppError;

use super::{
    models::{
        CreateUserRequest, ListUsersQuery, ListUsersResponse, LockUserRequest, UpdateUserRequest,
        UserResponse,
    },
    repository::UserRepository,
};

pub struct UserService {
    repo: UserRepository,
}

impl UserService {
    pub fn new(repo: UserRepository) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(email = %req.email))]
    pub async fn create(
        &self,
        req: CreateUserRequest,
        ctx: RequestContext,
    ) -> Result<UserResponse, AppError> {
        req.validate()?;
        if let Some(t) = &req.tags { validators::validate_tags(t)?; }
        info!("creating user");

        let tenant_id = self
            .repo
            .resolve_tenant(&req.tenant_id)
            .await?
            .ok_or_else(|| {
                warn!(tenant_id = %req.tenant_id, "tenant not found");
                AppError::NotFound(format!("Tenant not found: {}", req.tenant_id))
            })?;

        let organization_id = self
            .repo
            .resolve_organization(&req.organization_id)
            .await?
            .ok_or_else(|| {
                warn!(organization_id = %req.organization_id, "organization not found");
                AppError::NotFound(format!("Organization not found: {}", req.organization_id))
            })?;

        let user = self.repo.create(req, tenant_id, organization_id, ctx).await?;
        info!(public_id = %user.public_id, "user created");
        Ok(UserResponse::from(user))
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_public_id(&self, public_id: String) -> Result<UserResponse, AppError> {
        info!("fetching user");
        self.repo
            .get_by_public_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("user not found");
                AppError::NotFound(format!("User not found: {public_id}"))
            })
            .map(UserResponse::from)
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn update(
        &self,
        public_id: String,
        req: UpdateUserRequest,
        ctx: RequestContext,
    ) -> Result<UserResponse, AppError> {
        req.validate()?;
        if let Some(t) = &req.tags { validators::validate_tags(t)?; }
        info!("updating user");
        let user = self
            .repo
            .update(&public_id, req, ctx)
            .await?
            .ok_or_else(|| {
                warn!("user not found for update");
                AppError::NotFound(format!("User not found: {public_id}"))
            })?;
        info!("user updated");
        Ok(UserResponse::from(user))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete(&self, public_id: String, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting user");
        let deleted = self.repo.delete(&public_id, ctx).await?;
        if !deleted {
            warn!("user not found for delete");
            return Err(AppError::NotFound(format!("User not found: {public_id}")));
        }
        info!("user deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn suspend(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<UserResponse, AppError> {
        info!("suspending user");
        let user = self.repo.suspend(&public_id, ctx).await?.ok_or_else(|| {
            warn!("user not found or not active");
            AppError::NotFound(format!(
                "User not found or not in active state: {public_id}"
            ))
        })?;
        info!("user suspended");
        Ok(UserResponse::from(user))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn reactivate(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<UserResponse, AppError> {
        info!("reactivating user");
        let user = self
            .repo
            .reactivate(&public_id, ctx)
            .await?
            .ok_or_else(|| {
                warn!("user not found or not suspended");
                AppError::NotFound(format!(
                    "User not found or not in suspended state: {public_id}"
                ))
            })?;
        info!("user reactivated");
        Ok(UserResponse::from(user))
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn lock(
        &self,
        public_id: String,
        req: LockUserRequest,
        ctx: RequestContext,
    ) -> Result<UserResponse, AppError> {
        info!("locking user");
        let user = self
            .repo
            .lock(&public_id, req.locked_until, ctx)
            .await?
            .ok_or_else(|| {
                warn!("user not found or not active");
                AppError::NotFound(format!(
                    "User not found or not in active state: {public_id}"
                ))
            })?;
        info!("user locked");
        Ok(UserResponse::from(user))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn unlock(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<UserResponse, AppError> {
        info!("unlocking user");
        let user = self.repo.unlock(&public_id, ctx).await?.ok_or_else(|| {
            warn!("user not found or not locked");
            AppError::NotFound(format!(
                "User not found or not in locked state: {public_id}"
            ))
        })?;
        info!("user unlocked");
        Ok(UserResponse::from(user))
    }

    #[tracing::instrument(skip(self))]
    pub async fn list(&self, query: ListUsersQuery) -> Result<ListUsersResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

        let organization_id = match query.organization_id {
            Some(ref pid) => Some(
                self.repo
                    .resolve_organization(pid)
                    .await?
                    .ok_or_else(|| AppError::NotFound(format!("Organization not found: {pid}")))?,
            ),
            None => None,
        };

        let tenant_id = match query.tenant_id {
            Some(ref pid) => Some(
                self.repo
                    .resolve_tenant(pid)
                    .await?
                    .ok_or_else(|| AppError::NotFound(format!("Tenant not found: {pid}")))?,
            ),
            None => None,
        };

        let (users, next_cursor_id) = self
            .repo
            .list(
                limit,
                cursor,
                query.status,
                organization_id,
                tenant_id,
            )
            .await?;

        Ok(ListUsersResponse {
            items: users.into_iter().map(UserResponse::from).collect(),
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
