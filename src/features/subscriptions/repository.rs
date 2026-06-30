use chrono::{DateTime, Utc};

use uuid::Uuid;

use crate::common::nano_id::NanoId;
use crate::error::AppError;

use super::models::{ListQueryParams, SubscriptionRow};

const BASE_SELECT: &str = r#"
    SELECT
        s.id, s.public_id,
        s.endpoint_id,    ep.public_id AS endpoint_public_id,
        s.event_type_id,  et.public_id AS event_type_public_id, et.name AS event_type_name,
        s.application_id, a.public_id  AS application_public_id,
        s.tenant_id, s.organization_id,
        s.status, s.created_at
    FROM subscriptions s
    JOIN endpoints   ep ON ep.id = s.endpoint_id
    JOIN event_types et ON et.id = s.event_type_id
    JOIN applications a ON a.id  = s.application_id
"#;

#[derive(sqlx::FromRow)]
pub struct ApplicationRef {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub organization_id: Uuid,
}

#[derive(sqlx::FromRow)]
pub struct EndpointRef {
    pub id: Uuid,
}

#[derive(sqlx::FromRow)]
pub struct EventTypeRef {
    pub id: Uuid,
}

pub struct SubscriptionRepository {
    db: crate::common::CountingPool,
}

impl SubscriptionRepository {
    pub fn new(db: crate::common::CountingPool) -> Self {
        Self { db }
    }

    pub async fn get_application(&self, public_id: &str) -> Result<Option<ApplicationRef>, AppError> {
        sqlx::query_as::<_, ApplicationRef>(
            "SELECT id, tenant_id, organization_id FROM applications \
             WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    pub async fn get_endpoint_for_app(
        &self,
        public_id: &str,
        application_id: Uuid,
    ) -> Result<Option<EndpointRef>, AppError> {
        sqlx::query_as::<_, EndpointRef>(
            "SELECT id FROM endpoints \
             WHERE public_id = $1 AND application_id = $2 \
               AND status = 'active' AND deleted_at IS NULL",
        )
        .bind(public_id)
        .bind(application_id)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    pub async fn get_event_type_for_tenant(
        &self,
        public_id: &str,
        tenant_id: Uuid,
    ) -> Result<Option<EventTypeRef>, AppError> {
        sqlx::query_as::<_, EventTypeRef>(
            "SELECT id FROM event_types \
             WHERE public_id = $1 AND tenant_id = $2 \
               AND archived = FALSE AND deleted_at IS NULL",
        )
        .bind(public_id)
        .bind(tenant_id)
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::from)
    }

    pub async fn create(
        &self,
        app: ApplicationRef,
        endpoint_id: Uuid,
        event_type_id: Uuid,
    ) -> Result<SubscriptionRow, AppError> {
        let public_id = format!("sub_{}", NanoId::new());

        sqlx::query_scalar::<_, Uuid>(
            r#"INSERT INTO subscriptions
               (public_id, endpoint_id, event_type_id, application_id, tenant_id, organization_id)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id"#,
        )
        .bind(&public_id)
        .bind(endpoint_id)
        .bind(event_type_id)
        .bind(app.id)
        .bind(app.tenant_id)
        .bind(app.organization_id)
        .fetch_one(&self.db)
        .await?;

        self.get_by_id(&public_id).await?.ok_or_else(|| {
            AppError::Internal("subscription created but not found on fetch".into())
        })
    }

    pub async fn get_by_id(&self, public_id: &str) -> Result<Option<SubscriptionRow>, AppError> {
        let sql = format!("{} WHERE s.public_id = $1 AND s.deleted_at IS NULL", BASE_SELECT);
        sqlx::query_as::<_, SubscriptionRow>(&sql)
            .bind(public_id)
            .fetch_optional(&self.db)
            .await
            .map_err(AppError::from)
    }

    pub async fn list(&self, filter: ListQueryParams) -> Result<(Vec<SubscriptionRow>, i64), AppError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: Uuid,
            public_id: String,
            endpoint_id: Uuid,
            endpoint_public_id: String,
            event_type_id: Uuid,
            event_type_public_id: String,
            event_type_name: String,
            application_id: Uuid,
            application_public_id: String,
            tenant_id: Uuid,
            organization_id: Uuid,
            status: String,
            created_at: DateTime<Utc>,
            total_count: i64,
        }

        let limit = filter.limit.min(100) as i64;
        let offset = (filter.page.saturating_sub(1)) as i64 * limit;

        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT
                s.id, s.public_id,
                s.endpoint_id,    ep.public_id AS endpoint_public_id,
                s.event_type_id,  et.public_id AS event_type_public_id, et.name AS event_type_name,
                s.application_id, a.public_id  AS application_public_id,
                s.tenant_id, s.organization_id,
                s.status, s.created_at,
                COUNT(*) OVER() AS total_count
               FROM subscriptions s
               JOIN endpoints   ep ON ep.id = s.endpoint_id
               JOIN event_types et ON et.id = s.event_type_id
               JOIN applications a ON a.id  = s.application_id
               WHERE s.application_id = (
                   SELECT id FROM applications WHERE public_id = $1 AND deleted_at IS NULL
               )
                 AND s.deleted_at IS NULL
                 AND ($2::text IS NULL OR ep.public_id = $2)
                 AND ($3::text IS NULL OR et.public_id = $3)
               ORDER BY s.created_at DESC
               LIMIT $4 OFFSET $5"#,
        )
        .bind(&filter.application_id)
        .bind(&filter.endpoint_id)
        .bind(&filter.event_type_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let total = rows.first().map(|r| r.total_count).unwrap_or(0);
        let items = rows
            .into_iter()
            .map(|r| SubscriptionRow {
                id: r.id,
                public_id: r.public_id,
                endpoint_id: r.endpoint_id,
                endpoint_public_id: r.endpoint_public_id,
                event_type_id: r.event_type_id,
                event_type_public_id: r.event_type_public_id,
                event_type_name: r.event_type_name,
                application_id: r.application_id,
                application_public_id: r.application_public_id,
                tenant_id: r.tenant_id,
                organization_id: r.organization_id,
                status: r.status,
                created_at: r.created_at,
            })
            .collect();

        Ok((items, total))
    }

    pub async fn delete(&self, public_id: &str) -> Result<bool, AppError> {
        let result = sqlx::query(
            "UPDATE subscriptions SET deleted_at = NOW() \
             WHERE public_id = $1 AND deleted_at IS NULL",
        )
        .bind(public_id)
        .execute(&self.db)
        .await
        .map_err(AppError::from)?;
        Ok(result.rows_affected() > 0)
    }

    /// Returns endpoint UUIDs for all active subscriptions matching this
    /// (event_type, application) pair. Used by the event service to fan out delivery.
    pub async fn get_active_for_event_type(
        &self,
        event_type_id: Uuid,
        application_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT endpoint_id FROM subscriptions \
             WHERE event_type_id = $1 AND application_id = $2 \
               AND status = 'active' AND deleted_at IS NULL",
        )
        .bind(event_type_id)
        .bind(application_id)
        .fetch_all(&self.db)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
