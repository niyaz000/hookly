use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::nano_id::NanoId;
use crate::error::AppError;
use crate::features::delivery::models::{DeliveryJobRow, UnqueuedJob, WorkerJob};

#[derive(Clone)]
pub struct DeliveryRepository {
    db: PgPool,
}

impl DeliveryRepository {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    // --- Event service ---

    /// Looks up the org's delivery tier. Defaults to "default" if the org has
    /// no row in the organizations table (i.e. before migration 006 or before
    /// the org is explicitly registered).
    pub async fn get_org_tier(&self, org_id: Uuid) -> String {
        sqlx::query_scalar::<_, String>("SELECT tier FROM organizations WHERE id = $1")
            .bind(org_id)
            .fetch_optional(&self.db)
            .await
            .unwrap_or(None)
            .unwrap_or_else(|| "default".to_string())
    }

    pub async fn create_job(
        &self,
        event_id: Uuid,
        endpoint_id: Uuid,
        organization_id: Uuid,
        stream_name: &str,
    ) -> Result<DeliveryJobRow, AppError> {
        let public_id = format!("dj_{}", NanoId::new());
        sqlx::query_as::<_, DeliveryJobRow>(
            r#"INSERT INTO delivery_jobs
               (public_id, event_id, endpoint_id, organization_id, stream_name)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING *"#,
        )
        .bind(&public_id)
        .bind(event_id)
        .bind(endpoint_id)
        .bind(organization_id)
        .bind(stream_name)
        .fetch_one(&self.db)
        .await
        .map_err(AppError::from)
    }

    pub async fn mark_enqueued(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE delivery_jobs SET enqueued_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.db)
            .await
            .map_err(AppError::from)?;
        Ok(())
    }

    // --- Worker consumer ---

    #[allow(dead_code)]
    /// Fetches all data required for a single HTTP delivery attempt.
    ///
    /// Returns `None` when the job is not in a deliverable state (already
    /// succeeded / failed) or the endpoint is no longer active — the worker
    /// should XACK and move on.
    pub async fn get_job_for_delivery(
        &self,
        public_id: &str,
    ) -> Result<Option<WorkerJob>, AppError> {
        sqlx::query_as::<_, WorkerJob>(
            r#"SELECT
                dj.id             AS job_id,
                dj.public_id      AS job_public_id,
                dj.event_id,
                dj.endpoint_id,
                dj.organization_id,
                dj.attempt,
                dj.max_attempts,
                ev.public_id      AS event_public_id,
                ev.payload,
                ev.tenant_id,
                ep.config         AS endpoint_config,
                ep.rate_limit_per_minute,
                es.secret         AS encrypted_secret
               FROM delivery_jobs dj
               JOIN events    ev ON ev.id = dj.event_id
               JOIN endpoints ep ON ep.id = dj.endpoint_id
               JOIN LATERAL (
                   SELECT secret
                   FROM endpoint_secrets
                   WHERE endpoint_id = ep.id
                     AND is_active = TRUE
                     AND (expires_at IS NULL OR expires_at > NOW())
                   ORDER BY expires_at NULLS FIRST
                   LIMIT 1
               ) es ON TRUE
               WHERE dj.public_id = $1
                 AND dj.status IN ('pending', 'retrying')
                 AND ep.status = 'active'
                 AND ep.deleted_at IS NULL"#,
        )
        .bind(public_id)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    pub async fn insert_attempt(
        &self,
        delivery_job_id: Uuid,
        event_id: Uuid,
        endpoint_id: Uuid,
        attempt_number: i32,
        status: &str,
        http_status: Option<i32>,
        response_body: Option<&str>,
        latency_ms: Option<i32>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"INSERT INTO delivery_attempts
               (delivery_job_id, event_id, endpoint_id,
                attempt_number, status, http_status, response_body, latency_ms)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(delivery_job_id)
        .bind(event_id)
        .bind(endpoint_id)
        .bind(attempt_number)
        .bind(status)
        .bind(http_status)
        .bind(response_body)
        .bind(latency_ms)
        .execute(&self.db)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn complete_job(&self, job_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE delivery_jobs SET status = 'success', attempt = attempt + 1 WHERE id = $1",
        )
        .bind(job_id)
        .execute(&self.db)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn fail_job(&self, job_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE delivery_jobs SET status = 'failed', attempt = attempt + 1 WHERE id = $1",
        )
        .bind(job_id)
        .execute(&self.db)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    /// Schedules a retry by setting status='retrying', recording when to next
    /// attempt, and clearing enqueued_at so the outbox poller re-enqueues the
    /// job once retry_after has passed.
    #[allow(dead_code)]
    pub async fn schedule_retry(
        &self,
        job_id: Uuid,
        retry_after: DateTime<Utc>,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"UPDATE delivery_jobs
               SET status      = 'retrying',
                   attempt     = attempt + 1,
                   retry_after = $2,
                   enqueued_at = NULL
               WHERE id = $1"#,
        )
        .bind(job_id)
        .bind(retry_after)
        .execute(&self.db)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    // --- Retry ---

    /// Resets a failed delivery job back to pending for re-delivery.
    /// Returns the job row (with stream_name) if it was in 'failed' state,
    /// or None if not found / not in a retryable state.
    pub async fn reset_for_retry(
        &self,
        public_id: &str,
    ) -> Result<Option<DeliveryJobRow>, AppError> {
        sqlx::query_as::<_, DeliveryJobRow>(
            r#"UPDATE delivery_jobs
               SET status = 'pending',
                   enqueued_at = NULL
               WHERE public_id = $1
                 AND status = 'failed'
               RETURNING *"#,
        )
        .bind(public_id)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    pub async fn exists(&self, public_id: &str) -> Result<bool, AppError> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM delivery_jobs WHERE public_id = $1)",
        )
        .bind(public_id)
        .fetch_one(&self.db)
        .await
        .map_err(AppError::from)
    }

    // --- Outbox poller ---

    /// Returns jobs that need to be (re-)enqueued into Redis:
    /// - New pending jobs that never made it into the stream (XADD failure safety net).
    /// - Retrying jobs whose retry_after timestamp has passed.
    #[allow(dead_code)]
    pub async fn list_unqueued(&self, limit: i64) -> Result<Vec<UnqueuedJob>, AppError> {
        sqlx::query_as::<_, UnqueuedJob>(
            r#"SELECT id, public_id, stream_name
               FROM delivery_jobs
               WHERE (
                   status = 'pending'
                   AND enqueued_at IS NULL
                   AND created_at < NOW() - INTERVAL '5 seconds'
               ) OR (
                   status = 'retrying'
                   AND retry_after <= NOW()
                   AND enqueued_at IS NULL
               )
               ORDER BY created_at
               LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(&self.db)
        .await
        .map_err(AppError::from)
    }
}
