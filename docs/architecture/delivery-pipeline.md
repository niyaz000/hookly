# Delivery pipeline

This document covers how an event moves from emission to delivery at a tenant's endpoint. The pipeline is divided into two phases: **emission** (synchronous, API server) and **dispatch** (asynchronous, delivery worker).

---

## Overview

<!-- diagram: Delivery pipeline sequence diagram
Lanes (left to right):
  1. "API Handler" (Rust async fn)
  2. "PostgreSQL"
  3. "Redis Stream"
  4. "Delivery Worker"
  5. "Tenant Endpoint" (external HTTP)

Sequence:
  API Handler → PostgreSQL: INSERT event row
  API Handler → Redis Stream: XADD hookly:delivery:<tier> { event_id, tenant_id, event_type }
  [async boundary — handler returns 202 to caller]
  Delivery Worker → Redis Stream: XREADGROUP (blocks until message available)
  Delivery Worker → PostgreSQL: SELECT active platform_webhooks for tenant + subscribed event type
  Delivery Worker → PostgreSQL: INSERT delivery_attempt (status=delivering)
  Delivery Worker → Tenant Endpoint: POST /webhook { payload, X-Hookly-Signature }
  Tenant Endpoint → Delivery Worker: 200 OK
  Delivery Worker → PostgreSQL: UPDATE delivery_attempt (status=succeeded)
  Delivery Worker → Redis Stream: XACK

On failure path:
  Tenant Endpoint → Delivery Worker: 5xx / timeout
  Delivery Worker → PostgreSQL: UPDATE delivery_attempt (status=failed, increment attempts, set next_retry_at)
  [if attempts < max_retries]: Delivery Worker → Redis Stream: XADD (re-enqueue with delay)
  [if attempts >= max_retries]: delivery_attempt marked as dead-lettered

Style: use dashed line for async boundary
-->

---

## Phase 1 — Event emission (API server)

When an admin action occurs (e.g., an API key is deleted), the handler:

1. Completes the primary database write (the mutation itself)
2. Calls `queue::xadd(redis, stream, payload)` with the event metadata
3. Returns the HTTP response to the caller

The Redis `XADD` is a fire-and-forget from the handler's perspective. If the Redis call fails, the event is not re-tried by the handler — the primary operation has already succeeded. A future improvement is an outbox pattern that makes emission transactional with the primary write.

Stream key selection is based on delivery tier:

| Tier | Stream key | Used for |
|---|---|---|
| `high` | `hookly:delivery:high` | Security-sensitive events (key rotation, role changes) |
| `default` | `hookly:delivery:default` | All other platform events |

---

## Phase 2 — Delivery dispatch (worker)

The worker binary (`src/worker/main.rs`) runs a tight loop per stream:

1. **`XREADGROUP`** — blocks for up to 5 seconds waiting for new messages in the consumer group
2. **Fan-out** — for each message, queries:
   - Which tenants are subscribed to this event type?
   - For each subscribed tenant, which platform webhooks are `active`?
3. **Sign** — for each target webhook, decrypts the signing secret and computes `HMAC-SHA256(payload)`
4. **Deliver** — `POST` to the webhook URL with headers:
   ```
   X-Hookly-Signature: sha256=<hex_digest>
   X-Hookly-Event: api_key.deleted
   X-Hookly-Delivery: <delivery_attempt_id>
   ```
5. **Record** — writes a `delivery_attempts` row with the HTTP status code, response body (truncated), and latency
6. **`XACK`** — acknowledges the message regardless of delivery outcome (delivery failures are tracked in the DB, not the stream)

### Retry policy

Failed deliveries (non-2xx or connection error) are retried with exponential backoff:

| Attempt | Delay |
|---|---|
| 1 | immediate |
| 2 | 30 seconds |
| 3 | 5 minutes |
| 4 | 30 minutes |
| 5 | 2 hours |
| 6+ | dead-lettered |

The `next_retry_at` column on `delivery_jobs` gates when the worker picks up a retry.

---

## Signature verification (tenant side)

Tenants verify the `X-Hookly-Signature` header using their webhook's signing secret:

```python
import hmac, hashlib

def verify(payload: bytes, signature: str, secret: str) -> bool:
    # secret starts with "whsec_"; strip the prefix and base64-decode
    import base64
    raw_secret = base64.urlsafe_b64decode(secret[len("whsec_"):] + "==")
    expected = "sha256=" + hmac.new(raw_secret, payload, hashlib.sha256).hexdigest()
    return hmac.compare_digest(expected, signature)
```

The signing secret is only revealed on webhook creation and on explicit `POST /platform-webhooks/:id/rotate-secret`. Store it securely — it cannot be retrieved after the initial response.

---

## Delivery guarantees

| Property | Guarantee |
|---|---|
| At-least-once | Yes — consumer groups with `XACK` after processing |
| At-most-once | No — a worker crash after delivery but before `XACK` will re-deliver |
| Order | Best-effort within a stream; no ordering across tenants |
| Idempotency | Tenants should use the `X-Hookly-Delivery` ID for deduplication |
