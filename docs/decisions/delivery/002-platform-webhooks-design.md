# ADR-004: Platform webhook system design

## Status
Accepted

## Context

The platform needs to notify tenants when administrative configuration changes occur — for example, when an environment is added, an API key is rotated, or a role is deleted. This is a different concern from the tenant-facing webhook delivery system (which delivers events from tenant applications to their customers).

Key design questions:

1. **Where do webhook endpoints live?** Reuse the existing `endpoints` table with a discriminator, or a separate table?
2. **What are tenants subscribing to?** Per-endpoint subscriptions or per-tenant global subscriptions?
3. **Who defines the event types?** Tenants or the platform?
4. **How are secrets handled?** Same pattern as application webhook endpoints?

## Decision

### Separate tables

Platform webhooks live in `platform_webhooks`, not in the existing `endpoints` table. The two have meaningfully different shapes:
- Application endpoints are scoped to an application and linked to event types the tenant defined
- Platform webhooks are scoped to a tenant and linked to system-defined event types

Mixing them with a discriminator column would complicate queries, indexes, and future migrations.

### System-defined event type catalog

The platform defines 27 event types in a `platform_event_types` table, seeded via migration. Event types follow a `<resource>.<action>` naming convention (e.g., `api_key.rotated`, `environment.disabled`). Resources covered: `api_key`, `environment`, `jwt_key`, `role`, `user`, `endpoint`, `application`.

Tenants cannot create or modify platform event types. The catalog is read-only from the API.

### Per-tenant global subscriptions

A tenant subscribes globally — all active platform webhooks for that tenant receive events for the subscribed event types. This is simpler than per-endpoint subscriptions and reflects the real use case: a tenant wants to know about `api_key.deleted` events regardless of which of their endpoints handles it.

The `platform_webhook_subscriptions` table has a composite PK of `(tenant_id, event_type_public_id)`.

### Subscription table uses public_id strings, not UUIDs

The subscriptions table stores `event_type_public_id VARCHAR(20)` rather than a UUID FK pointing to `platform_event_types.id`. This is consistent with the no-FK-constraints decision (ADR-001) and avoids a UUID lookup on every subscribe call. The public IDs are stable and system-defined, so they are safe to treat as the canonical identifier.

### Max 10 webhooks per tenant

Enforced via a COUNT query before insert. The count includes `active` and `suspended` webhooks but excludes `disabled` (soft-deleted lifecycle state). This prevents the limit from being circumvented by suspending webhooks.

### Soft failure for unknown event type IDs on subscribe

When a subscribe request includes unknown event_type_ids, the valid ones are subscribed and the invalid ones are returned as `invalid_event_type_ids` in the response — a partial success rather than a hard rejection. This allows clients to handle catalog additions gracefully across API versions.

## Consequences

**Positive:**
- Clean separation of tenant-facing and platform-facing webhook concerns
- Subscriptions are simple to reason about — one row per (tenant, event type)
- Partial subscribe success reduces friction for clients with stale event type catalogs

**Negative:**
- Two webhook tables to maintain; operators must distinguish platform webhooks from application endpoints in runbooks
- The 10-webhook limit is hard-coded in the repository layer (`MAX_WEBHOOKS_PER_TENANT`) — changing it requires a code deploy, not a config change
- Subscription fan-out at delivery time (find all active webhooks for a tenant, for each subscribed event type) must be efficient; indexes on `(tenant_id, status)` and `event_type_public_id` cover the expected query patterns
