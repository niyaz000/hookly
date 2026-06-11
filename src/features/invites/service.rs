use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{Duration, Utc};
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use uuid::Uuid;

use crate::email::EmailService;
use crate::error::AppError;

use super::models::{
    AcceptInviteRequest, CreateInviteRequest, InviteResponse, InviteVerifyResponse,
    ListInvitesQuery, ListInvitesResponse, TenantMemberResponse, VerifyInviteRequest,
};
use super::repository::InviteRepository;

pub struct InviteService {
    repo: InviteRepository,
}

impl InviteService {
    pub fn new(repo: InviteRepository) -> Self {
        Self { repo }
    }

    fn generate_token() -> String {
        format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
    }

    fn hash_token(token: &str) -> String {
        let hash = Sha256::digest(token.as_bytes());
        hash.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn encode_cursor(id: Uuid) -> String {
        URL_SAFE_NO_PAD.encode(id.as_bytes())
    }

    fn decode_cursor(s: &str) -> Result<Uuid, AppError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|_| AppError::BadRequest("invalid cursor".into()))?;
        Uuid::from_slice(&bytes).map_err(|_| AppError::BadRequest("invalid cursor".into()))
    }

    #[tracing::instrument(skip(self, req, email), fields(email = %req.user_email))]
    pub async fn create(
        &self,
        req: CreateInviteRequest,
        request_id: Uuid,
        email: &dyn EmailService,
    ) -> Result<InviteResponse, AppError> {
        req.validate()?;
        info!("creating invite");

        let expires_at = req
            .expires_at
            .unwrap_or_else(|| Utc::now() + Duration::days(7));

        let token = Self::generate_token();
        let token_hash = Self::hash_token(&token);

        let tags = req.tags.clone().unwrap_or_else(|| serde_json::json!({}));
        let metadata = req.metadata.clone().unwrap_or_else(|| serde_json::json!({}));

        let row = self
            .repo
            .create(
                req.tenant_id,
                req.organization_id,
                req.user_email.trim(),
                req.role.trim(),
                &token_hash,
                &tags,
                &metadata,
                expires_at,
                req.created_by,
                request_id,
            )
            .await;

        let row = match row {
            Ok(r) => r,
            Err(e) => {
                warn!("invite insert failed: {e:?}");
                return Err(e);
            }
        };

        if let Err(e) = email
            .send_invite(&row.user_email, &token, &row.role, row.expires_at)
            .await
        {
            warn!("email delivery failed, marking invite as failed: {e:?}");
            let _ = self
                .repo
                .set_status(row.id, "failed", None, None)
                .await;
        }

        info!(public_id = %row.public_id, "invite created");
        Ok(InviteResponse::from_row(row).with_token(token))
    }

    #[tracing::instrument(skip(self))]
    pub async fn get(&self, public_id: String) -> Result<InviteResponse, AppError> {
        info!("fetching invite");
        self.repo
            .get_by_public_id(&public_id)
            .await?
            .map(InviteResponse::from_row)
            .ok_or_else(|| {
                warn!("invite not found");
                AppError::NotFound(format!("Invite not found: {public_id}"))
            })
    }

    #[tracing::instrument(skip(self, query))]
    pub async fn list(&self, query: ListInvitesQuery) -> Result<ListInvitesResponse, AppError> {
        let limit = query.limit.unwrap_or(20).clamp(1, 100);
        let cursor = query.cursor.as_deref().map(Self::decode_cursor).transpose()?;

        let (rows, next_id) = self
            .repo
            .list(
                limit,
                cursor,
                query.tenant_id,
                query.organization_id,
                query.status.as_deref(),
                query.user_email.as_deref(),
            )
            .await?;

        let next_cursor = next_id.map(Self::encode_cursor);
        let items = rows.into_iter().map(InviteResponse::from_row).collect();

        Ok(ListInvitesResponse { items, next_cursor, limit })
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, public_id: String) -> Result<(), AppError> {
        info!("deleting invite");
        let deleted = self.repo.delete(&public_id).await?;
        if !deleted {
            warn!("invite not found for delete");
            return Err(AppError::NotFound(format!("Invite not found: {public_id}")));
        }
        info!("invite deleted");
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn revoke(&self, public_id: String) -> Result<InviteResponse, AppError> {
        info!("revoking invite");
        let row = self
            .repo
            .get_by_public_id(&public_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Invite not found: {public_id}")))?;

        if !matches!(row.status.as_str(), "sent" | "opened" | "failed") {
            return Err(AppError::BadRequest(format!(
                "cannot revoke an invite with status '{}'",
                row.status
            )));
        }

        let updated = self
            .repo
            .set_status(row.id, "revoked", Some(Utc::now()), None)
            .await?
            .ok_or_else(|| AppError::Internal("invite revoke failed".into()))?;

        info!(public_id = %public_id, "invite revoked");
        Ok(InviteResponse::from_row(updated))
    }

    #[tracing::instrument(skip(self, email))]
    pub async fn resend(
        &self,
        public_id: String,
        request_id: Uuid,
        email: &dyn EmailService,
    ) -> Result<InviteResponse, AppError> {
        info!("resending invite");

        let token = Self::generate_token();
        let token_hash = Self::hash_token(&token);
        let new_expires_at = Utc::now() + Duration::days(7);

        let row = self
            .repo
            .resend(&public_id, &token_hash, new_expires_at, Uuid::nil(), request_id)
            .await?
            .ok_or_else(|| {
                warn!("invite not found or not resendable");
                AppError::NotFound(format!(
                    "Invite not found or cannot be resent: {public_id}"
                ))
            })?;

        if let Err(e) = email
            .send_invite(&row.user_email, &token, &row.role, row.expires_at)
            .await
        {
            warn!("email delivery failed on resend: {e:?}");
            let _ = self.repo.set_status(row.id, "failed", None, None).await;
        }

        info!(public_id = %row.public_id, "invite resent");
        Ok(InviteResponse::from_row(row).with_token(token))
    }

    #[tracing::instrument(skip(self, req))]
    pub async fn verify(&self, req: VerifyInviteRequest) -> Result<InviteVerifyResponse, AppError> {
        info!("verifying invite token");

        if req.token.trim().is_empty() {
            return Err(AppError::BadRequest("token cannot be empty".into()));
        }

        let token_hash = Self::hash_token(&req.token);
        let row = self
            .repo
            .get_by_token_hash(&token_hash)
            .await?
            .ok_or_else(|| AppError::NotFound("invite not found".into()))?;

        if row.expires_at <= Utc::now() {
            return Err(AppError::BadRequest("invite has expired".into()));
        }
        if matches!(row.status.as_str(), "revoked" | "accepted") {
            return Err(AppError::BadRequest(format!(
                "invite is {}",
                row.status
            )));
        }

        if row.status != "opened" {
            let updated = self
                .repo
                .set_status(row.id, "opened", None, None)
                .await?
                .ok_or_else(|| AppError::Internal("failed to update invite status".into()))?;
            return Ok(InviteVerifyResponse::from(updated));
        }

        Ok(InviteVerifyResponse::from(row))
    }

    #[tracing::instrument(skip(self, req))]
    pub async fn accept(&self, req: AcceptInviteRequest) -> Result<TenantMemberResponse, AppError> {
        info!("accepting invite");

        if req.token.trim().is_empty() {
            return Err(AppError::BadRequest("token cannot be empty".into()));
        }

        let token_hash = Self::hash_token(&req.token);
        let row = self
            .repo
            .get_by_token_hash(&token_hash)
            .await?
            .ok_or_else(|| AppError::NotFound("invite not found".into()))?;

        if row.expires_at <= Utc::now() {
            return Err(AppError::BadRequest("invite has expired".into()));
        }
        if row.status == "accepted" {
            return Err(AppError::Conflict("invite has already been accepted".into(), vec![]));
        }
        if matches!(row.status.as_str(), "revoked") {
            return Err(AppError::BadRequest("invite has been revoked".into()));
        }

        let updated = self
            .repo
            .set_status(row.id, "accepted", None, Some(Utc::now()))
            .await?
            .ok_or_else(|| AppError::Internal("failed to accept invite".into()))?;

        let member = self
            .repo
            .create_member(
                updated.id,
                &updated.public_id,
                updated.tenant_id,
                updated.organization_id,
                &updated.user_email,
                &updated.role,
            )
            .await?;

        info!(public_id = %updated.public_id, "invite accepted, member created");
        Ok(TenantMemberResponse::from(member))
    }
}
