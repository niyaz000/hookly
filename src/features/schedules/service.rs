use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{common::types::RequestContext, error::AppError};

use super::{
    models::{
        CreateScheduleRequest, ListExecutionsQuery, ListExecutionsResponse, ListSchedulesQuery,
        ListSchedulesResponse, ScheduleExecutionResponse, ScheduleResponse, UpdateScheduleRequest,
    },
    repository::ScheduleRepository,
};

pub struct ScheduleService {
    repo: ScheduleRepository,
}

impl ScheduleService {
    pub fn new(repo: ScheduleRepository) -> Self {
        Self { repo }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(name = %req.name))]
    pub async fn create(
        &self,
        req: CreateScheduleRequest,
        ctx: RequestContext,
    ) -> Result<ScheduleResponse, AppError> {
        req.validate()?;

        let timezone = req.timezone.as_deref().unwrap_or("UTC");
        let next_run_at = compute_next_run_at(&req.cron_expression, timezone)?;

        info!("creating schedule");
        let event_type_id = self
            .repo
            .resolve_event_type(&req.event_type_id, req.tenant_id)
            .await?;
        let endpoint_ids = self
            .repo
            .resolve_endpoints(&req.endpoint_ids, req.tenant_id)
            .await?;

        let row = self
            .repo
            .create(
                &req.name,
                req.description.as_deref(),
                req.tenant_id,
                req.organization_id,
                event_type_id,
                &endpoint_ids,
                &req.payload,
                &req.cron_expression,
                timezone,
                Some(next_run_at),
                ctx,
            )
            .await?;

        info!(public_id = %row.public_id, "schedule created");
        Ok(ScheduleResponse::from(row))
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_public_id(&self, public_id: String) -> Result<ScheduleResponse, AppError> {
        info!("fetching schedule");
        self.repo
            .get_by_public_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("schedule not found");
                AppError::NotFound(format!("Schedule not found: {public_id}"))
            })
            .map(ScheduleResponse::from)
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn update(
        &self,
        public_id: String,
        req: UpdateScheduleRequest,
        ctx: RequestContext,
    ) -> Result<ScheduleResponse, AppError> {
        req.validate()?;
        info!("updating schedule");

        let endpoint_ids = if let Some(ref ids) = req.endpoint_ids {
            let current = self
                .repo
                .get_by_public_id(&public_id)
                .await?
                .ok_or_else(|| {
                    warn!("schedule not found for update");
                    AppError::NotFound(format!("Schedule not found: {public_id}"))
                })?;
            Some(
                self.repo
                    .resolve_endpoints(ids, current.tenant_id)
                    .await?,
            )
        } else {
            None
        };

        // Recompute next_run_at only if cron_expression or timezone changed.
        let next_run_at = if req.cron_expression.is_some() || req.timezone.is_some() {
            let current = self.repo.get_by_public_id(&public_id).await?.ok_or_else(|| {
                AppError::NotFound(format!("Schedule not found: {public_id}"))
            })?;
            let expr = req
                .cron_expression
                .as_deref()
                .unwrap_or(&current.cron_expression);
            let tz = req.timezone.as_deref().unwrap_or(&current.timezone);
            Some(Some(compute_next_run_at(expr, tz)?))
        } else {
            None
        };

        let row = self
            .repo
            .update(&public_id, &req, endpoint_ids, next_run_at, ctx)
            .await?
            .ok_or_else(|| {
                warn!("schedule not found for update");
                AppError::NotFound(format!("Schedule not found: {public_id}"))
            })?;

        info!("schedule updated");
        Ok(ScheduleResponse::from(row))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete(&self, public_id: String, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting schedule");
        let deleted = self.repo.delete(&public_id, ctx).await?;
        if !deleted {
            warn!("schedule not found for delete");
            return Err(AppError::NotFound(format!(
                "Schedule not found: {public_id}"
            )));
        }
        info!("schedule deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn restore(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<ScheduleResponse, AppError> {
        info!("restoring schedule");
        let row = self
            .repo
            .restore(&public_id, ctx)
            .await?
            .ok_or_else(|| {
                warn!("schedule not found for restore");
                AppError::NotFound(format!("Schedule not found: {public_id}"))
            })?;
        info!("schedule restored");
        Ok(ScheduleResponse::from(row))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn pause(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<ScheduleResponse, AppError> {
        info!("pausing schedule");
        let row = self
            .repo
            .set_status(&public_id, "paused", ctx)
            .await?
            .ok_or_else(|| {
                warn!("schedule not found for pause");
                AppError::NotFound(format!("Schedule not found: {public_id}"))
            })?;
        info!("schedule paused");
        Ok(ScheduleResponse::from(row))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn resume(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<ScheduleResponse, AppError> {
        info!("resuming schedule");
        let row = self
            .repo
            .set_status(&public_id, "active", ctx)
            .await?
            .ok_or_else(|| {
                warn!("schedule not found for resume");
                AppError::NotFound(format!("Schedule not found: {public_id}"))
            })?;
        info!("schedule resumed");
        Ok(ScheduleResponse::from(row))
    }

    #[tracing::instrument(skip(self, _ctx))]
    pub async fn trigger(
        &self,
        public_id: String,
        _ctx: RequestContext,
    ) -> Result<ScheduleExecutionResponse, AppError> {
        info!("triggering schedule");
        let row = self
            .repo
            .get_by_public_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("schedule not found for trigger");
                AppError::NotFound(format!("Schedule not found: {public_id}"))
            })?;

        let execution = self
            .repo
            .create_execution(row.id, row.tenant_id, row.organization_id, Utc::now())
            .await?;

        info!(
            public_id = %execution.public_id,
            "schedule execution created"
        );
        Ok(ScheduleExecutionResponse::from(execution))
    }

    #[tracing::instrument(skip(self))]
    pub async fn list(&self, query: ListSchedulesQuery) -> Result<ListSchedulesResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

        let (rows, next_cursor_id) = self
            .repo
            .list(
                limit,
                cursor,
                query.organization_id,
                query.tenant_id,
                query.status.as_deref(),
            )
            .await?;

        Ok(ListSchedulesResponse {
            items: rows.into_iter().map(ScheduleResponse::from).collect(),
            next_cursor: next_cursor_id.map(encode_cursor),
            limit,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_executions(
        &self,
        public_id: String,
        query: ListExecutionsQuery,
    ) -> Result<ListExecutionsResponse, AppError> {
        let schedule_id = self
            .repo
            .get_schedule_id_by_public_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("schedule not found for listing executions");
                AppError::NotFound(format!("Schedule not found: {public_id}"))
            })?;

        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let cursor = query.cursor.as_deref().map(decode_cursor).transpose()?;

        let (rows, next_cursor_id) = self
            .repo
            .list_executions(schedule_id, limit, cursor)
            .await?;

        Ok(ListExecutionsResponse {
            items: rows
                .into_iter()
                .map(ScheduleExecutionResponse::from)
                .collect(),
            next_cursor: next_cursor_id.map(encode_cursor),
            limit,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_execution(
        &self,
        public_id: String,
        exec_public_id: String,
    ) -> Result<ScheduleExecutionResponse, AppError> {
        info!("fetching schedule execution");
        self.repo
            .get_execution(&public_id, &exec_public_id)
            .await?
            .ok_or_else(|| {
                warn!("schedule execution not found");
                AppError::NotFound(format!("Execution not found: {exec_public_id}"))
            })
            .map(ScheduleExecutionResponse::from)
    }
}

// --- Cron helpers ---

fn normalize_cron(expr: &str) -> Result<String, AppError> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    match parts.len() {
        5 => Ok(format!("0 {} *", expr)),
        7 => Ok(expr.to_string()),
        n => Err(AppError::BadRequest(format!(
            "cron expression must have 5 fields (min hour dom month dow), got {n}"
        ))),
    }
}

fn compute_next_run_at(
    cron_expr: &str,
    timezone: &str,
) -> Result<chrono::DateTime<Utc>, AppError> {
    let seven_field = normalize_cron(cron_expr)?;

    let schedule = Schedule::from_str(&seven_field)
        .map_err(|e| AppError::BadRequest(format!("invalid cron expression: {e}")))?;

    let tz: chrono_tz::Tz = timezone
        .parse()
        .map_err(|_| AppError::BadRequest(format!("invalid timezone: {timezone}")))?;

    schedule
        .upcoming(tz)
        .next()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or_else(|| {
            AppError::BadRequest("cron expression produces no future executions".into())
        })
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
