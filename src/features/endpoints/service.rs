use std::collections::HashMap;

use tracing::{info, warn};

use crate::common::{
    crypto::TenantCrypto,
    types::{PaginatedResponse, RequestContext},
};
use crate::error::AppError;
use crate::features::endpoints::models::{
    CreateEndpointRequest, EndpointResponse, HttpConfig, ListQueryParams, RotateSecretRequest,
    SecretResponse, UpdateEndpointRequest,
};
use crate::features::endpoints::repository::EndpointRepository;

pub struct EndpointService {
    repo: EndpointRepository,
    crypto: TenantCrypto,
}

impl EndpointService {
    pub fn new(repo: EndpointRepository, crypto: TenantCrypto) -> Self {
        Self { repo, crypto }
    }

    // --- Validation helpers ---

    fn validate_http_config(config: &serde_json::Value) -> Result<(), AppError> {
        let http: HttpConfig = serde_json::from_value(config.clone())
            .map_err(|e| AppError::BadRequest(format!("invalid http config: {e}")))?;

        if http.url.is_empty() {
            return Err(AppError::BadRequest("config.url is required".into()));
        }
        if http.url.len() > 2048 {
            return Err(AppError::BadRequest("config.url exceeds 2048 chars".into()));
        }
        if !http.url.starts_with("http://") && !http.url.starts_with("https://") {
            return Err(AppError::BadRequest(
                "config.url must start with http:// or https://".into(),
            ));
        }
        let method_upper = http.method.to_uppercase();
        if method_upper != "POST" && method_upper != "PUT" {
            return Err(AppError::BadRequest(
                "config.method must be POST or PUT".into(),
            ));
        }
        let headers = http.headers;
        if headers.len() > 10 {
            return Err(AppError::BadRequest(
                "config.headers: max 10 entries".into(),
            ));
        }
        for (k, v) in &headers {
            if k.len() > 256 {
                return Err(AppError::BadRequest(
                    "config.headers: key exceeds 256 chars".into(),
                ));
            }
            if v.len() > 1024 {
                return Err(AppError::BadRequest(
                    "config.headers: value exceeds 1024 chars".into(),
                ));
            }
        }
        Ok(())
    }

