use std::collections::HashMap;

use serde::Serialize;
use sqlx::{types::Json};
use tracing::{info, warn};
use uuid::Uuid;

use crate::common::{types::RequestContext, NanoId};
use crate::error::AppError;
use crate::features::{
    api_keys::{
        crypto,
        models::{ApiKey, ApiKeyResponse},
    },
    environments::models::{Environment, EnvironmentResponse},
    organizations::models::{CreateOrganizationRequest, Organization, OrganizationResponse},
    permissions::repository::PermissionRepository,
    roles::repository::RoleRepository,
    tenants::models::{Tenant, TenantResponse},
    users::models::{User, UserResponse},
};

#[derive(Debug, Serialize)]
pub struct BootstrapResponse {
    pub organization: OrganizationResponse,
    pub tenant: TenantResponse,
    pub environment: EnvironmentResponse,
    pub user: UserResponse,
    pub api_key: ApiKeyResponse,
}

pub struct BootstrapService {
    pool: crate::common::CountingPool,
    role_repo: RoleRepository,
    perm_repo: PermissionRepository,
}

impl BootstrapService {
    pub fn new(pool: crate::common::CountingPool, role_repo: RoleRepository, perm_repo: PermissionRepository) -> Self {
        Self { pool, role_repo, perm_repo }
    }

    /// Create an org and its required bootstrap resources in a single transaction:
    /// default tenant → production environment → owner user → default API key.
    /// Role seeding runs after the commit (best-effort, same as normal tenant creation).
    #[tracing::instrument(skip(self, req, ctx), fields(slug = %req.slug))]
    pub async fn bootstrap_organization(
        &self,
        req: CreateOrganizationRequest,
        ctx: RequestContext,
    ) -> Result<BootstrapResponse, AppError> {
        req.validate()?;

        let mut tx = self.pool.begin().await?;

        // Pre-generate the owner user ID so it can be used as created_by for all resources.
        let user_id = Uuid::now_v7();
        let user_public_id = format!("usr_{}", NanoId::generate(20));

        // ── 1. Organization ───────────────────────────────────────────────────
        let org_id = Uuid::now_v7();
        let org_public_id = format!("org_{}", NanoId::generate(20));

        let org = sqlx::query_as::<_, Organization>(
            r#"
            INSERT INTO organizations (
                id, public_id, name, slug,
                owner_email, external_id, tags,
                created_by, updated_by, request_id, version,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6, $7,
                $8, $8, $9, 0,
                NOW(), NOW()
            )
            RETURNING *
            "#,
        )
        .bind(org_id)
        .bind(&org_public_id)
        .bind(req.name.trim())
        .bind(req.slug.to_lowercase().trim())
        .bind(&req.owner_email)
        .bind(req.external_id.as_deref())
        .bind(Json(req.tags.clone().unwrap_or_default()))
        .bind(user_id)
        .bind(ctx.request_id)
        .fetch_one(&mut *tx)
        .await?;

        // ── 2. Default tenant ─────────────────────────────────────────────────
        let tenant_id = Uuid::now_v7();
        let tenant_public_id = format!("ten_{}", NanoId::generate(20));
        let tenant_name = format!("{} Default", req.name.trim());
        let tenant_description = "Default tenant created on organization signup.";
        let empty_map = Json(HashMap::<String, String>::new());

        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            WITH ins AS (
                INSERT INTO tenants (
                    id, public_id, organization_id, name, description,
                    tags, metadata, settings,
                    created_by, updated_by, request_id, version,
                    created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5,
                    $6, $7, $8,
                    $9, $9, $10, 0,
                    NOW(), NOW()
                )
                RETURNING *
            )
            SELECT ins.*, o.public_id AS organization_public_id
            FROM ins JOIN organizations o ON o.id = ins.organization_id
            "#,
        )
        .bind(tenant_id)
        .bind(&tenant_public_id)
        .bind(org_id)
        .bind(&tenant_name)
        .bind(tenant_description)
        .bind(&empty_map)
        .bind(&empty_map)
        .bind(&empty_map)
        .bind(user_id)
        .bind(ctx.request_id)
        .fetch_one(&mut *tx)
        .await?;

        // ── 3. Default environment (production) ───────────────────────────────
        let env_public_id = format!("env_{}", NanoId::new());

