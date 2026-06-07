# Architecture Decision Records

Each subdirectory groups decisions by concern. Records within a directory are numbered sequentially (`001-`, `002-`, …). Cross-references use the full relative path.

---

## api-design

Decisions about the shape of the HTTP API: response envelopes, error formats, identifier conventions, pagination, and mutation semantics.

| # | Title | Status |
|---|---|---|
| [001](api-design/001-cursor-pagination.md) | Cursor-based pagination over OFFSET | Accepted |
| 002 | Dual ID strategy: UUIDv7 (internal) + prefixed NanoId (public) | Planned |
| 003 | API error response shape | Planned |
| 004 | Soft delete as the default mutation pattern | Planned |

---

## architecture

Decisions about the overall system topology and process model.

| # | Title | Status |
|---|---|---|
| 001 | Two-binary architecture: API server + delivery worker | Planned |

---

## auditing

Decisions about what gets recorded, where, and how audit trails are structured.

| # | Title | Status |
|---|---|---|
| 001 | Audit trail design: database triggers + application-layer events | Planned |

---

## database

Decisions about the database layer: schema conventions, integrity enforcement, and storage choices.

| # | Title | Status |
|---|---|---|
| [001](database/001-no-fk-constraints.md) | No FK constraints — integrity enforced at the application layer | Accepted |
| 002 | PostgreSQL as the primary data store | Planned |

---

## delivery

Decisions about the event delivery pipeline: queue technology, delivery semantics, and failure handling.

| # | Title | Status |
|---|---|---|
| [001](delivery/001-redis-delivery-queue.md) | Redis Streams for the delivery queue | Accepted |
| [002](delivery/002-platform-webhooks-design.md) | Platform webhook system design | Accepted |
| 003 | At-least-once delivery semantics + idempotency keys | Planned |
| 004 | Retry policy and dead-letter design | Planned |

---

## language

Decisions about the programming language and core runtime framework.

| # | Title | Status |
|---|---|---|
| 001 | Why Rust | Planned |
| 002 | Axum + Tokio as the HTTP and async runtime | Planned |

---

## logging

Decisions about structured logging: what to emit, what to suppress, and log volume discipline.

| # | Title | Status |
|---|---|---|
| 001 | Structured logging strategy | Planned |

---

## multi-tenancy

Decisions about how tenant isolation, data boundaries, and the resource hierarchy are modeled.

| # | Title | Status |
|---|---|---|
| 001 | Organization → Tenant → Application hierarchy | Planned |

---

## observability

Decisions about how the platform surfaces its own state to operators and tenants.

| # | Title | Status |
|---|---|---|
| 001 | Observability model: self-serve event traces for tenants | Planned |

---

## security

Decisions about credential storage, cryptographic patterns, and access control models.

| # | Title | Status |
|---|---|---|
| [001](security/001-per-tenant-signing-secrets.md) | Per-tenant AES-256-GCM encrypted signing secrets | Accepted |
| [002](security/002-rbac-model.md) | RBAC model with scoped assignments | Accepted |
| 003 | Two credential storage patterns: hash for API keys, AES-GCM for signing secrets | Planned |
| 004 | One-time credential visibility: secrets shown only on create or rotate | Planned |
