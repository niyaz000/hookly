use tracing::{info, warn};

use crate::common::{types::RequestContext, validators};
use crate::error::AppError;
use crate::features::applications::models::{
    Application, CreateApplicationRequest, GetApplicationResponse,
};
use crate::features::applications::repository::ApplicationRepository;

pub struct ApplicationService {
    repo: ApplicationRepository,
}

impl ApplicationService {
    pub fn new(repo: ApplicationRepository) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(name = %req.name))]
    pub async fn create(
        &self,
        req: CreateApplicationRequest,
        ctx: RequestContext,
    ) -> Result<Application, AppError> {
        info!("creating application");
        validators::validate_tags(&req.tags)?;

        let environment_id = self
            .repo
            .resolve_environment(&req.environment_id)
            .await?
            .ok_or_else(|| {
                warn!(environment_id = %req.environment_id, "environment not found");
                AppError::NotFound(format!("Environment not found: {}", req.environment_id))
            })?;

        let application = self.repo.create(req, ctx.tenant_id, ctx.organization_id, environment_id, ctx).await?;
        info!(public_id = %application.public_id, "application created");
        Ok(application)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_id(&self, public_id: String) -> Result<GetApplicationResponse, AppError> {
        info!("fetching application");
        self.repo
            .get_by_id(public_id.clone())
            .await?
            .ok_or_else(|| {
                warn!("application not found");
                AppError::NotFound(format!("Application not found: {public_id}"))
            })
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete_by_id(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<(), AppError> {
        info!("deleting application");
        self.repo.delete_by_id(public_id, ctx).await?;
        info!("application deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn restore_by_id(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<GetApplicationResponse, AppError> {
        info!("restoring application");
        self.repo
            .restore_by_id(public_id.clone(), ctx)
            .await?
            .ok_or_else(|| {
                warn!("application not found for restore");
                AppError::NotFound(format!("Application not found: {public_id}"))
            })
    }
}