        let env = sqlx::query_as::<_, Environment>(
            r#"
            INSERT INTO environments (
                id, public_id, tenant_id, name, description, tags,
                request_id, version, created_by, updated_by, created_at, updated_at
            ) VALUES (
                gen_random_uuid(), $1, $2, 'production', NULL, $3,
                $4, 0, $5, $5, NOW(), NOW()
            )
            RETURNING id, public_id, tenant_id, name, description, status, tags,
                      version, created_by, updated_by, created_at, updated_at
            "#,
        )
        .bind(&env_public_id)
        .bind(tenant_id)
        .bind(Json(HashMap::<String, String>::new()))
        .bind(ctx.request_id)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

        // ── 4. Owner user ─────────────────────────────────────────────────────
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO identity.users (
                id, public_id, organization_id, tenant_id, email, phone,
                metadata, tags, settings, password_hash,
                created_by, updated_by, request_id,
                version, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, NULL,
                $6, $7, $8, NULL,
                $9, $9, $10,
                1, NOW(), NOW()
            )
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(&user_public_id)
        .bind(org_id)
        .bind(tenant_id)
        .bind(&req.owner_email)
        .bind(&empty_map)
        .bind(&empty_map)
        .bind(&empty_map)
        .bind(user_id)
        .bind(ctx.request_id)
        .fetch_one(&mut *tx)
        .await?;

        // ── 5. Default API key ────────────────────────────────────────────────
        let (full_key, key_prefix) = crypto::generate_api_key("production", 32);
        let key_hash = crypto::hash_key(&full_key);
        let api_key_public_id = format!("key_{}", NanoId::new());

        let api_key = sqlx::query_as::<_, ApiKey>(
            r#"
            INSERT INTO api_keys (
                id, public_id, organization_id, tenant_id, user_id,
                name, key_hash, key_prefix, environment_id,
                request_id, version, created_by, updated_by, created_at, updated_at
            ) VALUES (
                gen_random_uuid(), $1, $2, $3, $4,
                'default', $5, $6, $7,
                $8, 0, $9, $9, NOW(), NOW()
            )
            RETURNING id, public_id, organization_id, tenant_id, user_id,
                      name, description, key_hash, key_encrypted, key_prefix,
                      environment_id, status, expires_at, last_used_at,
                      version, created_by, updated_by, created_at, updated_at, deleted_at
            "#,
        )
        .bind(&api_key_public_id)
        .bind(org_id)
        .bind(tenant_id)
        .bind(user_id)
        .bind(&key_hash)
        .bind(&key_prefix)
        .bind(&env_public_id)
        .bind(ctx.request_id)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        info!(
            org = %org.public_id,
            tenant = %tenant.public_id,
            env = %env.public_id,
            user = %user.public_id,
            api_key = %api_key.public_id,
            "organization bootstrapped"
        );

        // ── Phase 2: best-effort role seeding (same pattern as TenantService) ─
        let seed_result = async {
            let system_perms = self.perm_repo.list_system().await?;
            self.role_repo.seed_default_roles(tenant_id, &system_perms, ctx).await
        }
        .await;
        if let Err(e) = seed_result {
            warn!(tenant_id = %tenant_id, error = ?e, "failed to seed default roles during bootstrap");
        }

        // The INSERT RETURNING values above don't include JOINed public IDs.
        // We already have them as local variables, so override them directly.
        let mut api_key_response = ApiKeyResponse::from(api_key);
        api_key_response.key = Some(full_key);
        api_key_response.organization_id = org_public_id.clone();
        api_key_response.tenant_id = tenant_public_id.clone();
        api_key_response.user_id = user_public_id.clone();
        api_key_response.created_by = user_public_id.clone();
        api_key_response.updated_by = user_public_id.clone();

        let mut env_response = EnvironmentResponse::from(env);
        env_response.organization_id = org_public_id.clone();
        env_response.tenant_id = tenant_public_id.clone();
        env_response.created_by = user_public_id.clone();
        env_response.updated_by = user_public_id.clone();

        let mut user_response = UserResponse::from(user);
        user_response.organization_id = org_public_id.clone();
        user_response.tenant_id = tenant_public_id.clone();
        user_response.created_by = user_public_id.clone();
        user_response.updated_by = user_public_id.clone();

        let mut org_response = OrganizationResponse::from(org);
        org_response.created_by = user_public_id.clone();
        org_response.updated_by = user_public_id.clone();

        let mut tenant_response = TenantResponse::from(tenant);
        tenant_response.created_by = user_public_id.clone();
        tenant_response.updated_by = user_public_id.clone();

        Ok(BootstrapResponse {
            organization: org_response,
            tenant: tenant_response,
            environment: env_response,
            user: user_response,
            api_key: api_key_response,
        })
    }
}
