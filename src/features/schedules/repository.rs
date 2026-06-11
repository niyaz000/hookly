use chrono::{DateTime, Utc};
use sqlx::{types::Json, PgPool, QueryBuilder};
use tracing::debug;
use uuid::Uuid;

use crate::{common::{types::RequestContext, NanoId}, error::AppError};

use super::models::{ScheduleExecutionRow, ScheduleRow, UpdateScheduleRequest};

const SCHEDULE_SELECT: &str = r#"
    SELECT
        s.id, s.public_id, s.name, s.description,
        s.tenant_id, s.organization_id, s.event_type_id,
        s.payload, s.cron_expression, s.timezone, s.status,
        s.next_run_at, s.last_run_at, s.last_run_status,
        s.created_by, s.updated_by, s.request_id, s.version,
        s.created_at, s.updated_at, s.deleted_at, s.assigned_shard,
        et.public_id AS event_type_public_id,
        COALESCE(
            array_agg(e.public_id ORDER BY e.public_id) FILTER (WHERE e.id IS NOT NULL),
            '{}'::TEXT[]
        ) AS endpoint_public_ids
    FROM schedules s
    JOIN event_types et ON et.id = s.event_type_id
    LEFT JOIN schedule_endpoints se ON se.schedule_id = s.id
    LEFT JOIN endpoints e ON e.id = se.endpoint_id AND e.deleted_at IS NULL
"#;

const SCHEDULE_GROUP_BY: &str = "GROUP BY s.id, et.public_id";

pub struct ScheduleRepository {
    pool: PgPool,
}

