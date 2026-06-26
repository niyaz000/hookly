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

use crate::common::{
    idempotency,
    types::{PaginatedResponse, RequestContext},
    validators,
};
use crate::error::AppError;
use crate::features::delivery::repository::DeliveryRepository;
use crate::features::event_types::models::PropertyDef;
use crate::features::events::models::{
    BulkCreateEventRequest, BulkCreateResponse, BulkEventError, BulkEventResultItem,
    CreateEventRequest, EventResponse, ListQueryParams, PayloadType, SchemaError,
};
use crate::features::events::repository::EventRepository;
use crate::features::subscriptions::repository::SubscriptionRepository;
use crate::queue;

pub struct EventService {
    repo: EventRepository,
    delivery: DeliveryRepository,
    subscriptions: SubscriptionRepository,
    redis: redis::Client,
}

impl EventService {
    pub fn new(
        repo: EventRepository,
        delivery: DeliveryRepository,
        subscriptions: SubscriptionRepository,
        redis: redis::Client,
    ) -> Self {
        Self {
            repo,
            delivery,
            subscriptions,
            redis,
        }
    }

    fn validate_payload(
        payload: &serde_json::Value,
        payload_type: &PayloadType,
    ) -> Result<(), AppError> {
        match payload_type {
            PayloadType::Json => {
                if !payload.is_object() {
                    return Err(AppError::BadRequest(
                        "payload must be a JSON object when payload_type is 'json'".into(),
                    ));
                }
            }
            PayloadType::Text => {
                if !payload.is_string() {
                    return Err(AppError::BadRequest(
                        "payload must be a string when payload_type is 'text'".into(),
                    ));
                }
            }
        }
        let size = serde_json::to_string(payload).map(|s| s.len()).unwrap_or(0);
        if size > 512 * 1024 {
            return Err(AppError::BadRequest("payload exceeds 512 KB limit".into()));
        }
        Ok(())
    }

    fn validate_against_schema(
        schema: &PropertyDef,
        payload: &serde_json::Value,
    ) -> (bool, Vec<SchemaError>) {
        let schema_json = schema.to_json_schema();
        match jsonschema::validator_for(&schema_json) {
            Ok(validator) => {
                let errors: Vec<SchemaError> = validator
                    .iter_errors(payload)
                    .map(|e| {
                        let raw = e.to_string();
                        let path = e.instance_path.to_string();
                        let (field, message) = if path.is_empty() {
                            let f = Self::extract_required_field(&raw).unwrap_or_default();
                            (f, "Missing required property".to_owned())
                        } else {
                            let f = path.trim_start_matches('/').replace('/', ".");
                            (f, raw)
                        };
                        SchemaError { field, message }
                    })
                    .collect();
                (errors.is_empty(), errors)
            }
            Err(e) => {
                warn!("event_schema compiled to invalid JSON Schema: {e}");
                (true, vec![])
            }
        }
    }

    fn extract_required_field(msg: &str) -> Option<String> {
        msg.strip_prefix('"')
            .and_then(|s| s.split_once('"'))
            .filter(|(_, rest)| rest.trim_start().starts_with("is a required property"))
            .map(|(field, _)| field.to_owned())
    }

