# Data model

## Conventions

Every table follows a consistent set of conventions:

| Convention | Detail |
|---|---|
| Primary key | `id UUID` generated via `Uuid::now_v7()` — time-ordered, index-friendly |
| API identifier | `public_id VARCHAR` — prefixed NanoId (e.g. `app_`, `evn_`, `dj_`) shown in all API responses |
| Soft delete | `deleted_at TIMESTAMPTZ` — rows are never physically deleted; partial indexes exclude soft-deleted rows |
| Audit fields | `created_by UUID`, `updated_by UUID`, `request_id UUID` on every mutable table |
| Optimistic lock | `version INTEGER` incremented on every update; used to detect concurrent modifications |
| No FK constraints | Referential integrity is enforced at the application layer. See [ADR: No FK constraints](../decisions/database/001-no-fk-constraints.md). |

---

## Domain map

The schema is split into seven domains. Every table carries `tenant_id`; rows from different tenants never join.

```
┌──────────────────────────────────────────────────────────────────────────┐
│  multi-tenancy                                                           │
│  organizations ──► tenants ──► applications                             │
│                       └──► environments                                  │
└─────────────────────────────┬────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼──────────────────────┐
        ▼                     ▼                       ▼
┌─────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
│ identity & teams    │ │ event delivery       │ │ scheduling           │
│ ──────────────────  │ │ ────────────────────  │ │ ────────────────────  │
│ users               │ │ event_types          │ │ schedules            │
│ teams               │ │ endpoints            │ │ schedule_endpoints   │
│ team_members        │ │ endpoint_secrets     │ │ schedule_executions  │
│ invites             │ │ events               │ │ scheduler_shards     │
│ tenant_members      │ │ delivery_jobs        │ │ tenant_shard_affinity│
└─────────────────────┘ │ delivery_attempts    │ └──────────────────────┘
                        └──────────────────────┘
┌─────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
│ rbac                │ │ credentials          │ │ platform webhooks    │
│ ──────────────────  │ │ ────────────────────  │ │ ────────────────────  │
│ permissions         │ │ api_keys             │ │ platform_event_types │
│ roles               │ │ api_key_settings     │ │ platform_webhooks    │
│ role_permissions    │ │ jwt_keys             │ │ platform_webhook_    │
│ user_roles          │ └──────────────────────┘ │   subscriptions      │
│ user_permissions    │                          └──────────────────────┘
│ api_key_roles       │
│ api_key_permissions │
└─────────────────────┘
```

---

## Multi-tenancy

### `organizations`

Top-level billing account. An organization contains one or more tenants and holds billing metadata.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK, UUIDv7 |
| `public_id` | VARCHAR(24) | API-facing identifier |
| `name` | VARCHAR(255) | Display name, unique |
| `slug` | VARCHAR(64) | URL-safe unique slug; format: `[a-z0-9-]+` |
| `status` | ENUM | `active`, `suspended`, `inactive` |
| `billing_email` | VARCHAR(64) | Contact for invoicing |
| `plan` | VARCHAR(32) | Subscription plan name, default `free` |
| `stripe_customer_id` | VARCHAR(32) | Optional Stripe reference |
| `tier` | VARCHAR(32) | Delivery tier used for queue routing |

### `tenants`

The primary unit of data isolation. Every other table is scoped to a tenant. A tenant lives inside one organization.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK, UUIDv7 |
| `public_id` | VARCHAR(24) | API-facing identifier |
| `organization_id` | UUID | Parent organization |
| `name` | VARCHAR(255) | Unique across the platform |
| `status` | ENUM | `active`, `suspended`, `inactive` |
| `settings` | JSONB | Tenant-level feature flags and configuration |

### `applications`

A logical namespace within a tenant. Events are published to an application; endpoints are registered to one. Useful for separating concerns (e.g. a "payments" app vs. a "notifications" app within the same tenant).

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | Prefix: `app_` |
| `tenant_id` | UUID | Owning tenant |
| `organization_id` | UUID | Owning organization |
| `name` | VARCHAR(64) | Unique per tenant |
| `description` | VARCHAR(255) | |
| `status` | ENUM | `active`, `suspended`, `disabled` |

