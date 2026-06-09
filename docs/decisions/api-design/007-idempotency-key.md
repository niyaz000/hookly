# ADR api-design/007: Idempotency key design and replay semantics

## Status
Accepted

## Context

Webhook delivery infrastructure is operated over unreliable networks. A client creating an endpoint, submitting an event, or triggering an action can't always distinguish "the request failed" from "the request succeeded but the response was lost." Without idempotency, the correct recovery strategy — retry — becomes dangerous: it may create duplicate resources or fire duplicate actions.

Idempotency keys give clients a safe retry primitive: the server tracks whether a request with a given key has already been executed and, if so, returns the original response without re-executing.

The OASIS Repeatable Requests v1.0 spec (endorsed by Microsoft's API guidelines) defines `Repeatability-Request-ID` + `Repeatability-First-Sent` headers. Stripe popularized a simpler single-header convention (`Idempotency-Key`) that is now widely known across the developer ecosystem.

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

Idempotency keys are **opt-in per handler**. Not every endpoint supports them. Handlers that do not read the `Idempotency-Key` header ignore it silently. The API documentation for each endpoint states whether the header is supported.

Supported on: all state-mutating `POST` endpoints (creates and actions).
Not supported on: `GET`, `DELETE`, `PUT`, `PATCH`.

### Replay semantics

**Same key, same body → cached response:**
```
Client sends: POST /applications  { "name": "My App" }  Idempotency-Key: abc123
Server responds: 201 Created  { "id": "app_..." }

Client retries (same key, same body):
Server responds: 201 Created  { "id": "app_..." }  ← cached, no second insert
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

## Principles upheld

- **Reliability through simplicity** — clients can always retry a failed request safely; no duplicate resource creation; no duplicate delivery triggers
- **Developer experience** — `Idempotency-Key` is a Stripe convention known to the majority of API developers; single header, no timestamp companion required
- **Battle-tested components** — Redis `SET NX PX` + Lua release is a well-understood distributed locking pattern; no novel consensus primitive

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| OASIS `Repeatability-Request-ID` + `Repeatability-First-Sent` headers | Two headers instead of one; `Repeatability-First-Sent` (timestamp) adds validation complexity for no additional correctness guarantee in our use case |
| Database-backed idempotency store (PostgreSQL) | Higher write latency than Redis; requires a transaction for lock + record atomicity; adds DB write load on what should be a fast read path |
| 5-minute TTL (MS minimum) | Insufficient for webhook delivery scenarios where a client may retry an event submission minutes later; 24 hours matches the real retry window for infrastructure integrations |
| Enforce idempotency key on all POST endpoints globally | Too restrictive — health checks, auth flows, and read-only POST actions don't need it; opt-in per handler keeps the mechanism purposeful |

## Consequences

**Positive:**
- Clients can retry any idempotent POST safely without fear of duplicates
- Body hash comparison catches accidental key reuse with different payloads
- 24-hour window covers overnight retries and delayed automation pipelines
- Lock TTL of 60 seconds caps the concurrent-request block window

**Negative:**
- Redis is a required dependency for idempotency — a Redis outage degrades idempotency protection (requests proceed but replay is unavailable)
- 24-hour record retention in Redis adds memory pressure on high-volume endpoints; keys should be short and namespaced to contain growth
- No idempotency protection on endpoints that don't opt in — handlers must explicitly wire the `idempotency::resolve` call

## Implementation

For body hashing, Redis storage layout, lock protocol, and failure semantics see [docs/architecture/idempotency.md](../../architecture/idempotency.md).
