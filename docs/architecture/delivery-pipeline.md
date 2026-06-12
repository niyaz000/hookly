# Delivery pipeline

This document covers how an event moves from tenant emission to delivery at an endpoint. The pipeline is divided into two phases: **emission** (synchronous, API server) and **dispatch** (asynchronous, delivery worker).

Platform webhooks (system notifications to tenants when Hookly itself changes) reuse this same mechanism — they produce `delivery_jobs` rows and enqueue to the same Redis streams. Their fan-out logic is different (they target `platform_webhooks` rows, not tenant `endpoints`), but the delivery worker, retry policy, and stream model are shared. This document focuses on the primary path: tenant-published events delivered to tenant-owned endpoints.

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
  API Handler → PostgreSQL: BEGIN; INSERT events; INSERT delivery_jobs (one per matching endpoint); COMMIT
  API Handler → Redis Stream: XADD hookly:q:tier:{tier} { delivery_job_id, tenant_id, event_type } [best-effort]
  [async boundary — handler returns 202 to caller]
  Delivery Worker → Redis Stream: XREADGROUP (blocks until message available)
  Delivery Worker → PostgreSQL: SELECT delivery_job + endpoint + event (JOIN)
  Delivery Worker → PostgreSQL: INSERT delivery_attempts (status=delivering)
  Delivery Worker → Tenant Endpoint: POST /your-webhook { payload, X-Hookly-Signature }
  Tenant Endpoint → Delivery Worker: 200 OK
  Delivery Worker → PostgreSQL: UPDATE delivery_attempts (status=succeeded); UPDATE delivery_jobs (status=delivered)
  Delivery Worker → Redis Stream: XACK

On failure path:
  Tenant Endpoint → Delivery Worker: 5xx / timeout
  Delivery Worker → PostgreSQL: UPDATE delivery_attempts (status=failed); UPDATE delivery_jobs (attempt_count++, next_retry_at)
  [if attempts < max_retries]: Delivery Worker → Redis Stream: XADD (re-enqueue after backoff delay)
  [if attempts >= max_retries]: delivery_jobs marked dead_lettered

Style: use dashed line for async boundary
-->

---

## Phase 1 — Event emission (API server)

When a tenant publishes an event (e.g. `POST /api/v1/events`), the handler:

1. Resolves which active `endpoints` are subscribed to the event type within the tenant's application
2. Writes atomically in a single transaction:
   - One `events` row (immutable record of the occurrence)
   - One `delivery_jobs` row per matching endpoint (the outbox record, status `pending`)
3. For each `delivery_jobs` row, attempts a best-effort `XADD` to the appropriate Redis stream
4. Returns `202 Accepted` to the caller — the HTTP response is not gated on delivery

The `XADD` is fire-and-forget from the handler's perspective. If Redis is unavailable, the insert still commits. An outbox poller runs in the worker process every few seconds, scanning for `delivery_jobs` rows with status `pending` and no corresponding Redis entry, and re-enqueues them. PostgreSQL is the durable source of truth; Redis is an acceleration layer.

### Stream selection

Each `delivery_jobs` row is enqueued to one of two stream families based on tenant configuration:

| Stream key | Used for | Worker assignment |
|---|---|---|
| `hookly:q:tier:high` | High-priority tenants, security-sensitive delivery (e.g. key rotation events) | Dedicated high-tier worker pool |
| `hookly:q:tier:default` | All other tenants on shared infrastructure | Shared default worker pool |
| `hookly:q:org:{org_id}` | Tenants with isolated delivery (enterprise plan) | Dedicated per-org workers |

**Tier streams** (`hookly:q:tier:{tier}`) are consumed by worker pools that subscribe to a single tier. Adding more worker instances to a consumer group automatically distributes load — Redis assigns each message to exactly one worker in the group. Scaling the high tier does not affect the default tier, and vice versa.

**Org streams** (`hookly:q:org:{org_id}`) give a specific organization its own stream, consumed only by workers assigned to that org. This prevents a high-volume tenant from consuming worker capacity allocated to another tenant and provides a contractual isolation guarantee for enterprise customers. A tenant on an org stream does not share consumer group membership with any tier stream workers.

