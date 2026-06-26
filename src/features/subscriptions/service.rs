use tracing::{info, warn};

use crate::common::types::{PaginatedResponse, RequestContext};
use crate::error::AppError;

use super::models::{CreateSubscriptionRequest, ListQueryParams, SubscriptionResponse};
use super::repository::SubscriptionRepository;

pub struct SubscriptionService {
    repo: SubscriptionRepository,
}

impl SubscriptionService {
    pub fn new(repo: SubscriptionRepository) -> Self {
        Self { repo }
    }

    pub async fn create(
        &self,
        req: CreateSubscriptionRequest,
        _ctx: RequestContext,
    ) -> Result<SubscriptionResponse, AppError> {
        let app = self
            .repo
            .get_application(&req.application_id)
            .await?
            .ok_or_else(|| {
                warn!("application not found");
                AppError::NotFound(format!("Application not found: {}", req.application_id))
            })?;

        let ep = self
            .repo
            .get_endpoint_for_app(&req.endpoint_id, app.id)
            .await?
            .ok_or_else(|| {
                warn!("endpoint not found or not active");
                AppError::NotFound(format!(
                    "Endpoint not found or not active: {}",
                    req.endpoint_id
                ))
            })?;

        let et = self
            .repo
            .get_event_type_for_tenant(&req.event_type_id, app.tenant_id)
            .await?
            .ok_or_else(|| {
                warn!("event type not found or archived");
                AppError::NotFound(format!(
                    "Event type not found or archived: {}",
                    req.event_type_id
                ))
            })?;

        let row = self.repo.create(app, ep.id, et.id).await?;
        info!(public_id = %row.public_id, "subscription created");
        Ok(SubscriptionResponse::from(row))
    }

    pub async fn get_by_id(&self, public_id: &str) -> Result<SubscriptionResponse, AppError> {
        self.repo
            .get_by_id(public_id)
            .await?
            .ok_or_else(|| {
                warn!("subscription not found");
                AppError::NotFound(format!("Subscription not found: {public_id}"))
            })
            .map(SubscriptionResponse::from)
    }

    pub async fn list(
        &self,
        filter: ListQueryParams,
    ) -> Result<PaginatedResponse<SubscriptionResponse>, AppError> {
        let page = filter.page;
        let limit = filter.limit;
        let (items, total) = self.repo.list(filter).await?;
        Ok(PaginatedResponse {
            items: items.into_iter().map(SubscriptionResponse::from).collect(),
            total,
            page: page as i32,
            limit: limit as i32,
        })
    }

    pub async fn delete(&self, public_id: &str) -> Result<(), AppError> {
        let deleted = self.repo.delete(public_id).await?;
        if !deleted {
            return Err(AppError::NotFound(format!(
                "Subscription not found: {public_id}"
            )));
        }
        info!(public_id = %public_id, "subscription deleted");
        Ok(())
    }
}