### `environments`

Named runtime scopes within a tenant (e.g. `live`, `sandbox`, `staging`). API keys are issued per environment so teams can maintain completely separate credential sets without needing separate tenants.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | |
| `tenant_id` | UUID | Owning tenant |
| `name` | VARCHAR(64) | Unique per tenant; format `[a-z][a-z0-9_-]{2,63}` |
| `status` | ENUM | `active`, `disabled` |

---

## Identity & Teams

### `users`

A human who authenticates to the platform. Stored in the `identity` schema to keep auth concerns separate from the business domain. A user belongs to one organization and can be a member of multiple tenants via `tenant_members`.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK, UUIDv7 |
| `public_id` | VARCHAR(24) | |
| `organization_id` | UUID | |
| `tenant_id` | UUID | Primary tenant |
| `email` | VARCHAR(64) | Globally unique |
| `status` | ENUM | `active`, `suspended`, `inactive`, `locked` |
| `password_hash` | VARCHAR(255) | Argon2 hash; nullable for SSO-only users |
| `locked_until` | TIMESTAMPTZ | Set on too many failed logins |
| `login_count` | INTEGER | Monotonic counter |
| `email_verified_at` | TIMESTAMPTZ | Null until email is confirmed |

### `teams`

A named group of users within a tenant. Used for role and permission assignments that apply to all members.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `name` | VARCHAR(255) | |
| `tenant_id` | UUID | |
| `organization_id` | UUID | |

### `team_members`

Join table associating a user with a team. Soft-deleted when a user is removed from a team.

| Column | Type | Notes |
|---|---|---|
| `team_id` | UUID | References `teams.id` |
| `user_id` | UUID | References `users.id` |
| Unique | | `(team_id, user_id)` |

### `invites`

A time-limited, single-use invitation to join a tenant. The token is stored as a hash; the raw token is delivered by email and used once to accept the invite.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `tenant_id` | UUID | |
| `organization_id` | UUID | |
| `user_email` | VARCHAR(255) | Recipient |
| `role` | VARCHAR(50) | Role to assign on acceptance |
| `status` | VARCHAR(20) | `sent`, `opened`, `accepted`, `expired`, `revoked`, `failed` |
| `token_hash` | TEXT | SHA-256 of the raw invite token; unique |
| `expires_at` | TIMESTAMPTZ | Invite becomes invalid after this |
| `accepted_at` | TIMESTAMPTZ | Set when the user accepts |
| `revoked_at` | TIMESTAMPTZ | Set when an operator revokes |

### `tenant_members`

Created when an invite is accepted. Records the active membership of a user in a tenant. One user can be a member of multiple tenants.

| Column | Type | Notes |
|---|---|---|
| `tenant_id` | UUID | |
| `user_id` | UUID | Nullable until the user creates an account |
| `user_email` | VARCHAR(255) | Always set; used to match before account creation |
| `invite_id` | UUID | The accepted invite; unique |
| `role` | VARCHAR(50) | Role at the time of acceptance |
| `status` | VARCHAR(20) | `active`, `disabled` |
| Unique | | `(tenant_id, user_email) WHERE deleted_at IS NULL` |

---

## Event Delivery

### `event_types`

Customer-defined event schemas within a tenant. Describes the shape of events that can be published. Contains a `event_schema` JSONB field for future payload validation.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | Prefix: `evt_` |
| `tenant_id` | UUID | |
| `organization_id` | UUID | |
| `name` | VARCHAR(255) | Unique per `(tenant_id, name, schema_version)` |
| `schema_version` | VARCHAR(50) | Default `1.0` |
| `event_schema` | JSONB | JSON Schema for payload validation |
| `archived` | BOOLEAN | Archived types cannot receive new events |
| `tags` | JSONB | Free-form key/value labels |

### `endpoints`

