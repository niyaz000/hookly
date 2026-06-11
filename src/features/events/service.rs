use std::collections::HashMap;
use std::sync::OnceLock;

use opentelemetry::metrics::Counter;
use opentelemetry::{global, KeyValue};
use tracing::{info, warn};
use uuid::Uuid;

static EVENTS_COUNTER: OnceLock<Counter<u64>> = OnceLock::new();

fn events_counter() -> &'static Counter<u64> {
    EVENTS_COUNTER.get_or_init(|| {
        global::meter("hookly.api")
            .u64_counter("events_published_total")
            .with_description("Total events successfully published")
            .build()
    })
}

use crate::common::types::{PaginatedResponse, RequestContext};
use crate::error::AppError;
use crate::features::delivery::repository::DeliveryRepository;
use crate::features::events::models::{CreateEventRequest, EventResponse, ListQueryParams};
use crate::features::events::repository::EventRepository;
use crate::queue;

pub struct EventService {
    repo: EventRepository,
    delivery: DeliveryRepository,
    redis: redis::Client,
}

impl EventService {
    pub fn new(repo: EventRepository, delivery: DeliveryRepository, redis: redis::Client) -> Self {
        Self {
            repo,
            delivery,
            redis,
        }
    }

    fn validate_payload(payload: &serde_json::Value) -> Result<(), AppError> {
        if !payload.is_object() {
            return Err(AppError::BadRequest("payload must be a JSON object".into()));
        }
        let size = serde_json::to_string(payload).map(|s| s.len()).unwrap_or(0);
        if size > 512 * 1024 {
            return Err(AppError::BadRequest("payload exceeds 512 KB limit".into()));
        }
        Ok(())
    }

    fn validate_tags(tags: &HashMap<String, String>) -> Result<(), AppError> {
        if tags.len() > 20 {
            return Err(AppError::BadRequest("tags: max 20 entries".into()));
        }
        for (k, v) in tags {
            if k.len() > 128 {
                return Err(AppError::BadRequest("tags: key exceeds 128 chars".into()));
            }
            if v.len() > 256 {
                return Err(AppError::BadRequest("tags: value exceeds 256 chars".into()));
            }
        }
        Ok(())
    }

    /// Creates a delivery_job row and enqueues a reference into Redis Streams.
    ///
    /// Failures are logged as warnings and do not propagate — the outbox poller
    /// (Phase 3) will re-enqueue any jobs whose `enqueued_at` remains NULL.
    async fn enqueue_delivery(&self, event_id: Uuid, endpoint_id: Uuid, organization_id: Uuid) {
        let tier = self.delivery.get_org_tier(organization_id).await;
        let stream = queue::stream_for_tier(&tier, organization_id);
        match self
            .delivery
            .create_job(event_id, endpoint_id, organization_id, &stream)
            .await
        {
            Ok(job) => match queue::enqueue(&self.redis, &stream, &job.public_id).await {
                Ok(_) => {
                    if let Err(e) = self.delivery.mark_enqueued(job.id).await {
                        warn!(job_public_id = %job.public_id, "mark_enqueued failed: {e:?}");
                    }
                    info!(job_public_id = %job.public_id, "delivery job enqueued");
                }
                Err(e) => {
                    warn!(job_public_id = %job.public_id, "XADD failed, outbox poller will retry: {e}");
                }
            },
            Err(e) => {
                warn!(event_id = %event_id, "failed to create delivery_job: {e:?}");
            }
        }
    }

    /// Creates an event. Returns `(response, true)` for a fresh insert and
    /// `(response, false)` when the `idempotency_key` already exists (replay).
    #[tracing::instrument(skip(self, req, ctx), fields(application_id = %req.application_id))]
    pub async fn create(
        &self,
        req: CreateEventRequest,
        ctx: RequestContext,
    ) -> Result<(EventResponse, bool), AppError> {
        Self::validate_payload(&req.payload)?;
        Self::validate_tags(&req.tags)?;
        if let Some(k) = &req.idempotency_key {
            if k.is_empty() || k.len() > 256 {
                return Err(AppError::BadRequest(
                    "idempotency_key must be between 1 and 256 chars".into(),
                ));
            }
        }

        let app = self
            .repo
            .get_application(&req.application_id)
            .await?
            .ok_or_else(|| {
                warn!("application not found");
                AppError::NotFound(format!("Application not found: {}", req.application_id))
            })?;

        let et = self
            .repo
            .get_event_type(&req.event_type_id, app.tenant_id)
            .await?
            .ok_or_else(|| {
                warn!("event type not found or archived");
                AppError::NotFound(format!(
                    "Event type not found or archived: {}",
                    req.event_type_id
                ))
            })?;

        let ep = self
            .repo
            .get_endpoint_for_event(&req.endpoint_id, app.id)
            .await?
            .ok_or_else(|| {
                warn!("endpoint not found or not active");
                AppError::NotFound(format!(
                    "Endpoint not found or not active: {}",
                    req.endpoint_id
                ))
            })?;

        info!("creating event");
        let (row, created) = self
            .repo
            .create(
                app,
                et.id,
                Some(ep.id),
                &req.payload,
                req.idempotency_key.as_deref(),
                &req.tags,
                ctx,
            )
            .await?;

        if created {
            info!(public_id = %row.public_id, "event created");
            self.enqueue_delivery(row.id, ep.id, row.organization_id)
                .await;
            events_counter().add(1, &[
                KeyValue::new("tenant_id", row.tenant_id.to_string()),
                KeyValue::new("application_id", req.application_id.clone()),
            ]);
        } else {
            info!(public_id = %row.public_id, "idempotent replay, returning existing event");
        }

        Ok((EventResponse::from(row), created))
    }

    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        filter: ListQueryParams,
    ) -> Result<PaginatedResponse<EventResponse>, AppError> {
        let page = filter.page;
        let limit = filter.limit;
        let (items, total) = self.repo.list(filter).await?;
        Ok(PaginatedResponse {
            items: items.into_iter().map(EventResponse::from).collect(),
            total,
            page: page as i32,
            limit: limit as i32,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_id(&self, public_id: String) -> Result<EventResponse, AppError> {
        self.repo
            .get_by_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("event not found");
                AppError::NotFound(format!("Event not found: {public_id}"))
            })
            .map(EventResponse::from)
    }
}