impl ScheduleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // --- FK resolution helpers ---

    pub async fn resolve_event_type(
        &self,
        public_id: &str,
        tenant_id: Uuid,
    ) -> Result<Uuid, AppError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM event_types \
             WHERE public_id = $1 AND tenant_id = $2 AND archived = FALSE AND deleted_at IS NULL",
        )
        .bind(public_id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("EventType not found: {public_id}")))
    }

    pub async fn resolve_endpoints(
        &self,
        public_ids: &[String],
        tenant_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        if public_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT id, public_id FROM endpoints \
             WHERE public_id = ANY($1) AND tenant_id = $2 AND deleted_at IS NULL",
        )
        .bind(public_ids)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        let missing: Vec<&str> = public_ids
            .iter()
            .filter(|pid| !rows.iter().any(|(_, found)| found == *pid))
            .map(|s| s.as_str())
            .collect();

        if !missing.is_empty() {
            return Err(AppError::NotFound(format!(
                "Endpoints not found: {}",
                missing.join(", ")
            )));
        }

        Ok(rows.into_iter().map(|(id, _)| id).collect())
    }

    pub async fn get_tenant_shard_affinity(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<i16>, AppError> {
        sqlx::query_scalar::<_, i16>(
            "SELECT shard_id FROM tenant_shard_affinity WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn get_schedule_id_by_public_id(
        &self,
        public_id: &str,
    ) -> Result<Option<Uuid>, AppError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM schedules WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    // --- Internal fetch ---

    async fn get_full_by_id(&self, id: Uuid) -> Result<Option<ScheduleRow>, AppError> {
        let sql = format!("{} WHERE s.id = $1 {}", SCHEDULE_SELECT, SCHEDULE_GROUP_BY);
        sqlx::query_as::<_, ScheduleRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)
    }

    // --- CRUD ---

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
        tenant_id: Uuid,
        organization_id: Uuid,
        event_type_id: Uuid,
        endpoint_ids: &[Uuid],
        payload: &serde_json::Value,
        cron_expression: &str,
        timezone: &str,
        next_run_at: Option<DateTime<Utc>>,
        assigned_shard: i16,
        ctx: RequestContext,
    ) -> Result<ScheduleRow, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("sch_{}", NanoId::generate(20));

        debug!(public_id = %public_id, "inserting schedule");

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO schedules (
                id, public_id, name, description,
                tenant_id, organization_id, event_type_id,
                payload, cron_expression, timezone,
                next_run_at, assigned_shard,
                created_by, updated_by, request_id,
                version, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7,
                $8, $9, $10,
                $11, $12,
                $13, $13, $14,
                1, NOW(), NOW()
            )
            "#,
        )
        .bind(id)
        .bind(&public_id)
        .bind(name)
        .bind(description)
        .bind(tenant_id)
        .bind(organization_id)
        .bind(event_type_id)
        .bind(Json(payload))
        .bind(cron_expression)
        .bind(timezone)
        .bind(next_run_at)
        .bind(assigned_shard)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .execute(&mut *tx)
        .await?;

        for ep_id in endpoint_ids {
            sqlx::query(
                "INSERT INTO schedule_endpoints (schedule_id, endpoint_id) VALUES ($1, $2)",
            )
            .bind(id)
            .bind(ep_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        self.get_full_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("schedule created but not found on fetch".into()))
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<ScheduleRow>, AppError> {
        debug!(public_id = %public_id, "querying schedule");

        let sql = format!(
            "{} WHERE s.public_id = $1 AND s.deleted_at IS NULL {}",
            SCHEDULE_SELECT, SCHEDULE_GROUP_BY
        );
        sqlx::query_as::<_, ScheduleRow>(&sql)
            .bind(public_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)
    }

    pub async fn update(
        &self,
        public_id: &str,
        req: &UpdateScheduleRequest,
        endpoint_ids: Option<Vec<Uuid>>,
        next_run_at: Option<Option<DateTime<Utc>>>,
        ctx: RequestContext,
    ) -> Result<Option<ScheduleRow>, AppError> {
        debug!(public_id = %public_id, "updating schedule");

        let mut tx = self.pool.begin().await?;

        let updated_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE schedules SET
                name            = COALESCE($1, name),
                description     = COALESCE($2, description),
                payload         = COALESCE($3, payload),
                cron_expression = COALESCE($4, cron_expression),
                timezone        = COALESCE($5, timezone),
                next_run_at     = CASE WHEN $6::bool THEN $7 ELSE next_run_at END,
                updated_by      = $8,
                request_id      = $9,
                version         = version + 1,
                updated_at      = NOW()
            WHERE public_id = $10 AND deleted_at IS NULL
            RETURNING id
            "#,
        )
        .bind(req.name.as_deref())
        .bind(req.description.as_deref())
        .bind(req.payload.as_ref().map(Json))
        .bind(req.cron_expression.as_deref())
        .bind(req.timezone.as_deref())
        .bind(next_run_at.is_some())
        .bind(next_run_at.flatten())
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .bind(public_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(id) = updated_id else {
            tx.rollback().await?;
            return Ok(None);
        };

        if let Some(ids) = endpoint_ids {
            sqlx::query("DELETE FROM schedule_endpoints WHERE schedule_id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;

            for ep_id in &ids {
                sqlx::query(
                    "INSERT INTO schedule_endpoints (schedule_id, endpoint_id) VALUES ($1, $2)",
                )
                .bind(id)
                .bind(ep_id)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        self.get_full_by_id(id).await
    }

    // Returns (id, assigned_shard) of the deleted row, or None if not found.
    pub async fn delete(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<Option<(Uuid, i16)>, AppError> {
        debug!(public_id = %public_id, "soft deleting schedule");

        sqlx::query_as::<_, (Uuid, i16)>(
            r#"
            UPDATE schedules SET
                deleted_at = NOW(),
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1 AND deleted_at IS NULL
            RETURNING id, assigned_shard
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn restore(
        &self,
        public_id: &str,
        ctx: RequestContext,
    ) -> Result<Option<ScheduleRow>, AppError> {
        debug!(public_id = %public_id, "restoring schedule");

        let id: Option<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE schedules SET
                deleted_at = NULL,
                status     = 'active',
                updated_by = $2,
                request_id = $3,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1
            RETURNING id
            "#,
        )
        .bind(public_id)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        match id {
            Some(id) => self.get_full_by_id(id).await,
            None => Ok(None),
        }
    }

    pub async fn set_status(
        &self,
        public_id: &str,
        status: &str,
        ctx: RequestContext,
    ) -> Result<Option<ScheduleRow>, AppError> {
        debug!(public_id = %public_id, status = %status, "setting schedule status");

        let id: Option<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE schedules SET
                status     = $2,
                updated_by = $3,
                request_id = $4,
                version    = version + 1,
                updated_at = NOW()
            WHERE public_id = $1 AND deleted_at IS NULL
            RETURNING id
            "#,
        )
        .bind(public_id)
        .bind(status)
        .bind(ctx.created_by)
        .bind(ctx.request_id)
        .fetch_optional(&self.pool)
        .await?;

        match id {
            Some(id) => self.get_full_by_id(id).await,
            None => Ok(None),
        }
    }

    pub async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        organization_id: Option<Uuid>,
        tenant_id: Option<Uuid>,
        status: Option<&str>,
    ) -> Result<(Vec<ScheduleRow>, Option<Uuid>), AppError> {
        debug!(limit = limit, "listing schedules");

        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "{} WHERE s.deleted_at IS NULL",
            SCHEDULE_SELECT
        ));

        if let Some(c) = cursor {
            qb.push(" AND s.id > ").push_bind(c);
        }
        if let Some(org_id) = organization_id {
            qb.push(" AND s.organization_id = ").push_bind(org_id);
        }
        if let Some(t_id) = tenant_id {
            qb.push(" AND s.tenant_id = ").push_bind(t_id);
        }
        if let Some(s) = status {
            qb.push(" AND s.status = ").push_bind(s.to_string());
        }

        qb.push(format!(
            " {} ORDER BY s.id ASC LIMIT ",
            SCHEDULE_GROUP_BY
        ));
        qb.push_bind(limit + 1);

        let mut rows: Vec<ScheduleRow> = qb
            .build_query_as::<ScheduleRow>()
            .fetch_all(&self.pool)
            .await?;

        let next_cursor = if rows.len() as i64 > limit {
            rows.pop().map(|r| r.id)
        } else {
            None
        };

        Ok((rows, next_cursor))
    }

    // --- Schedule executions ---

    pub async fn create_execution(
        &self,
        schedule_id: Uuid,
        tenant_id: Uuid,
        organization_id: Uuid,
        triggered_at: DateTime<Utc>,
    ) -> Result<ScheduleExecutionRow, AppError> {
        let id = Uuid::now_v7();
        let public_id = format!("sxe_{}", NanoId::generate(20));

        debug!(public_id = %public_id, "inserting schedule execution");

        sqlx::query(
            r#"
            INSERT INTO schedule_executions (
                id, public_id, schedule_id,
                tenant_id, organization_id,
                status, triggered_at, created_at
            ) VALUES (
                $1, $2, $3, $4, $5, 'pending', $6, NOW()
            )
            "#,
        )
        .bind(id)
        .bind(&public_id)
        .bind(schedule_id)
        .bind(tenant_id)
        .bind(organization_id)
        .bind(triggered_at)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, ScheduleExecutionRow>(
            r#"
            SELECT se.id, se.public_id, se.schedule_id, s.public_id AS schedule_public_id,
                   se.tenant_id, se.organization_id, se.status,
                   se.triggered_at, se.started_at, se.completed_at,
                   se.error_message, se.created_at
            FROM schedule_executions se
            JOIN schedules s ON s.id = se.schedule_id
            WHERE se.id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)
    }

    pub async fn list_executions(
        &self,
        schedule_id: Uuid,
        limit: i64,
        cursor: Option<Uuid>,
    ) -> Result<(Vec<ScheduleExecutionRow>, Option<Uuid>), AppError> {
        debug!(schedule_id = %schedule_id, limit = limit, "listing schedule executions");

        let mut qb = QueryBuilder::<sqlx::Postgres>::new(
            r#"SELECT se.id, se.public_id, se.schedule_id, s.public_id AS schedule_public_id,
                      se.tenant_id, se.organization_id, se.status,
                      se.triggered_at, se.started_at, se.completed_at,
                      se.error_message, se.created_at
               FROM schedule_executions se
               JOIN schedules s ON s.id = se.schedule_id
               WHERE se.schedule_id = "#,
        );
        qb.push_bind(schedule_id);

        if let Some(c) = cursor {
            qb.push(" AND se.id < ").push_bind(c);
        }

        qb.push(" ORDER BY se.id DESC LIMIT ").push_bind(limit + 1);

        let mut rows: Vec<ScheduleExecutionRow> = qb
            .build_query_as::<ScheduleExecutionRow>()
            .fetch_all(&self.pool)
            .await?;

        let next_cursor = if rows.len() as i64 > limit {
            rows.pop().map(|r| r.id)
        } else {
            None
        };

        Ok((rows, next_cursor))
    }

    pub async fn get_execution(
        &self,
        schedule_public_id: &str,
        exec_public_id: &str,
    ) -> Result<Option<ScheduleExecutionRow>, AppError> {
        sqlx::query_as::<_, ScheduleExecutionRow>(
            r#"
            SELECT se.id, se.public_id, se.schedule_id, s.public_id AS schedule_public_id,
                   se.tenant_id, se.organization_id, se.status,
                   se.triggered_at, se.started_at, se.completed_at,
                   se.error_message, se.created_at
            FROM schedule_executions se
            JOIN schedules s ON s.id = se.schedule_id
            WHERE se.public_id = $1
              AND s.public_id  = $2
            "#,
        )
        .bind(exec_public_id)
        .bind(schedule_public_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::from)
    }
}