An HTTP URL registered by the tenant to receive webhook deliveries. An endpoint subscribes to one or more event types and carries its own rate limit and signing secrets.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | Prefix: `ep_` |
| `application_id` | UUID | Owning application |
| `tenant_id` | UUID | |
| `organization_id` | UUID | |
| `endpoint_type` | VARCHAR(50) | Currently always `http` |
| `config` | JSONB | URL and HTTP-specific settings |
| `event_types` | TEXT[] | Array of event type names this endpoint receives |
| `status` | VARCHAR(20) | `active`, `paused` |
| `rate_limit_per_minute` | INTEGER | Nullable; 1–100000 |

GIN index on `event_types` supports efficient routing: "find all endpoints subscribed to event type X."

### `endpoint_secrets`

HMAC-SHA256 signing secrets for an endpoint, stored AES-256-GCM encrypted. Supports rotation grace periods — two secrets can be `is_active = true` simultaneously while consumers update their verification logic.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `endpoint_id` | UUID | Owning endpoint |
| `secret` | TEXT | Encrypted envelope: `v1$<nonce_b64url>$<ciphertext_b64url>` |
| `is_active` | BOOLEAN | False once the rotation grace period has expired |
| `expires_at` | TIMESTAMPTZ | Null for primary secrets; set for rotated-out secrets during grace period |

### `events`

Immutable record of something that happened. Created at publish time and never updated or deleted. Each event references an `event_type` and carries an arbitrary JSON payload.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | Prefix: `evn_` |
| `application_id` | UUID | |
| `event_type_id` | UUID | |
| `endpoint_id` | UUID | Nullable — null means fan-out to all matching endpoints |
| `tenant_id` | UUID | |
| `organization_id` | UUID | |
| `payload` | JSONB | Arbitrary event payload |
| `idempotency_key` | VARCHAR(256) | Optional; unique per `(application_id, idempotency_key)` |
| `tags` | JSONB | |
| No `updated_at` | | Events are immutable — no mutation columns |

### `delivery_jobs`

Mutable state for one `(event, endpoint)` delivery pair. This is the outbox record — created in the same transaction as the event. The worker claims jobs from this table, attempts delivery, and updates status.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | Prefix: `dj_` |
| `event_id` | UUID | |
| `endpoint_id` | UUID | |
| `organization_id` | UUID | Used for queue stream selection |
| `status` | TEXT | `pending`, `delivering`, `succeeded`, `failed` |
| `attempt` | INT | Current attempt count |
| `stream_name` | TEXT | Redis stream this job was enqueued to |
| `enqueued_at` | TIMESTAMPTZ | Null until the XADD succeeds |

Partial index `WHERE enqueued_at IS NULL AND status = 'pending'` is the outbox poller's scan target — it finds jobs that were written to Postgres but whose Redis XADD failed.

### `delivery_attempts`

Append-only log of every HTTP call made for a delivery job. One row per attempt. Never updated after insert — the full retry history is always queryable.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `delivery_job_id` | UUID | Parent job |
| `event_id` | UUID | Denormalized for direct lookup |
| `endpoint_id` | UUID | Denormalized for direct lookup |
| `attempt_number` | INT | 1-based attempt index |
| `status` | TEXT | `success`, `failed`, `timeout` |
| `http_status` | INT | HTTP response code; null on timeout or connection error |
| `response_body` | TEXT | First N bytes of the response; null on timeout |
| `latency_ms` | INT | Round-trip time in milliseconds |
| `attempted_at` | TIMESTAMPTZ | |

---

## Scheduling

The scheduler binary owns a Redis sorted set per shard (`sched:pending:{shard}`) where each member is a `schedule_id` scored by its `next_run_at` Unix timestamp. On each tick it calls `ZRANGEBYSCORE` to find due schedules, acquires a per-schedule fire lock, and calls the transactional fire function.

### `schedules`