    async fn enqueue_for_subscriptions(
        &self,
        event_id: Uuid,
        event_type_id: Uuid,
        application_id: Uuid,
        organization_id: Uuid,
    ) {
        match self
            .subscriptions
            .get_active_for_event_type(event_type_id, application_id)
            .await
        {
            Ok(endpoint_ids) => {
                for endpoint_id in endpoint_ids {
                    self.enqueue_delivery(event_id, endpoint_id, organization_id)
                        .await;
                }
            }
            Err(e) => {
                warn!(event_id = %event_id, "subscription lookup failed, no delivery scheduled: {e:?}");
            }
        }
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
                    if let Err(e) = queue::register_stream(&self.redis, &stream).await {
                        warn!(stream = %stream, "register_stream failed: {e}");
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

    /// Creates an event. Returns `(response, true)` on a fresh insert and
    /// `(response, false)` on an idempotent replay.
    #[tracing::instrument(skip(self, req, ctx), fields(application_id = %req.application_id))]
    pub async fn create(
        &self,
        req: CreateEventRequest,
        ctx: RequestContext,
        idempotency_key: Option<&str>,
    ) -> Result<(EventResponse, bool), AppError> {
        Self::validate_payload(&req.payload, &req.payload_type)?;
        validators::validate_tags(&req.tags)?;

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
            .get_event_type(
                &req.event_type_id,
                app.tenant_id,
                req.schema_version.as_deref(),
            )
            .await?
            .ok_or_else(|| {
                warn!("event type not found or archived");
                AppError::NotFound(format!(
                    "Event type not found or archived: {}",
                    req.event_type_id
                ))
            })?;

        let (schema_valid, schema_errors) = match &req.payload_type {
            PayloadType::Text => (true, vec![]),
            PayloadType::Json => Self::validate_against_schema(&et.event_schema.0, &req.payload),
        };

        let payload_type_str = req.payload_type.as_str();
        let app_id = app.id;
        let et_id = et.id;

        if let Some(key) = idempotency_key {
            let hash = idempotency::body_hash_bytes(&req);
            let lock_token = idempotency::acquire_lock(&self.redis, "events", key).await?;

            let result: Result<(EventResponse, bool), AppError> =
                match self.repo.find_by_idempotency_key(app_id, key).await {
                    Ok(Some(row)) => {
                        if row.body_hash.as_deref() == Some(hash.as_slice()) {
                            info!(public_id = %row.public_id, "idempotent replay");
                            Ok((EventResponse::from(row), false))
                        } else {
                            Err(AppError::Conflict(
                                "Idempotency key already used with a different request body".into(),
                                vec![],
                            ))
                        }
                    }
                    Ok(None) => {
                        info!("creating event");
                        match self
                            .repo
                            .create(
                                app,
                                et_id,
                                &req.payload,
                                payload_type_str,
                                &req.tags,
                                Some(key),
                                Some(&hash),
                                schema_valid,
                                &schema_errors,
                                ctx,
                            )
                            .await
                        {
                            Ok(row) => {
                                info!(public_id = %row.public_id, "event created");
                                self.enqueue_for_subscriptions(
                                    row.id,
                                    et_id,
                                    app_id,
                                    row.organization_id,
                                )
                                .await;
                                events_counter().add(
                                    1,
                                    &[
                                        KeyValue::new("tenant_id", row.tenant_id.to_string()),
                                        KeyValue::new("application_id", req.application_id.clone()),
                                    ],
                                );
                                Ok((EventResponse::from(row), true))
                            }
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => Err(e),
                };

            idempotency::release_lock(&self.redis, "events", key, &lock_token).await;
            return result;
        }

        info!("creating event");
        let row = self
            .repo
            .create(
                app,
                et_id,
                &req.payload,
                payload_type_str,
                &req.tags,
                None,
                None,
                schema_valid,
                &schema_errors,
                ctx,
            )
            .await?;
        info!(public_id = %row.public_id, "event created");
        self.enqueue_for_subscriptions(row.id, et_id, app_id, row.organization_id)
            .await;
        events_counter().add(
            1,
            &[
                KeyValue::new("tenant_id", row.tenant_id.to_string()),
                KeyValue::new("application_id", req.application_id.clone()),
            ],
        );
        Ok((EventResponse::from(row), true))
    }

    /// Processes up to 10 events independently. Returns per-item results with HTTP 207.
    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn create_bulk(
        &self,
        req: BulkCreateEventRequest,
        ctx: RequestContext,
    ) -> BulkCreateResponse {
        let mut results = Vec::with_capacity(req.events.len());
        let mut succeeded = 0usize;
        let mut failed = 0usize;

        for (index, item) in req.events.into_iter().enumerate() {
            let idempotency_key = item.idempotency_key.clone();
            let event_req = CreateEventRequest {
                application_id: item.application_id,
                event_type_id: item.event_type_id,
                schema_version: item.schema_version,
                payload: item.payload,
                payload_type: item.payload_type,
                tags: item.tags,
            };

            match self
                .create(event_req, ctx, idempotency_key.as_deref())
                .await
            {
                Ok((ev, created)) => {
                    let status = if created { 201 } else { 200 };
                    results.push(BulkEventResultItem {
                        index,
                        status,
                        event: Some(ev),
                        error: None,
                    });
                    succeeded += 1;
                }
                Err(e) => {
                    let (status, code, message) = e.to_error_info();
                    results.push(BulkEventResultItem {
                        index,
                        status,
                        event: None,
                        error: Some(BulkEventError {
                            code: code.to_owned(),
                            message,
                        }),
                    });
                    failed += 1;
                }
            }
        }

        BulkCreateResponse {
            results,
            succeeded,
            failed,
        }
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
