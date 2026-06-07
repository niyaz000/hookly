# Data model

All tables use:
- `id UUID` as primary key (generated via `Uuid::now_v7()` — time-ordered)
- `public_id VARCHAR` as the API-facing identifier (prefixed NanoId)
- `created_at TIMESTAMPTZ DEFAULT NOW()`
- Soft deletes via `deleted_at TIMESTAMPTZ` where applicable

No foreign key constraints are defined. See [ADR: No FK constraints](../decisions/database/001-no-fk-constraints.md) for rationale.

---

## Domain map

<!-- diagram: Entity relationship map (no FK arrows, just ownership groupings)
Group 1 — Organization layer:
  organizations → tenants → users
  organizations → teams → (users via team_members)
  tenants → invites

Group 2 — Application layer:
  applications → event_types
  applications → endpoints
  endpoints → events → delivery_jobs

Group 3 — Credentials:
  users → api_keys
  api_keys → api_key_assignments (→ roles)
  users → user_assignments (→ roles)
  roles → role_permissions → permissions
  tenants → jwt_keys

Group 4 — Platform webhooks:
  tenants → platform_webhooks
  platform_event_types ← platform_webhook_subscriptions → tenants

Style: group boxes with light fill, ownership direction shown by label not arrow
-->

---

## Organization layer

### `organizations`
Top-level container. An organization owns many tenants.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | PK |
| `public_id` | VARCHAR | API-facing |
| `name` | VARCHAR(128) | unique |
| `created_at` | TIMESTAMPTZ | |

### `tenants`
A tenant is a customer of the platform. All resources (endpoints, webhooks, API keys) are scoped to a tenant.

### `users`
Humans who authenticate to the platform. Users belong to an organization and can be members of multiple teams.

### `teams`
Group users within a tenant for role assignment purposes.

### `invites`
Pending invitations to join a tenant. Expire on acceptance or TTL.

---

## Application layer

### `applications`
A logical container for a set of event types and endpoints. Supports states: `active`, `suspended`, `disabled`.

### `event_types`
Customer-defined event schemas within an application. Each event type has a `name`, optional `schema` (JSON Schema for payload validation), and `tags`.

### `endpoints`
Webhook delivery targets registered by the tenant. Each endpoint has a URL, a set of subscribed event types, retry settings, and a status (`active`, `suspended`, `disabled`). Soft-deleted via `deleted_at`.

### `events`
Inbound events submitted to an application. Each event references an event type and carries an arbitrary JSON payload. Events are immutable after creation.

### `delivery_jobs`
One row per (event, endpoint) pair. Tracks delivery attempts, status (`pending`, `delivering`, `succeeded`, `failed`), next retry time, and stream name. The delivery worker processes these.

---

## Credentials

### `api_keys`
API keys issued to users or service accounts. The raw key is shown once on creation. The stored value is an encrypted hash (via `KeyProvider`). Keys can be scoped to an environment and assigned roles.

### `api_key_settings`
Per-tenant configuration for API key behavior (e.g., default expiry).

### `environments`
Named deployment environments within a tenant (e.g., `production`, `staging`). API keys and endpoints can be scoped to an environment.

### `permissions`
Atomic platform-defined capabilities. Each permission has `resource` + `action` (e.g., `endpoint` + `delete`). Seeded via migration; not user-creatable.

### `roles`
Named bundles of permissions. The platform seeds system roles; tenants can create custom roles. Linked to permissions via `role_permissions`.

### `user_assignments`
Assigns a role to a user with an optional scope (organization, tenant, application).

### `api_key_assignments`
Assigns a role to an API key with an optional scope.

### `jwt_keys`
RSA/EC key pairs for JWT signing. Each key has a `key_type` (RS256, ES256, ES384), `status` (active, rotating, disabled), and a `grace_period_ends_at` during rotation. Private key stored encrypted; public key stored in PEM. A background task disables keys whose grace period has elapsed.

---

## Platform webhooks

### `platform_event_types`
System-defined catalog of 27 observable platform events, seeded on first migration. Covers resources: `api_key`, `environment`, `jwt_key`, `role`, `user`, `endpoint`, `application`. Read-only from the API.

| Public ID prefix | Example |
|---|---|
| `pet_apk_*` | `pet_apk_created` → `api_key.created` |
| `pet_env_*` | `pet_env_disabled` → `environment.disabled` |
| `pet_jwk_*` | `pet_jwk_rotated` → `jwt_key.rotated` |
| `pet_rol_*` | `pet_rol_deleted` → `role.deleted` |
| `pet_usr_*` | `pet_usr_rol_asgn` → `user.role_assigned` |
| `pet_ep_*` | `pet_ep_disabled` → `endpoint.disabled` |
| `pet_app_*` | `pet_app_created` → `application.created` |

### `platform_webhooks`
Per-tenant webhook endpoints for receiving platform events. Status enum: `active`, `suspended`, `disabled`. Max 10 per tenant (active + suspended). Signing secret stored encrypted (AES-256-GCM). Soft-deleted via `deleted_at`.

### `platform_webhook_subscriptions`
Composite PK `(tenant_id, event_type_public_id)`. A tenant subscribes globally — all active platform webhooks for the tenant receive events for each subscribed event type.

---

## Indexes summary

Key indexes beyond PKs:

| Table | Index | Purpose |
|---|---|---|
| `platform_webhooks` | `(tenant_id, status) WHERE deleted_at IS NULL` | List by tenant and filter by status |
| `platform_webhooks` | `(tenant_id, name) WHERE deleted_at IS NULL` (unique) | Enforce unique name per tenant |
| `platform_event_types` | `resource` | Filter event types by resource |
| `platform_webhook_subscriptions` | `event_type_public_id` | Reverse lookup (which tenants subscribe to an event type) |
| `delivery_jobs` | `(endpoint_id, status, next_retry_at)` | Worker polling query |