    fn validate_event_types(event_types: &[String]) -> Result<(), AppError> {
        if event_types.len() > 100 {
            return Err(AppError::BadRequest("event_types: max 100 entries".into()));
        }
        for et in event_types {
            if !et.starts_with("evt_") || et.len() != 20 {
                return Err(AppError::BadRequest(format!(
                    "event_types: invalid id '{et}' (expected evt_<16 chars>)"
                )));
            }
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

    fn validate_rate_limit(rl: i32) -> Result<(), AppError> {
        if !(1..=100_000).contains(&rl) {
            return Err(AppError::BadRequest(
                "rate_limit_per_minute must be between 1 and 100000".into(),
            ));
        }
        Ok(())
    }

    // --- Public service methods ---

    #[tracing::instrument(skip(self, req, ctx), fields(application_id = %req.application_id))]
    pub async fn create(
        &self,
        req: CreateEndpointRequest,
        ctx: RequestContext,
    ) -> Result<EndpointResponse, AppError> {
        Self::validate_http_config(&req.config)?;
        Self::validate_event_types(&req.event_types)?;
        Self::validate_tags(&req.tags)?;
        if let Some(rl) = req.rate_limit_per_minute {
            Self::validate_rate_limit(rl)?;
        }

        let app = self
            .repo
            .get_application(&req.application_id)
            .await?
            .ok_or_else(|| {
                warn!("application not found");
                AppError::NotFound(format!("Application not found: {}", req.application_id))
            })?;

        let plaintext = TenantCrypto::generate_webhook_secret();
        let encrypted = self.crypto.encrypt(app.tenant_id, &plaintext)?;

        info!("creating endpoint");
        let ep = self
            .repo
            .create(
                app,
                req.endpoint_type.as_str(),
                req.description.as_deref(),
                &req.config,
                &req.event_types,
                req.rate_limit_per_minute,
                &req.tags,
                &encrypted,
                ctx,
            )
            .await?;

        info!(public_id = %ep.public_id, "endpoint created");
        Ok(EndpointResponse::from(ep))
    }

    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        filter: ListQueryParams,
    ) -> Result<PaginatedResponse<EndpointResponse>, AppError> {
        let page = filter.page;
        let limit = filter.limit;
        let (items, total) = self.repo.list(filter).await?;
        Ok(PaginatedResponse {
            items: items.into_iter().map(EndpointResponse::from).collect(),
            total,
            page: page as i32,
            limit: limit as i32,
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_by_id(&self, public_id: String) -> Result<EndpointResponse, AppError> {
        self.repo
            .get_by_id(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("endpoint not found");
                AppError::NotFound(format!("Endpoint not found: {public_id}"))
            })
            .map(EndpointResponse::from)
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn update(
        &self,
        public_id: String,
        req: UpdateEndpointRequest,
        ctx: RequestContext,
    ) -> Result<EndpointResponse, AppError> {
        if let Some(config) = &req.config {
            // TODO: when more endpoint types exist, fetch current type before validating
            Self::validate_http_config(config)?;
        }
        if let Some(event_types) = &req.event_types {
            Self::validate_event_types(event_types)?;
        }
        if let Some(tags) = &req.tags {
            Self::validate_tags(tags)?;
        }
        if let Some(Some(rl)) = req.rate_limit_per_minute {
            Self::validate_rate_limit(rl)?;
        }

        info!("updating endpoint");
        match self.repo.update(&public_id, req, ctx).await? {
            Some(ep) => Ok(EndpointResponse::from(ep)),
            None => {
                if self.repo.get_by_id(&public_id).await?.is_none() {
                    warn!("endpoint not found");
                    Err(AppError::NotFound(format!(
                        "Endpoint not found: {public_id}"
                    )))
                } else {
                    warn!("endpoint version conflict");
                    Err(AppError::Conflict(
                        "version mismatch — fetch the latest version and retry".into(),
                    ))
                }
            }
        }
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn delete(&self, public_id: String, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting endpoint");
        self.repo.delete(&public_id, ctx).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn pause(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<EndpointResponse, AppError> {
        info!("pausing endpoint");
        self.repo
            .set_status(&public_id, "paused", ctx)
            .await?
            .ok_or_else(|| {
                warn!("endpoint not found for pause");
                AppError::NotFound(format!("Endpoint not found: {public_id}"))
            })
            .map(EndpointResponse::from)
    }

    #[tracing::instrument(skip(self, ctx))]
    pub async fn resume(
        &self,
        public_id: String,
        ctx: RequestContext,
    ) -> Result<EndpointResponse, AppError> {
        info!("resuming endpoint");
        self.repo
            .set_status(&public_id, "active", ctx)
            .await?
            .ok_or_else(|| {
                warn!("endpoint not found for resume");
                AppError::NotFound(format!("Endpoint not found: {public_id}"))
            })
            .map(EndpointResponse::from)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_secret(&self, public_id: String) -> Result<SecretResponse, AppError> {
        info!("fetching endpoint secret");
        let row = self
            .repo
            .get_active_secret(&public_id)
            .await?
            .ok_or_else(|| {
                warn!("endpoint or secret not found");
                AppError::NotFound(format!("Endpoint not found: {public_id}"))
            })?;

        let plaintext = self.crypto.decrypt(row.tenant_id, &row.secret)?;
        Ok(SecretResponse {
            id: row.public_id,
            secret: plaintext,
            created_at: row.created_at,
        })
    }

    #[tracing::instrument(skip(self, req, ctx))]
    pub async fn rotate_secret(
        &self,
        public_id: String,
        req: RotateSecretRequest,
        ctx: RequestContext,
    ) -> Result<SecretResponse, AppError> {
        info!("rotating endpoint secret");
        let meta = self.repo.get_meta(&public_id).await?.ok_or_else(|| {
            warn!("endpoint not found for secret rotation");
            AppError::NotFound(format!("Endpoint not found: {public_id}"))
        })?;

        let expiry = req.expiry_seconds.unwrap_or(0);
        if expiry > 86_400 {
            return Err(AppError::BadRequest(
                "expiry_seconds must be between 0 and 86400".into(),
            ));
        }

        let plaintext = TenantCrypto::generate_webhook_secret();
        let encrypted = self.crypto.encrypt(meta.tenant_id, &plaintext)?;

        let row = self
            .repo
            .rotate_secret(&public_id, &encrypted, expiry, ctx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Endpoint not found: {public_id}")))?;

        info!(public_id = %row.public_id, "endpoint secret rotated");
        Ok(SecretResponse {
            id: row.public_id,
            secret: plaintext,
            created_at: row.created_at,
        })
    }
}
