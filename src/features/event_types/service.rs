use tracing::{info, warn};

use crate::common::types::{PaginatedResponse, RequestContext};
use crate::error::AppError;
use crate::features::event_types::models::{
    CreateEventTypeRequest, CreateVersionRequest, EventTypeResponse, EventTypeSchemaResponse,
    FieldType, ListQueryParams, UpdateEventTypeRequest,
};
use crate::features::event_types::repository::EventTypeRepository;

pub struct EventTypeService {
    repo: EventTypeRepository,
}

impl EventTypeService {
    pub fn new(repo: EventTypeRepository) -> Self {
        Self { repo }
    }

    fn validate_schema(
        req_schema: &crate::features::event_types::models::PropertyDef,
    ) -> Result<(), AppError> {
        if req_schema.field_type != FieldType::Object {
            return Err(AppError::BadRequest(
                "event_schema root must be of type 'object'".into(),
            ));
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, req, ctx), fields(name = %req.name))]
    pub async fn create(
        &self,
        req: CreateEventTypeRequest,
        ctx: RequestContext,
    ) -> Result<EventTypeResponse, AppError> {
        Self::validate_schema(&req.event_schema)?;
        info!("creating event_type");
        let et = self.repo.create(req, ctx).await?;
        info!(public_id = %et.public_id, "event_type created");
        Ok(EventTypeResponse::from(et))
    }

    #[tracing::instrument(skip(self, req, ctx), fields(source = %source_public_id))]
    pub async fn create_version(
        &self,
        source_public_id: String,
        req: CreateVersionRequest,
        ctx: RequestContext,
    ) -> Result<EventTypeResponse, AppError> {
        Self::validate_schema(&req.event_schema)?;
        info!("creating event_type version");
        self.repo
            .create_version(&source_public_id, req, ctx)
            .await?
            .ok_or_else(|| {
                warn!("source event_type not found");
                AppError::NotFound(format!("EventType not found: {source_public_id}"))
            })
            .map(EventTypeResponse::from)
    }

    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        filter: ListQueryParams,
    ) -> Result<PaginatedResponse<EventTypeResponse>, AppError> {
        let page = filter.page;
        let limit = filter.limit;
        let (items, total) = self.repo.list(filter).await?;
        Ok(PaginatedResponse {
            items: items.into_iter().map(EventTypeResponse::from).collect(),
            total,
            page: page as i32,
            limit: limit as i32,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_id(&self, public_id: String) -> Result<EventTypeResponse, AppError> {
        info!("fetching event_type");
        self.repo
            .get_by_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("event_type not found");
                AppError::NotFound(format!("EventType not found: {public_id}"))
            })
            .map(EventTypeResponse::from)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_versions(
        &self,
        public_id: String,
    ) -> Result<Vec<EventTypeResponse>, AppError> {
        info!("fetching event_type versions");
        let versions = self.repo.get_versions(&public_id).await?;
        Ok(versions.into_iter().map(EventTypeResponse::from).collect())
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_schema(&self, public_id: String) -> Result<EventTypeSchemaResponse, AppError> {
        info!("fetching event_type schema");
        self.repo
            .get_by_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("event_type not found");
                AppError::NotFound(format!("EventType not found: {public_id}"))
            })
            .map(EventTypeSchemaResponse::from)
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn update_description(
        &self,
        public_id: String,
        req: UpdateEventTypeRequest,
        ctx: RequestContext,
    ) -> Result<EventTypeResponse, AppError> {
        info!("updating event_type description");
        match self
            .repo
            .update_description(&public_id, req.description, req.version, ctx)
            .await?
        {
            Some(et) => Ok(EventTypeResponse::from(et)),
            None => {
                // distinguish not-found from version conflict
                if self.repo.get_by_id(&public_id).await?.is_none() {
                    warn!("event_type not found");
                    Err(AppError::NotFound(format!(
                        "EventType not found: {public_id}"
                    )))
                } else {
                    warn!("event_type version conflict");
                    Err(AppError::Conflict(
                        "version mismatch — fetch the latest version and retry".into(),
                    ))
                }
            }
        }
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete_by_id(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<(), AppError> {
        info!("deleting event_type");
        self.repo.delete_by_id(&public_id, ctx).await?;
        info!("event_type deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn archive(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<EventTypeResponse, AppError> {
        info!("archiving event_type");
        self.repo
            .set_archived(&public_id, true, ctx)
            .await?
            .ok_or_else(|| {
                warn!("event_type not found for archive");
                AppError::NotFound(format!("EventType not found: {public_id}"))
            })
            .map(EventTypeResponse::from)
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn unarchive(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<EventTypeResponse, AppError> {
        info!("unarchiving event_type");
        self.repo
            .set_archived(&public_id, false, ctx)
            .await?
            .ok_or_else(|| {
                warn!("event_type not found for unarchive");
                AppError::NotFound(format!("EventType not found: {public_id}"))
            })
            .map(EventTypeResponse::from)
    }
}
