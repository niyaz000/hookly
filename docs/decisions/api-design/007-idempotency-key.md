# ADR api-design/007: Idempotency key design and replay semantics

## Status
Accepted

## Context

Webhook delivery infrastructure is operated over unreliable networks. A client creating an
event or a schedule can't always distinguish "the request failed" from "the request succeeded
but the response was lost." Without idempotency, the correct recovery strategy — retry —
becomes dangerous: a duplicate event fires a webhook twice; a duplicate schedule registers
conflicting cron entries.

Idempotency keys give clients a safe retry primitive: the server tracks whether a request
with a given key has already been executed and, if so, returns the original result without
re-executing.

Stripe popularized the single-header `Idempotency-Key` convention that is now the de facto
standard across the developer ecosystem. The OASIS Repeatable Requests v1.0 spec defines an
alternative two-header format (`Repeatability-Request-ID` + `Repeatability-First-Sent`).

## Decision

### Header

```
Idempotency-Key: <client-generated-string>
```

- Header name: `Idempotency-Key`
- Value: any ASCII string, 1–64 characters
- Generation: client's responsibility — a UUIDv4 or UUIDv7 is recommended
- Missing header: request proceeds without idempotency protection (normal flow)
- Empty or >64-char value: `400 Bad Request`

### Scope

Idempotency is supported on **events** and **schedules** only:

| Endpoint | Supported |
|---|---|
| `POST /api/v1/events` | Yes — duplicate create fires a webhook twice |
| `POST /api/v1/schedules` | Yes — duplicate create registers conflicting cron entries |
| All other `POST` endpoints | No — header silently ignored |
| `GET`, `DELETE`, `PUT`, `PATCH` | No — not applicable |

All other `POST` handlers ignore the header silently. The API documentation for each
endpoint states whether the header is supported.

### Replay semantics

**Same key, same body → replay (no re-execution):**
```
Client sends: POST /api/v1/events  { "event_type_id": "...", "payload": {...} }  Idempotency-Key: abc123
Server responds: 201 Created  { "id": "evn_..." }

Client retries (same key, same body):
Server responds: 200 OK  { "id": "evn_..." }  ← same entity row, no second insert
```

**Same key, different body → 409 Conflict:**
```json
{
  "error_code": "conflict",
  "error_message": "Idempotency key already used with a different request body"
}
```

**Concurrent requests with same key → 409 Conflict:**
```json
{
  "error_code": "conflict",
  "error_message": "A concurrent request with this idempotency key is already in progress"
}
```

**Key reuse after 1-hour window → treated as a new request:**
A key used more than 1 hour ago is no longer in the lookup window. The next request with
that key executes fresh. Keys should be unique per operation — UUIDv4 or UUIDv7 is
recommended to eliminate any reuse risk.

### Durability

Idempotency is a **core guarantee**, not best-effort. The idempotency record is stored on
the entity row in PostgreSQL — the same durability as the entity itself. A Redis outage
affects only the concurrent-request lock (in-flight protection); completed records in
PostgreSQL are unaffected.

## Principles upheld

- **Reliability through simplicity** — clients can always retry a failed request safely;
  no duplicate resource creation; no duplicate delivery triggers
- **Developer experience** — `Idempotency-Key` is the Stripe convention known to the
  majority of API developers; single header, no timestamp companion required
- **Focused scope** — only the two entities where duplicate creates have irreversible
  side effects carry idempotency overhead; all other handlers are unaffected

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| OASIS `Repeatability-Request-ID` + `Repeatability-First-Sent` headers | Two headers instead of one; `Repeatability-First-Sent` (timestamp) adds validation complexity for no additional correctness guarantee in our use case |
| Apply to all state-mutating POST endpoints | Creates, updates, and soft-deletes on non-core entities (applications, endpoints, invites) are idempotent enough by nature — same name or URL produces an obvious conflict, not a silent duplicate. Adding idempotency overhead to every handler for no safety benefit. |
| Redis-only storage (no PostgreSQL) | Volatile: node crash or eviction loses all records → silent duplicate execution. RAM is ~100× more expensive than disk per GB. Entity-level columns in PostgreSQL are durable and cost nothing additional. |
| Configurable TTL | Adds an env var and operational surface for no real benefit. Clients aren't tuning retry window granularity below hours. Fixed 24 hours matches Stripe and covers all automated retry scenarios. |
| Forever deduplication (unique constraint, no TTL) | Prevents key reuse after the resource is deleted months later. A client rotating keys on a schedule (or reusing short keys) would hit stale 409s with no clear expiry. 24-hour window is sufficient and predictable. |
| Body field `idempotency_key` in request JSON | Mixes idempotency identity into the resource model. Standard practice (Stripe, Square, Adyen) is to separate it as a header. Also produces two competing idempotency mechanisms if both a header and a body field are present. |

## Consequences

**Positive:**
- Clients can retry any event or schedule create safely without fear of duplicates
- Body hash comparison catches accidental key reuse with different payloads → 409
- 1-hour window covers all automated retry windows — Hookly clients are code-level retriers, not overnight batch jobs
- No extra infrastructure — idempotency durability is inherited from the entity table
- Lock TTL of 60 seconds caps the concurrent-request block window

**Negative:**
- Redis is required for concurrent-request protection — a Redis outage allows duplicate
  concurrent requests through; the SELECT-before-INSERT catches most duplicates once the
  first request completes, but is not a perfect substitute for the lock
- 1-hour replay window means a key can't be safely reused for a different operation within
  that window (use UUIDv4/v7 to make reuse a non-issue)
- Only events and schedules are protected; other endpoints offer no idempotency guarantee

## Implementation

For storage schema, request flow, lock protocol, and failure semantics see
[docs/architecture/idempotency.md](../../architecture/idempotency.md).
For storage backend rationale and alternatives see
[docs/decisions/database/004-idempotency-storage.md](../database/004-idempotency-storage.md).
