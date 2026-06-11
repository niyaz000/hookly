use chrono::Utc;
use sqlx::{types::Json, PgPool};
use tracing::{info, warn};
use uuid::Uuid;

use hookly::common::nano_id::NanoId;
use hookly::queue;

/// Atomically fires a due schedule:
///   - Fetches the schedule + its endpoint IDs from the DB.
///   - For each endpoint: inserts an event and a delivery_job in one transaction.
///   - Updates next_run_at and last_run_at on the schedule row.
///   - Re-scores the schedule in the Redis sorted set.
///   - Best-effort XADD to the delivery stream (outbox poller covers failures).
///
/// Returns `true` on success, `false` if the schedule was deleted/paused/not found.
pub async fn fire_schedule(
    schedule_id: Uuid,
    db: &PgPool,
    redis: &redis::Client,
) -> bool {
    match fire_inner(schedule_id, db, redis).await {
        Ok(fired) => fired,
        Err(e) => {
            warn!(schedule_id = %schedule_id, error = ?e, "schedule fire failed");
            false
        }
    }
}

/// Detailed fire result type used internally.
#[derive(Debug)]
struct FireResult {
    schedule_public_id: String,
    endpoint_count: usize,
}

async fn fire_inner(
    schedule_id: Uuid,
    db: &PgPool,
    redis: &redis::Client,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // ── 1. Fetch schedule and its active endpoint IDs ────────────────────────
    let row: Option<ScheduleFireRow> = sqlx::query_as(
        r#"
        SELECT
            s.id, s.public_id, s.tenant_id, s.organization_id,
            s.event_type_id, s.payload, s.cron_expression, s.timezone,
            s.assigned_shard, s.status,
            COALESCE(
                array_agg(se.endpoint_id) FILTER (WHERE e.id IS NOT NULL),
                '{}'::uuid[]
            ) AS endpoint_ids,
            COALESCE(
                array_agg(e.public_id ORDER BY e.public_id) FILTER (WHERE e.id IS NOT NULL),
                '{}'::text[]
            ) AS endpoint_public_ids,
            COALESCE(
                array_agg(
                    CASE WHEN o.tier IS NOT NULL THEN o.tier ELSE 'default' END
                    ORDER BY e.public_id
                ) FILTER (WHERE e.id IS NOT NULL),
                '{}'::text[]
            ) AS tiers
        FROM schedules s
        LEFT JOIN schedule_endpoints se ON se.schedule_id = s.id
        LEFT JOIN endpoints e ON e.id = se.endpoint_id AND e.deleted_at IS NULL AND e.status = 'active'
        LEFT JOIN organizations o ON o.id = s.organization_id
        WHERE s.id = $1 AND s.deleted_at IS NULL AND s.status = 'active'
        GROUP BY s.id
        "#,
    )
    .bind(schedule_id)
    .fetch_optional(db)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            // Schedule was deleted or paused — nothing to do.
            return Ok(false);
        }
    };

    if row.endpoint_ids.is_empty() {
        warn!(schedule_public_id = %row.public_id, "schedule has no active endpoints, skipping fire");
        return Ok(false);
    }

    // ── 2. Compute next_run_at ───────────────────────────────────────────────
    let next_run_at = compute_next_run_at(&row.cron_expression, &row.timezone)?;

    // ── 3. Transactional fire ────────────────────────────────────────────────
    let triggered_at = Utc::now();
    let mut tx = db.begin().await?;

    // Insert one event per endpoint. Events are immutable; each gets its own row
    // so that delivery_jobs can reference a specific (event, endpoint) pair.
    let tier = row.tiers.first().map(|s| s.as_str()).unwrap_or("default");
    let stream = queue::stream_for_tier(tier, row.organization_id);

    let mut job_ids: Vec<(Uuid, String)> = Vec::with_capacity(row.endpoint_ids.len());

    for (endpoint_id, endpoint_public_id) in row.endpoint_ids.iter().zip(row.endpoint_public_ids.iter()) {
        let event_public_id = format!("evn_{}", NanoId::new());
        let event_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO events
               (public_id, application_id, event_type_id, endpoint_id,
                tenant_id, organization_id,
                payload, tags, request_id, created_by)
               VALUES ($1,
                   (SELECT id FROM applications
                    WHERE tenant_id = $2 AND deleted_at IS NULL
                    LIMIT 1),
                   $3, $4, $2, $5,
                   $6::jsonb, '{}'::jsonb, $7, $8)
               RETURNING id"#,
        )
        .bind(&event_public_id)
        .bind(row.tenant_id)
        .bind(row.event_type_id)
        .bind(endpoint_id)
        .bind(row.organization_id)
        .bind(Json(&row.payload))
        .bind(Uuid::now_v7()) // request_id for the scheduler-triggered event
        .bind(Uuid::nil())    // created_by — system/scheduler
        .fetch_one(&mut *tx)
        .await?;

        let job_public_id = format!("dj_{}", NanoId::new());
        let _: () = sqlx::query(
            r#"INSERT INTO delivery_jobs
               (public_id, event_id, endpoint_id, organization_id, stream_name)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(&job_public_id)
        .bind(event_id)
        .bind(endpoint_id)
        .bind(row.organization_id)
        .bind(&stream)
        .execute(&mut *tx)
        .await
        .map(|_| ())?;

        job_ids.push((event_id, job_public_id));

        let _ = endpoint_public_id; // suppress unused warning
    }

    // Update schedule timestamps and increment version.
    sqlx::query(
        r#"UPDATE schedules SET
               next_run_at     = $1,
               last_run_at     = $2,
               last_run_status = 'fired',
               version         = version + 1,
               updated_at      = NOW()
           WHERE id = $3"#,
    )
    .bind(next_run_at)
    .bind(triggered_at)
    .bind(schedule_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // ── 4. Re-score in Redis sorted set (best effort) ────────────────────────
    let shard = row.assigned_shard;
    let pending_key = format!("sched:pending:{shard}");
    if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
        let score = next_run_at.timestamp() as f64;
        let _: Result<(), _> = redis::cmd("ZADD")
            .arg(&pending_key)
            .arg(score)
            .arg(schedule_id.to_string())
            .query_async(&mut conn)
            .await;
    }

    // ── 5. Best-effort XADD for each delivery job ────────────────────────────
    for (_event_id, job_public_id) in &job_ids {
        if let Err(e) = queue::enqueue(redis, &stream, job_public_id).await {
            warn!(job_public_id = %job_public_id, error = %e, "XADD failed; outbox poller will retry");
        }
    }

    let result = FireResult {
        schedule_public_id: row.public_id,
        endpoint_count: row.endpoint_ids.len(),
    };

    info!(
        schedule_public_id = %result.schedule_public_id,
        endpoint_count = result.endpoint_count,
        "schedule fired"
    );

    Ok(true)
}

