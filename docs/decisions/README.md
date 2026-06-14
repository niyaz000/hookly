# Architecture Decision Records

Each subdirectory groups decisions by concern. Records within a directory are numbered sequentially (`001-`, `002-`, …). Cross-references use the full relative path.

---

## api-design

Decisions about the shape of the HTTP API: response envelopes, error formats, identifier conventions, pagination, and mutation semantics.

| # | Title | Status |
|---|---|---|
| [001](api-design/001-cursor-pagination.md) | Cursor-based pagination over OFFSET | Accepted |
| [002](api-design/002-dual-id-strategy.md) | Dual ID strategy: internal UUIDv7 and public prefixed NanoId | Accepted |
| [003](api-design/003-error-response-shape.md) | Error response shape, status codes, and auth failure handling | Accepted |
| [004](api-design/004-soft-delete.md) | Soft delete as the default mutation pattern | Accepted |
| [005](api-design/005-naming-conventions.md) | Naming conventions, URL structure, and request limits | Accepted |
| [006](api-design/006-versioning-strategy.md) | API versioning strategy and breaking change policy | Accepted |
| [007](api-design/007-idempotency-key.md) | Idempotency key design and replay semantics | Accepted |
| [008](api-design/008-filtering-and-sorting.md) | Filtering and sorting query parameters | Accepted |
| [009](api-design/009-public-id-length.md) | Public ID length — 16-character NanoId on a 62-symbol alphabet | Accepted |
| [010](api-design/010-entity-tags.md) | Entity tags — structured key-value metadata on all resources | Accepted |

---

## architecture

Decisions about the overall system topology and process model.

| # | Title | Status |
|---|---|---|
| [001](architecture/001-two-binary-architecture.md) | Two-binary architecture: API server + delivery worker | Accepted |
| [002](architecture/002-scheduler-binary.md) | Scheduler as a separate binary | Accepted |
| [003](architecture/003-outbox-pattern.md) | Outbox pattern for reliable job enqueuing | Accepted |

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
| [002](database/002-postgresql-primary.md) | PostgreSQL as the primary data store | Accepted |
| [003](database/003-read-replica.md) | Read/write pool split with PostgreSQL read replica | Accepted |

---

## delivery

Decisions about the event delivery pipeline: queue technology, delivery semantics, failure handling, and worker architecture.

| # | Title | Status |
|---|---|---|
| [001](delivery/001-redis-delivery-queue.md) | Redis Streams for the delivery queue | Accepted |
| [002](delivery/002-platform-webhooks-design.md) | Platform webhook system design | Accepted |
| [003](delivery/003-at-least-once-delivery.md) | At-least-once delivery semantics | Accepted |
| [004](delivery/004-retry-policy-dead-letter.md) | Retry policy and dead-letter design | Accepted |
| [005](delivery/005-worker-pool-priority-queues.md) | Work-stealing worker pool and priority queues | Accepted |
| [006](delivery/006-circuit-breaker.md) | Per-endpoint circuit breaker | Accepted |
| [007](delivery/007-rate-limiting.md) | Per-endpoint and per-tenant rate limiting | Accepted |
| [008](delivery/008-queue-abstraction.md) | Queue backend abstraction | Accepted |
| [009](delivery/009-tenant-tiering.md) | Tenant tiering and dedicated queues | Accepted |

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

## operations

Decisions about safe operation, maintenance procedures, and infrastructure resilience.

| # | Title | Status |
|---|---|---|
| [001](operations/001-maintenance-mode.md) | Maintenance mode and safe pause/resume | Accepted |
| [002](operations/002-redis-multi-role.md) | Redis split by operational role | Accepted |
| [003](operations/003-redis-crash-recovery.md) | Redis crash recovery | Accepted |

---

## scheduler

Decisions about the cron schedule evaluation pipeline and scheduler binary.

| # | Title | Status |
|---|---|---|
| [001](scheduler/001-sorted-set-sharding.md) | Redis sorted set sharding for cron schedule evaluation | Accepted |
| [002](scheduler/002-missed-fire-policy.md) | Missed fire policy for cron schedules | Accepted |

---

## security

Decisions about credential storage, cryptographic patterns, and access control models.

| # | Title | Status |
|---|---|---|
| [001](security/001-per-tenant-signing-secrets.md) | Per-tenant AES-256-GCM encrypted signing secrets | Accepted |
| [002](security/002-rbac-model.md) | RBAC model with scoped assignments | Accepted |
| 003 | Two credential storage patterns: hash for API keys, AES-GCM for signing secrets | Planned |
| 004 | One-time credential visibility: secrets shown only on create or rotate | Planned |
