
use tracing::debug;
use uuid::Uuid;

use crate::error::AppError;

use super::models::PlatformSubscription;

const SELECT_COLS: &str = "tenant_id, event_type_public_id, created_at";

#[derive(Clone)]
pub struct PlatformSubscriptionRepository {
    pool: crate::common::CountingPool,
}

impl PlatformSubscriptionRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
        Self { pool }
    }

    pub async fn list_for_tenant(&self, tenant_id: Uuid) -> Result<Vec<PlatformSubscription>, AppError> {
        debug!(tenant_id = %tenant_id, "listing platform subscriptions");
        let subs = sqlx::query_as::<_, PlatformSubscription>(&format!(
            "SELECT {SELECT_COLS} FROM platform_webhook_subscriptions
             WHERE tenant_id = $1 ORDER BY event_type_public_id ASC"
        ))
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(subs)
    }

    /// Inserts the given event_type_public_ids for the tenant; skips duplicates.
    /// Returns (inserted, already_present).
    pub async fn subscribe(
        &self,
        tenant_id: Uuid,
        event_type_public_ids: &[String],
    ) -> Result<(usize, usize), AppError> {
        if event_type_public_ids.is_empty() {
            return Ok((0, 0));
        }

        debug!(
            tenant_id = %tenant_id,
            count = event_type_public_ids.len(),
            "subscribing to platform event types"
        );

        // Count how many already exist
        let existing: Vec<(String,)> = sqlx::query_as(
            "SELECT event_type_public_id FROM platform_webhook_subscriptions
             WHERE tenant_id = $1 AND event_type_public_id = ANY($2)",
        )
        .bind(tenant_id)
        .bind(event_type_public_ids)
        .fetch_all(&self.pool)
        .await?;

        let already_present = existing.len();

        let result = sqlx::query(
            "INSERT INTO platform_webhook_subscriptions (tenant_id, event_type_public_id)
             SELECT $1, unnest($2::varchar[])
             ON CONFLICT DO NOTHING",
        )
        .bind(tenant_id)
        .bind(event_type_public_ids)
        .execute(&self.pool)
        .await?;

        let inserted = result.rows_affected() as usize;
        Ok((inserted, already_present))
    }

    /// Deletes one subscription for the tenant.
    pub async fn unsubscribe(
        &self,
        tenant_id: Uuid,
        event_type_public_id: &str,
    ) -> Result<bool, AppError> {
        debug!(tenant_id = %tenant_id, event_type = %event_type_public_id, "unsubscribing");
        let result = sqlx::query(
            "DELETE FROM platform_webhook_subscriptions
             WHERE tenant_id = $1 AND event_type_public_id = $2",
        )
        .bind(tenant_id)
        .bind(event_type_public_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Replaces the tenant's subscriptions atomically.
    /// Returns (inserted, removed).
    pub async fn replace(
        &self,
        tenant_id: Uuid,
        event_type_public_ids: &[String],
    ) -> Result<(usize, usize), AppError> {
        debug!(
            tenant_id = %tenant_id,
            count = event_type_public_ids.len(),
            "replacing platform subscriptions"
        );

        let mut tx = self.pool.begin().await?;

        // Delete all existing subscriptions for the tenant
        let deleted = sqlx::query(
            "DELETE FROM platform_webhook_subscriptions WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;

        // Insert the new set
        let inserted = if event_type_public_ids.is_empty() {
            0
        } else {
            let result = sqlx::query(
                "INSERT INTO platform_webhook_subscriptions (tenant_id, event_type_public_id)
                 SELECT $1, unnest($2::varchar[])
                 ON CONFLICT DO NOTHING",
            )
            .bind(tenant_id)
            .bind(event_type_public_ids)
            .execute(&mut *tx)
            .await?;
            result.rows_affected() as usize
        };

        tx.commit().await?;

        Ok((inserted, deleted.rows_affected() as usize))
    }
}