A cron expression paired with an event type and a fixed payload. When the scheduler fires a schedule it inserts an `event` and a `delivery_job` for each associated endpoint, updates `next_run_at`, and re-scores the entry in the Redis sorted set.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(24) | |
| `tenant_id` | UUID | |
| `organization_id` | UUID | |
| `event_type_id` | UUID | Event type to publish on each fire |
| `payload` | JSONB | Fixed payload injected into every fired event |
| `cron_expression` | VARCHAR(100) | Standard 5-field cron; e.g. `0 9 * * 1-5` |
| `timezone` | VARCHAR(64) | IANA tz name; default `UTC` |
| `status` | VARCHAR(20) | `active`, `paused`, `disabled` |
| `next_run_at` | TIMESTAMPTZ | Precomputed next fire time; indexed for poller |
| `last_run_at` | TIMESTAMPTZ | Timestamp of most recent fire |
| `last_run_status` | VARCHAR(20) | `fired`, `skipped`, `error` |
| `assigned_shard` | SMALLINT | Which scheduler shard owns this schedule |

### `schedule_endpoints`

Join table: which endpoints a schedule fans out to when it fires. A single schedule can target multiple endpoints.

| Column | Type | Notes |
|---|---|---|
| `schedule_id` | UUID | |
| `endpoint_id` | UUID | |
| PK | | `(schedule_id, endpoint_id)` |

### `schedule_executions`

One row per schedule fire. Provides a queryable history of when a schedule ran, how long it took, and whether it succeeded. Append-only after insert.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `schedule_id` | UUID | |
| `status` | VARCHAR(20) | `pending`, `running`, `success`, `partial_failure`, `failure` |
| `triggered_at` | TIMESTAMPTZ | When the scheduler decided to fire |
| `started_at` | TIMESTAMPTZ | When the transaction began |
| `completed_at` | TIMESTAMPTZ | When the transaction committed |
| `error_message` | TEXT | Populated on failure |

### `scheduler_shards`

Registry of all scheduler shards. Each shard is an independent sorted set in Redis. Multiple shards allow horizontal scaling of the scheduler and enable enterprise tenants to be pinned to a dedicated shard for SLA isolation.

| Column | Type | Notes |
|---|---|---|
| `id` | SMALLINT | PK; shard number (0, 1, 2, …) |
| `state` | VARCHAR(20) | `active`, `draining`, `paused`, `drained` |
| `redis_url` | VARCHAR(255) | Redis node for this shard's sorted set |
| `note` | TEXT | Operator notes |

### `tenant_shard_affinity`

Pins a tenant to a specific scheduler shard. Used for enterprise SLA isolation — a tenant's schedules always fire on their dedicated shard regardless of how the scheduler scales.

| Column | Type | Notes |
|---|---|---|
| `tenant_id` | UUID | PK |
| `shard_id` | SMALLINT | References `scheduler_shards.id` |
| `note` | TEXT | Reason for pinning |

---

## RBAC

Permissions are the atomic unit. Roles bundle permissions. Assignments attach roles or individual permissions to users and API keys, with optional expiry.

### `permissions`

A `(resource, action)` pair representing a single capability (e.g. `endpoint` + `delete`). System permissions are seeded at startup and cannot be modified via the API. Tenants may define custom permissions.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | |
| `tenant_id` | UUID | Null for system permissions |
| `name` | VARCHAR(128) | Unique per tenant; globally unique when `tenant_id` is null |
| `perm_type` | ENUM | `system`, `custom` |
| `resource` | VARCHAR(64) | Resource category (e.g. `endpoint`) |
| `action` | VARCHAR(64) | Operation (e.g. `delete`) |

### `roles`

A named collection of permissions. System roles are seeded at startup. Tenant-defined roles allow grouping of custom and system permissions for assignment.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | |
| `tenant_id` | UUID | |
| `name` | VARCHAR(128) | Unique per `(tenant_id, name) WHERE deleted_at IS NULL` |
| `is_system` | BOOLEAN | System roles cannot be deleted |

### `role_permissions`

Join table linking roles to permissions. Composite PK `(role_id, permission_id)`.

### `user_roles`

Assigns a role to a user within a tenant. Optional `expires_at` for time-bounded access.

| Column | Type | Notes |
|---|---|---|
| `user_public_id` | VARCHAR(20) | References `users.public_id` |
| `role_id` | UUID | |
| `tenant_id` | UUID | |
| `expires_at` | TIMESTAMPTZ | Nullable; role revokes after this time |
| PK | | `(user_public_id, role_id)` |

