use crate::common::types::RequestContext;
use crate::error::AppError;
use crate::features::applications::models::{Application, CreateApplicationRequest, GetApplicationResponse};
use crate::features::applications::repository::ApplicationRepository;

pub struct ApplicationService {
    repo: ApplicationRepository,
}

impl ApplicationService {
    pub fn new(repo: ApplicationRepository) -> Self {
        Self { repo }
    }

    pub async fn create(
        &self,
        req: CreateApplicationRequest,
        ctx: RequestContext,
    ) -> Result<Application, AppError> {
        self.repo.create(req, ctx).await
    }

    pub async fn get_by_id(&self, public_id: String) -> Result<GetApplicationResponse, AppError> {
        self.repo
            .get_by_id(public_id.clone())
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Application not found: {public_id}")))
    }

    pub async fn delete_by_id(&self, public_id: String) -> Result<(), AppError> {
        self.repo.delete_by_id(public_id).await
    }
}