The routing decision is made at emission time based on the `delivery_tier` field on the `tenants` row. If a tenant has `isolated_delivery = true`, their org stream is used instead of any tier stream.

---

## Phase 2 — Delivery dispatch (worker)

The worker binary (`src/worker/main.rs`) runs a consumer loop per stream assignment:

1. **`XREADGROUP`** — blocks for up to 5 seconds waiting for new messages in the consumer group
2. **Load** — fetches the `delivery_jobs` row joined with `endpoints` and `events` from PostgreSQL; skips if the job is already in a terminal state (delivered, dead-lettered) to handle duplicate delivery from XAUTOCLAIM recovery
3. **Sign** — decrypts the `endpoint_secrets` row using the per-tenant AES-256-GCM key, computes `HMAC-SHA256(payload)` using the active signing secret; if a rotation grace period is active, the older secret is also valid for verification by the recipient
4. **Deliver** — `POST` to the endpoint URL with headers:
   ```
   X-Hookly-Signature: sha256=<hex_digest>
   X-Hookly-Event: <event_type>
   X-Hookly-Delivery: <delivery_attempt_id>
   Content-Type: application/json
   ```
5. **Record** — writes a `delivery_attempts` row with HTTP status, response body (truncated to 4 KB), and round-trip latency
6. **Update** — marks `delivery_jobs.status` as `delivered` on success, or increments `attempt_count` and sets `next_retry_at` on failure
7. **`XACK`** — acknowledges the message; delivery failures are tracked in the database, not the stream

### XAUTOCLAIM recovery

If a worker crashes after processing a message but before `XACK`, the message stays in the pending entry list (PEL). A separate recovery loop runs `XAUTOCLAIM` every 30 seconds, reclaiming messages that have been in the PEL for more than 60 seconds. Step 2 above (checking terminal state before acting) makes this idempotent — a re-claimed message that was already delivered is detected and acked immediately without re-sending.

### Retry policy

Failed deliveries (non-2xx response or connection error) are retried with exponential backoff:

| Attempt | Delay |
|---|---|
| 1 | immediate |
| 2 | 30 seconds |
| 3 | 5 minutes |
| 4 | 30 minutes |
| 5 | 2 hours |
| 6+ | dead-lettered |

`delivery_jobs.next_retry_at` gates when the worker picks up a retry. The worker only re-enqueues a job when `next_retry_at <= now()`, so a crashed worker that restarts early will skip jobs that are not yet due.

---

## Signature verification (recipient side)

Recipients verify the `X-Hookly-Signature` header using the endpoint's signing secret:

```python
import hmac, hashlib, base64

def verify(payload: bytes, signature: str, secret: str) -> bool:
    # secret starts with "whsec_"; strip the prefix and base64-decode
    raw_secret = base64.urlsafe_b64decode(secret[len("whsec_"):] + "==")
    expected = "sha256=" + hmac.new(raw_secret, payload, hashlib.sha256).hexdigest()
    return hmac.compare_digest(expected, signature)
```

The signing secret is revealed once on endpoint creation and once on explicit secret rotation (`POST /endpoints/:id/rotate-secret`). During a rotation grace period both the old and new secrets produce valid signatures, giving recipients time to update their stored secret before the old one is retired.

---

## Delivery guarantees

| Property | Guarantee |
|---|---|
| At-least-once | Yes — `XAUTOCLAIM` recovery re-delivers if a worker crashes after delivery but before `XACK` |
| At-most-once | No — duplicate delivery is possible on worker crash; recipients should deduplicate on `X-Hookly-Delivery` |
| Order | Best-effort within a stream; no ordering guarantee across tenants or across streams |
| Durability | `delivery_jobs` is written before any XADD; a Redis restart loses the stream entry but not the job — the outbox poller recovers it |
| Tenant isolation | A slow or high-volume tenant on a shared tier stream can delay other tenants on the same stream; org streams eliminate this for enterprise tenants |