### `user_permissions`

Direct permission assignment to a user, bypassing roles. Same structure as `user_roles` but references a `permission_id`.

### `api_key_roles`

Assigns a role to an API key. Same semantics as `user_roles` but keyed by `api_key_public_id`.

### `api_key_permissions`

Direct permission assignment to an API key.

---

## Credentials

### `api_keys`

Bearer tokens for API access. The raw key is shown exactly once at creation and is never stored in plaintext — only an argon2 hash (`key_hash`) is persisted. An optional AES-256-GCM encrypted copy (`key_encrypted`) is stored if the tenant's settings allow post-creation retrieval.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | |
| `tenant_id` | UUID | |
| `user_id` | UUID | Owning user |
| `name` | VARCHAR(64) | Unique per user |
| `key_hash` | TEXT | Argon2 hash; used for authentication |
| `key_encrypted` | TEXT | AES-256-GCM encrypted copy; null if not enabled |
| `key_prefix` | VARCHAR(3) | First 3 chars of the raw key; shown in listings for identification |
| `environment` | ENUM | `live`, `test`, `dev`, `sandbox` |
| `status` | ENUM | `active`, `expired` |
| `expires_at` | TIMESTAMPTZ | Null means no expiry |
| `last_used_at` | TIMESTAMPTZ | Updated on each successful authentication |

### `api_key_settings`

Per-tenant configuration for API key issuance behavior. One row per tenant.

| Column | Type | Notes |
|---|---|---|
| `tenant_id` | UUID | Unique per `(organization_id, tenant_id)` |
| `max_keys_per_user` | INTEGER | Nullable; caps how many active keys a user can hold |
| `key_length` | SMALLINT | Length of the generated raw key; default 32 |
| `default_ttl_seconds` | INTEGER | Nullable; applied to keys that don't specify expiry |
| `allow_view_later` | BOOLEAN | Whether `key_encrypted` is stored to enable post-creation retrieval |

### `jwt_keys`

RS256, ES256, or ES384 key pairs for JWT signing or webhook signature verification. Private keys are stored AES-256-GCM encrypted. Supports rotation with a grace period during which both the old and new key are valid — consumers have time to cache the new JWKS before the old key is disabled.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | |
| `tenant_id` | UUID | |
| `name` | VARCHAR(128) | |
| `key_use` | ENUM | `authentication`, `webhook_signature` |
| `algorithm` | ENUM | `RS256`, `RS384`, `RS512`, `ES256`, `ES384`, `ES512`, `HS256`, `HS512` |
| `key_id` | VARCHAR(64) | `kid` field in the JWKS; globally unique |
| `status` | ENUM | `active`, `disabled`, `expired` |
| `public_key` | TEXT | PEM; null for symmetric keys |
| `private_key_enc` | TEXT | AES-256-GCM encrypted PEM; null for public-only entries |
| `secret_enc` | TEXT | AES-256-GCM encrypted secret; used for HMAC algorithms |
| `expires_at` | TIMESTAMPTZ | Nullable |
| `grace_period_ends_at` | TIMESTAMPTZ | After rotation: old key stays valid until this time |
| `rotated_from_id` | VARCHAR(20) | Public ID of the key this was rotated from |

---

## Platform Webhooks

Platform webhooks are a system-level notification channel, separate from the tenant-level event delivery pipeline. Hookly fires platform events when its own resources change — a key is rotated, a user is invited, an endpoint is disabled. Tenants subscribe to the event types they care about and nominate one or more HTTP endpoints to receive them.

### `platform_event_types`

System-defined catalog of observable platform events. Read-only from the API; seeded at startup. 27 event types across 7 resource categories.

