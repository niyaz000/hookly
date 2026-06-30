use sqlx::{QueryBuilder};
use tracing::debug;

use crate::error::AppError;

use super::models::PlatformEventType;

const SELECT_COLS: &str = "id, public_id, name, description, resource, action, created_at";

#[derive(Clone)]
pub struct PlatformEventTypeRepository {
    pool: crate::common::CountingPool,
}

impl PlatformEventTypeRepository {
    pub fn new(pool: crate::common::CountingPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_public_id(&self, public_id: &str) -> Result<Option<PlatformEventType>, AppError> {
        debug!(public_id = %public_id, "querying platform event type");
        let et = sqlx::query_as::<_, PlatformEventType>(&format!(
            "SELECT {SELECT_COLS} FROM platform_event_types WHERE public_id = $1"
        ))
        .bind(public_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(et)
    }

    pub async fn list(
        &self,
        resource: Option<String>,
        limit: i64,
        cursor: Option<String>,
    ) -> Result<(Vec<PlatformEventType>, Option<String>), AppError> {
        debug!(resource = ?resource, limit = limit, "listing platform event types");
        let cols = SELECT_COLS;
        let mut qb = QueryBuilder::<sqlx::Postgres>::new(format!(
            "SELECT {cols} FROM platform_event_types WHERE 1=1"
        ));
        if let Some(res) = resource {
            qb.push(" AND resource = ").push_bind(res);
        }
        if let Some(ref c) = cursor {
            qb.push(" AND public_id > ").push_bind(c.clone());
        }
        qb.push(" ORDER BY name ASC LIMIT ").push_bind(limit + 1);

        let mut ets: Vec<PlatformEventType> =
            qb.build_query_as::<PlatformEventType>().fetch_all(&self.pool).await?;

        let next_cursor = if ets.len() as i64 > limit {
            ets.pop().map(|e| e.public_id)
        } else {
            None
        };

        Ok((ets, next_cursor))
    }

    /// Used by subscription handlers to validate that event type public_ids exist.
    pub async fn get_public_ids_by_ids(
        &self,
        public_ids: &[String],
    ) -> Result<Vec<String>, AppError> {
        if public_ids.is_empty() {
            return Ok(vec![]);
        }
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT public_id FROM platform_event_types WHERE public_id = ANY($1)",
        )
        .bind(public_ids)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}
