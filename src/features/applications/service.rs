use crate::common::types::RequestContext;
use crate::error::AppError;
use crate::features::applications::models::{Application, CreateApplicationRequest};
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
}