| Public ID prefix | Resource | Actions covered |
|---|---|---|
| `pet_apk_*` | `api_key` | `created`, `updated`, `deleted`, `rotated` |
| `pet_env_*` | `environment` | `created`, `updated`, `deleted`, `disabled` |
| `pet_jwk_*` | `jwt_key` | `created`, `rotated`, `disabled`, `deleted` |
| `pet_rol_*` | `role` | `created`, `updated`, `deleted` |
| `pet_usr_*` | `user` | `invited`, `joined`, `deleted`, `role_assigned`, `role_removed` |
| `pet_ep_*` | `endpoint` | `created`, `updated`, `deleted`, `disabled` |
| `pet_app_*` | `application` | `created`, `updated`, `deleted` |

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | e.g. `pet_apk_created` |
| `name` | VARCHAR(128) | Unique; e.g. `api_key.created` |
| `resource` | VARCHAR(64) | e.g. `api_key` |
| `action` | VARCHAR(64) | e.g. `created` |

### `platform_webhooks`

A tenant-owned HTTP endpoint that receives platform notifications. Each webhook has its own AES-256-GCM encrypted signing secret for HMAC-SHA256 payload signing.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR(20) | |
| `tenant_id` | UUID | |
| `name` | VARCHAR(128) | Unique per `(tenant_id, name) WHERE deleted_at IS NULL` |
| `url` | TEXT | Delivery target |
| `signing_secret_enc` | TEXT | AES-256-GCM encrypted HMAC secret |
| `status` | ENUM | `active`, `suspended`, `disabled` |

### `platform_webhook_subscriptions`

Opt-in table: which platform event types a tenant's webhooks receive. All active `platform_webhooks` for the tenant receive every subscribed event type. Composite PK `(tenant_id, event_type_public_id)` prevents duplicates.

| Column | Type | Notes |
|---|---|---|
| `tenant_id` | UUID | |
| `event_type_public_id` | VARCHAR(20) | References `platform_event_types.public_id` |
| PK | | `(tenant_id, event_type_public_id)` |

---

## Key indexes

| Table | Index | Purpose |
|---|---|---|
| `tenants` | `(organization_id)` | List tenants by org |
| `users` | `(organization_id)`, `(tenant_id)`, `(email)` | Auth lookup and tenant membership queries |
| `invites` | `(token_hash)` | Accept/verify flow — single lookup by raw token hash |
| `invites` | `(expires_at) WHERE status IN ('sent','opened','failed')` | TTL expiry sweep |
| `tenant_members` | `(tenant_id, user_email) WHERE deleted_at IS NULL` (unique) | Prevent duplicate memberships |
| `endpoints` | `event_types` GIN | Route an event to all subscribed endpoints |
| `endpoint_secrets` | `(endpoint_id, is_active)` | Fetch active signing secrets for an endpoint |
| `events` | `(application_id, idempotency_key) WHERE idempotency_key IS NOT NULL` (unique) | Idempotent publish |
| `events` | `(application_id, event_type_id, created_at DESC)` | List events by type |
| `delivery_jobs` | `(created_at) WHERE enqueued_at IS NULL AND status = 'pending'` | Outbox poller scan |
| `delivery_attempts` | `(delivery_job_id)`, `(event_id)` | Fetch attempt history for a job or event |
| `schedules` | `(next_run_at) WHERE status = 'active' AND deleted_at IS NULL` | Reconciliation scan |
| `schedules` | `(assigned_shard) WHERE deleted_at IS NULL` | Shard-scoped schedule list |
| `platform_webhooks` | `(tenant_id, status) WHERE deleted_at IS NULL` | List active webhooks per tenant |
| `platform_webhooks` | `(tenant_id, name) WHERE deleted_at IS NULL` (unique) | Name uniqueness per tenant |
| `platform_webhook_subscriptions` | `(event_type_public_id)` | Fan-out: which tenants subscribe to a given event type |
| `api_keys` | `(key_hash)` (unique) | Authentication — hash lookup |
| `api_keys` | `(tenant_id, user_id) WHERE deleted_at IS NULL` | List keys by user |
| `jwt_keys` | `(grace_period_ends_at) WHERE grace_period_ends_at IS NOT NULL AND status = 'active'` | Background task: disable expired grace period keys |
| `permissions` | `(name) WHERE tenant_id IS NULL` (unique) | System permission name uniqueness |
| `roles` | `(tenant_id, name) WHERE deleted_at IS NULL` (unique) | Role name uniqueness per tenant |
