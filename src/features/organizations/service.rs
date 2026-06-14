use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use tracing::{info, warn};
use uuid::Uuid;

use crate::common::{types::RequestContext, validators};
use crate::error::AppError;

use super::{
    models::{
        CreateOrganizationRequest, ListOrganizationsQuery, ListOrganizationsResponse,
        OrganizationResponse, UpdateOrganizationRequest,
    },
    repository::OrganizationRepository,
};

pub struct OrganizationService {
    repo: OrganizationRepository,
}

impl OrganizationService {
    pub fn new(repo: OrganizationRepository) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(slug = %req.slug))]
    pub async fn create(
        &self,
        req: CreateOrganizationRequest,
        ctx: RequestContext,
    ) -> Result<OrganizationResponse, AppError> {
        req.validate()?;
        if let Some(t) = &req.tags {
            validators::validate_tags(t)?;
        }
        info!(
            org_name = %req.name,
            org_slug = %req.slug,
            "creating organization with name={} and slug={}",
            req.name, req.slug
        );
        let org = self.repo.create(req, ctx).await?;
        info!(public_id = %org.public_id, "organization created with public_id={}", org.public_id);
        Ok(OrganizationResponse::from(org))
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_public_id(
        &self,
        public_id: String,
    ) -> Result<OrganizationResponse, AppError> {
        self.repo
            .get_by_public_id(&public_id)
            .await?
            .ok_or_else(|| {
                info!(public_id = %public_id, "Could not find organization with public_id = {}", public_id);
                AppError::NotFound(format!("Organization not found: {public_id}"))
            })
            .map(OrganizationResponse::from)
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn update(
        &self,
        public_id: String,
        req: UpdateOrganizationRequest,
        ctx: RequestContext,
    ) -> Result<OrganizationResponse, AppError> {
        req.validate()?;
        if let Some(t) = &req.tags {
            validators::validate_tags(t)?;
        }
        info!("updating organization");
        let org = self
            .repo
            .update(&public_id, req, ctx)
            .await?
            .ok_or_else(|| {
                warn!("organization not found for update");
                AppError::NotFound(format!("Organization not found: {public_id}"))
            })?;
        info!("organization updated");
        Ok(OrganizationResponse::from(org))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete(&self, public_id: String, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting organization");
        let deleted = self.repo.delete(&public_id, ctx).await?;
        if !deleted {
            warn!("organization not found for delete");
            return Err(AppError::NotFound(format!(
                "Organization not found: {public_id}"
            )));
        }
        info!("organization deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn suspend(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<OrganizationResponse, AppError> {
        info!("suspending organization");
        let org = self.repo.suspend(&public_id, ctx).await?.ok_or_else(|| {
            warn!("organization not found or not active");
            AppError::NotFound(format!(
                "Organization not found or not in active state: {public_id}"
            ))
        })?;
        info!("organization suspended");
        Ok(OrganizationResponse::from(org))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn restore(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<OrganizationResponse, AppError> {
        info!("restoring organization");
        let org = self.repo.restore(&public_id, ctx).await?.ok_or_else(|| {
            warn!("organization not found for restore");
            AppError::NotFound(format!("Organization not found: {public_id}"))
        })?;
        info!("organization restored");
        Ok(OrganizationResponse::from(org))
    }

    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        query: ListOrganizationsQuery,
    ) -> Result<ListOrganizationsResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

        let (orgs, next_cursor_id) = self.repo.list(limit, cursor, query.status).await?;

        Ok(ListOrganizationsResponse {
            items: orgs.into_iter().map(OrganizationResponse::from).collect(),
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
