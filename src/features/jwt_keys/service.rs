use tracing::{info, warn};
use uuid::Uuid;

use crate::common::{types::RequestContext, TenantCrypto};
use crate::error::AppError;

use super::{
    crypto,
    models::{
        CreateJwtKeyRequest, GenerateKeyPairRequest, GenerateKeyPairResponse, JwtKey,
        JwtKeyResponse, JwtKeyStatus, JwksResponse, ListJwtKeysQuery, ListJwtKeysResponse,
        RotateJwtKeyRequest, UpdateJwtKeyRequest,
    },
    repository::JwtKeyRepository,
};

pub struct JwtKeyService {
    repo: JwtKeyRepository,
    crypto: TenantCrypto,
}

impl JwtKeyService {
    pub fn new(repo: JwtKeyRepository, crypto: TenantCrypto) -> Self {
        Self { repo, crypto }
    }

    #[tracing::instrument(skip(self, req, ctx), fields(tenant_id = %req.tenant_id, name = %req.name))]
    pub async fn create(
        &self,
        req: CreateJwtKeyRequest,
        ctx: RequestContext,
    ) -> Result<JwtKeyResponse, AppError> {
        info!("creating jwt key");

        let pair = crypto::generate_key_pair(&req.algorithm)?;

        let private_key_enc = pair
            .private_key_pem
            .as_ref()
            .map(|pem| self.crypto.encrypt(req.tenant_id, pem))
            .transpose()?;

        let secret_enc = pair
            .hmac_secret
            .as_ref()
            .map(|s| self.crypto.encrypt(req.tenant_id, s))
            .transpose()?;

        let key = self
            .repo
            .create(
                req.tenant_id,
                req.application_id,
                req.name,
                req.key_use,
                req.algorithm,
                pair.public_key_pem.clone(),
                private_key_enc,
                secret_enc,
                req.expires_at,
                ctx,
            )
            .await?;

        info!(public_id = %key.public_id, "jwt key created");

        let mut resp = JwtKeyResponse::from_key(key);

        // Return the raw private key / secret once on creation
        if let Some(pem) = pair.private_key_pem {
            resp = resp.with_private_key(pem);
        } else if let Some(secret) = pair.hmac_secret {
            resp = resp.with_private_key(secret);
        }

        Ok(resp)
    }

    #[tracing::instrument(skip(self), fields(public_id = %public_id))]
    pub async fn get_by_id(&self, public_id: &str) -> Result<JwtKey, AppError> {
        self.repo.get_by_public_id(public_id).await?.ok_or_else(|| {
            warn!(public_id = %public_id, "jwt key not found");
            AppError::NotFound(format!("jwt key not found: {public_id}"))
        })
    }