fn compute_next_run_at(
    cron_expr: &str,
    timezone: &str,
) -> Result<chrono::DateTime<Utc>, Box<dyn std::error::Error + Send + Sync>> {
    use chrono::TimeZone;
    use chrono_tz::Tz;

    let tz: Tz = timezone.parse().map_err(|e| {
        format!("invalid timezone '{}': {}", timezone, e)
    })?;
    let schedule = cron::Schedule::try_from(cron_expr)
        .map_err(|e| format!("invalid cron '{}': {}", cron_expr, e))?;
    let now_in_tz = tz.from_utc_datetime(&Utc::now().naive_utc());
    schedule
        .after(&now_in_tz)
        .next()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or_else(|| "cron expression produced no future occurrence".into())
}

// Minimal row projection for the fire query.
#[derive(sqlx::FromRow)]
struct ScheduleFireRow {
    id: Uuid,
    public_id: String,
    tenant_id: Uuid,
    organization_id: Uuid,
    event_type_id: Uuid,
    payload: sqlx::types::Json<serde_json::Value>,
    cron_expression: String,
    timezone: String,
    assigned_shard: i16,
    #[allow(dead_code)]
    status: String,
    endpoint_ids: Vec<Uuid>,
    endpoint_public_ids: Vec<String>,
    tiers: Vec<String>,
}
