use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use cron::Schedule;
use redis::AsyncCommands;
use std::str::FromStr;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{common::{call_counter, idempotency, types::RequestContext}, error::AppError};

use super::{
    models::{
        CreateScheduleRequest, ListExecutionsQuery, ListExecutionsResponse, ListSchedulesQuery,
        ListSchedulesResponse, ScheduleExecutionResponse, ScheduleResponse, UpdateScheduleRequest,
    },
    repository::ScheduleRepository,
};

// Atomically picks the member with the lowest score (fewest schedules) and increments it.
// Returns the shard id as a string, or "-1" if the routing set is empty.
const PICK_SHARD_LUA: &str = r#"
local shard = redis.call('ZRANGE', KEYS[1], 0, 0)[1]
if not shard then
    return '-1'
end
redis.call('ZINCRBY', KEYS[1], 1, shard)
return shard
"#;

const ROUTING_KEY: &str = "sched:routing";
const SHARDS_KEY: &str = "sched:shards";

fn pending_key(shard: i16) -> String {
    format!("sched:pending:{shard}")
}

pub struct ScheduleService {
    repo: ScheduleRepository,
    redis: redis::Client,
}

impl ScheduleService {
    pub fn new(repo: ScheduleRepository, redis: redis::Client) -> Self {
        Self { repo, redis }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(name = %req.name))]
    pub async fn create(
        &self,
        req: CreateScheduleRequest,
        ctx: RequestContext,
        idempotency_key: Option<&str>,
    ) -> Result<(ScheduleResponse, bool), AppError> {
        req.validate()?;

        let timezone = req.timezone.as_deref().unwrap_or("UTC");
        let next_run_at = compute_next_run_at(&req.cron_expression, timezone)?;

        let application_id = self
            .repo
            .resolve_application(&req.application_id)
            .await?
            .ok_or_else(|| {
                warn!(application_id = %req.application_id, "application not found");
                AppError::NotFound(format!("Application not found: {}", req.application_id))
            })?;

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

        let event_type_id = self
            .repo
            .resolve_event_type(&req.event_type_id, tenant_id)
            .await?;
        let endpoint_ids = self
            .repo
            .resolve_endpoints(&req.endpoint_ids, tenant_id)
            .await?;

        let assigned_shard = self.pick_shard(tenant_id).await?;

        if let Some(key) = idempotency_key {
            let hash = idempotency::body_hash_bytes(&req);
            let lock_token = idempotency::acquire_lock(&self.redis, "schedules", key).await?;

            let result: Result<(ScheduleResponse, bool), AppError> =
                match self.repo.find_by_idempotency_key(application_id, key).await {
                    Ok(Some(row)) => {
                        if row.body_hash.as_deref() == Some(hash.as_slice()) {
                            info!(public_id = %row.public_id, "idempotent replay");
                            Ok((ScheduleResponse::from(row), false))
                        } else {
                            Err(AppError::Conflict(
                                "Idempotency key already used with a different request body"
                                    .into(),
                                vec![],
                            ))
                        }
                    }
                    Ok(None) => {
                        info!("creating schedule");
                        match self
                            .repo
                            .create(
                                &req.name,
                                req.description.as_deref(),
                                application_id,
                                tenant_id,
                                organization_id,
                                event_type_id,
                                &endpoint_ids,
                                &req.payload,
                                &req.cron_expression,
                                timezone,
                                Some(next_run_at),
                                assigned_shard,
                                Some(key),
                                Some(&hash),
                                ctx,
                            )
                            .await
                        {
                            Ok(row) => {
                                self.zadd_pending(row.assigned_shard, row.id, next_run_at).await;
                                self.zadd_shards(row.assigned_shard).await;
                                info!(public_id = %row.public_id, shard = row.assigned_shard, "schedule created");
                                Ok((ScheduleResponse::from(row), true))
                            }
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => Err(e),
                };

            idempotency::release_lock(&self.redis, "schedules", key, &lock_token).await;
            return result;
        }

        info!("creating schedule");
        let row = self
            .repo
            .create(
                &req.name,
                req.description.as_deref(),
                application_id,
                tenant_id,
                organization_id,
                event_type_id,
                &endpoint_ids,
                &req.payload,
                &req.cron_expression,
                timezone,
                Some(next_run_at),
                assigned_shard,
                None,
                None,
                ctx,
            )
            .await?;

        self.zadd_pending(row.assigned_shard, row.id, next_run_at).await;
        self.zadd_shards(row.assigned_shard).await;

        info!(public_id = %row.public_id, shard = row.assigned_shard, "schedule created");
        Ok((ScheduleResponse::from(row), true))
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
            Some(self.repo.resolve_endpoints(ids, current.tenant_id).await?)
        } else {
            None
        };

        // Recompute next_run_at only if cron_expression or timezone changed.
        let next_run_at = if req.cron_expression.is_some() || req.timezone.is_some() {
            let current = self
                .repo
                .get_by_public_id(&public_id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Schedule not found: {public_id}")))?;
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

        // If next_run_at changed, update the pending sorted set score.
        if let Some(ts) = row.next_run_at {
            self.zadd_pending(row.assigned_shard, row.id, ts).await;
            self.zadd_shards(row.assigned_shard).await;
        }

        info!("schedule updated");
        Ok(ScheduleResponse::from(row))
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete(&self, public_id: String, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting schedule");
        match self.repo.delete(&public_id, ctx).await? {
            None => {
                warn!("schedule not found for delete");
                Err(AppError::NotFound(format!(
                    "Schedule not found: {public_id}"
                )))
            }
            Some((id, shard)) => {
                info!("schedule deleted");
                self.zrem_pending(shard, id).await;
                self.zincrby_routing(shard, -1).await;
                Ok(())
            }
        }
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn restore(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<ScheduleResponse, AppError> {
        info!("restoring schedule");
        let row = self.repo.restore(&public_id, ctx).await?.ok_or_else(|| {
            warn!("schedule not found for restore");
            AppError::NotFound(format!("Schedule not found: {public_id}"))
        })?;

        if let Some(ts) = row.next_run_at {
            self.zadd_pending(row.assigned_shard, row.id, ts).await;
            self.zadd_shards(row.assigned_shard).await;
        }
        self.zincrby_routing(row.assigned_shard, 1).await;

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
        // Remove from pending set while paused; scheduler won't fire it.
        self.zrem_pending(row.assigned_shard, row.id).await;
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
        if let Some(ts) = row.next_run_at {
            self.zadd_pending(row.assigned_shard, row.id, ts).await;
            self.zadd_shards(row.assigned_shard).await;
        }
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

        let (rows, next_cursor_id) = self
            .repo
            .list(
                limit,
                cursor,
                organization_id,
                tenant_id,
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

    // --- Shard assignment ---

    async fn pick_shard(&self, tenant_id: Uuid) -> Result<i16, AppError> {
        // Enterprise path: tenant has a dedicated shard.
        if let Some(shard) = self.repo.get_tenant_shard_affinity(tenant_id).await? {
            self.zincrby_routing(shard, 1).await;
            return Ok(shard);
        }

        // Standard path: pick the lowest-score active shard from the routing sorted set.
        call_counter::inc_redis();
        let mut conn = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(AppError::Redis)?;

        let raw: String = redis::Script::new(PICK_SHARD_LUA)
            .key(ROUTING_KEY)
            .invoke_async(&mut conn)
            .await
            .map_err(AppError::Redis)?;

        let shard = raw
            .parse::<i16>()
            .map_err(|_| AppError::Internal(format!("invalid shard id from routing set: {raw}")))?;

        if shard < 0 {
            return Err(AppError::Internal(
                "no active scheduler shards configured".into(),
            ));
        }

        Ok(shard)
    }

    // --- Best-effort Redis helpers ---
    // These update Redis sorted sets after the DB write succeeds. Failures are
    // logged and swallowed — the reconciliation task corrects drift every 2 min.

    async fn zadd_pending(&self, shard: i16, schedule_id: Uuid, next_run_at: DateTime<Utc>) {
        call_counter::inc_redis();
        let Ok(mut conn) = self.redis.get_multiplexed_async_connection().await else {
            warn!(shard, "redis unavailable; skipping ZADD sched:pending");
            return;
        };
        let score = next_run_at.timestamp() as f64;
        let _: Result<(), _> = conn
            .zadd(pending_key(shard), schedule_id.to_string(), score)
            .await;
    }

    async fn zrem_pending(&self, shard: i16, schedule_id: Uuid) {
        call_counter::inc_redis();
        let Ok(mut conn) = self.redis.get_multiplexed_async_connection().await else {
            warn!(shard, "redis unavailable; skipping ZREM sched:pending");
            return;
        };
        let _: Result<(), _> = conn
            .zrem(pending_key(shard), schedule_id.to_string())
            .await;
    }

    // Adds the shard to the scheduler discovery set (sched:shards).
    // Score = current unix_ms. GT flag means the score only moves forward,
    // protecting against clock skew and ensuring workers see the latest version.
    async fn zadd_shards(&self, shard: i16) {
        call_counter::inc_redis();
        let Ok(mut conn) = self.redis.get_multiplexed_async_connection().await else {
            warn!(shard, "redis unavailable; skipping ZADD sched:shards");
            return;
        };
        let now_ms = Utc::now().timestamp_millis() as f64;
        let _: Result<(), _> = redis::cmd("ZADD")
            .arg(SHARDS_KEY)
            .arg("GT")
            .arg(now_ms)
            .arg(shard.to_string())
            .query_async(&mut conn)
            .await;
    }

    async fn zincrby_routing(&self, shard: i16, delta: i64) {
        call_counter::inc_redis();
        let Ok(mut conn) = self.redis.get_multiplexed_async_connection().await else {
            return;
        };
        let _: Result<(), _> = conn.zincr(ROUTING_KEY, shard.to_string(), delta).await;
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

fn compute_next_run_at(cron_expr: &str, timezone: &str) -> Result<DateTime<Utc>, AppError> {
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
        .ok_or_else(|| AppError::BadRequest("cron expression produces no future executions".into()))
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