    #[tracing::instrument(skip(self, query))]
    pub async fn list(
        &self,
        query: ListJwtKeysQuery,
    ) -> Result<ListJwtKeysResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);

        let (keys, next_cursor) = self
            .repo
            .list(
                query.tenant_id,
                query.application_id,
                query.key_use,
                query.status,
                limit,
                query.cursor,
            )
            .await?;

        let items: Vec<JwtKeyResponse> =
            keys.into_iter().map(JwtKeyResponse::from_key).collect();

        Ok(ListJwtKeysResponse { items, next_cursor, limit })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(public_id = %public_id))]
    pub async fn update(
        &self,
        public_id: &str,
        req: UpdateJwtKeyRequest,
        ctx: RequestContext,
    ) -> Result<JwtKeyResponse, AppError> {
        info!("updating jwt key");

        self.repo
            .update(public_id, req.name, req.expires_at, ctx)
            .await?
            .map(JwtKeyResponse::from_key)
            .ok_or_else(|| {
                warn!(public_id = %public_id, "jwt key not found for update");
                AppError::NotFound(format!("jwt key not found: {public_id}"))
            })
    }

    #[tracing::instrument(skip(self, req, ctx), fields(public_id = %public_id))]
    pub async fn rotate(
        &self,
        public_id: &str,
        req: RotateJwtKeyRequest,
        ctx: RequestContext,
    ) -> Result<JwtKeyResponse, AppError> {
        info!("rotating jwt key");

        let old_key = self.get_by_id(public_id).await?;
        let grace_hours = req.grace_period_hours.unwrap_or(24).clamp(1, 720);

        let pair = crypto::generate_key_pair(&old_key.algorithm)?;

        let private_key_enc = pair
            .private_key_pem
            .as_ref()
            .map(|pem| self.crypto.encrypt(old_key.tenant_id, pem))
            .transpose()?;

        let secret_enc = pair
            .hmac_secret
            .as_ref()
            .map(|s| self.crypto.encrypt(old_key.tenant_id, s))
            .transpose()?;

        let new_key = self
            .repo
            .rotate(
                public_id,
                old_key.tenant_id,
                old_key.application_id,
                old_key.name,
                old_key.key_use,
                old_key.algorithm,
                pair.public_key_pem.clone(),
                private_key_enc,
                secret_enc,
                old_key.expires_at,
                grace_hours,
                ctx,
            )
            .await?;

        info!(old_id = %public_id, new_id = %new_key.public_id, "jwt key rotated");

        let mut resp = JwtKeyResponse::from_key(new_key);

        if let Some(pem) = pair.private_key_pem {
            resp = resp.with_private_key(pem);
        } else if let Some(secret) = pair.hmac_secret {
            resp = resp.with_private_key(secret);
        }

        Ok(resp)
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn enable(&self, public_id: &str, ctx: RequestContext) -> Result<JwtKeyResponse, AppError> {
        info!("enabling jwt key");

        self.repo
            .set_status(public_id, JwtKeyStatus::Active, ctx)
            .await?
            .map(JwtKeyResponse::from_key)
            .ok_or_else(|| AppError::NotFound(format!("jwt key not found: {public_id}")))
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn disable(&self, public_id: &str, ctx: RequestContext) -> Result<JwtKeyResponse, AppError> {
        info!("disabling jwt key");

        self.repo
            .set_status(public_id, JwtKeyStatus::Disabled, ctx)
            .await?
            .map(JwtKeyResponse::from_key)
            .ok_or_else(|| AppError::NotFound(format!("jwt key not found: {public_id}")))
    }

    #[tracing::instrument(skip(self, ctx), fields(public_id = %public_id))]
    pub async fn delete(&self, public_id: &str, ctx: RequestContext) -> Result<(), AppError> {
        info!("deleting jwt key");

        let deleted = self.repo.soft_delete(public_id, ctx).await?;
        if !deleted {
            return Err(AppError::NotFound(format!("jwt key not found: {public_id}")));
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn get_jwks(&self, tenant_id: Uuid) -> Result<JwksResponse, AppError> {
        let keys = self.repo.list_active_for_jwks(tenant_id).await?;

        let mut jwk_list = Vec::new();
        for key in &keys {
            if let Some(public_key) = &key.public_key {
                if let Some(jwk) =
                    crypto::public_key_to_jwk(&key.algorithm, &key.key_id, public_key)?
                {
                    jwk_list.push(jwk);
                }
            }
        }

        Ok(JwksResponse { keys: jwk_list })
    }

    /// Ephemeral key pair generation — no persistence, no encryption.
    pub fn generate_ephemeral(req: GenerateKeyPairRequest) -> Result<GenerateKeyPairResponse, AppError> {
        let pair = crypto::generate_key_pair(&req.algorithm)?;

        Ok(GenerateKeyPairResponse {
            algorithm: req.algorithm,
            public_key: pair.public_key_pem,
            private_key: pair.private_key_pem,
            secret: pair.hmac_secret,
        })
    }

    pub async fn expire_grace_period_keys(&self) -> Result<u64, AppError> {
        self.repo.expire_grace_period_keys().await
    }
}
