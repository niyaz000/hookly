# Architecture overview

```
                              ┌─────────────────────────────────────────┐
                              │            API Clients                  │
                              └───────────────────┬─────────────────────┘
                                                  │ REST API
  ┌─── hookly (API Server) ───────────────────────▼──────────────────────────────┐
  │                    ┌─────────────────────────────────────────────────┐       │
  │                    │           API Server                            │       │
  │                    │  auth · RBAC · CRUD · events· schedules  emit   │       │
  │                    └──────────┬──────────────────────────────────────┘       │
  └──────────────────────────────────────────────────────────────────────────────┘
                 write │                       │ outbox + events
                       ▼                       ▼
        ┌──────────────────────┐   ┌───────────────────────────┐
        │  PostgreSQL          │   │   Redis: Delivery Queue   │◀──────────────┐
        │  primary + replica   │   │   (Streams + delayed set) │               │
        └──────────────────────┘   └────────────┬──────────────┘               │
                       ▲                        │ XREADGROUP                   │ enqueue jobs
                       │ write attempts         ▼                              │
  ┌─── hookly-worker ───────────────────────────────────────────────────────┐  │
  │                    ┌────────────────────────────────────┐               │  │
  │                    │         Delivery Worker            │               │  │
  │                    │  rate limit · circuit breaker ·    │               │  │
  │                    │  retry · work-stealing pool        │               │  │
  │                    └──────────────────┬─────────────────┘               │  │
  └─────────────────────────────────────────────────────────────────────────┘  │
                                          │ HTTP POST                          │
                                          ▼                                    │
                              ┌───────────────────────┐                        │
                              │   Tenant Endpoints    │                        │
                              └───────────────────────┘                        │
                                                                               │
  ┌─── hookly-scheduler ──────────────────────────────────────────────────────┐│
  │   ┌────────────────────────┐       ┌───────────────────────────┐          ││
  │   │  Scheduler             │──────▶│  Redis: Scheduler Sets    │          ││
  │   │  cron eval · sharding  │       │  (sorted sets + locks)    │          ││
  │   └────────────────────────┘       └───────────────────────────┘          ││
  └───────────────────────────────────────────────────────────────────────────┘│
                                                                               │
  (scheduler fires cron jobs into the Delivery Queue; worker delivers them) ───┘
```

## System components

<!-- diagram: System component diagram
Boxes:
  - "API Server (hookly)" — central box
  - "Delivery Worker (worker)" — separate box to the right
  - "PostgreSQL" — database, below center
  - "Redis Streams" — message queue, between API server and worker
  - "Tenant Webhook Endpoint" — external HTTP target, far right

Connections:
  - API Server → PostgreSQL (read/write, labeled "SQLx / PgPool")
  - API Server → Redis Streams (XADD, labeled "emit events")
  - Delivery Worker → Redis Streams (XREADGROUP / XACK, labeled "consume")
  - Delivery Worker → PostgreSQL (write delivery attempts, labeled "record attempts")
  - Delivery Worker → Tenant Webhook Endpoint (HTTP POST, labeled "deliver payload")
  - HTTP Clients → API Server (labeled "REST API")

Style: horizontal flow left-to-right, API server in the center
-->

Hookly runs as two independent processes:

| Process | Binary | Role |
|---|---|---|
| API server | `hookly` | Handles all REST API traffic; emits events to the delivery queue |
| Delivery worker | `worker` | Reads from Redis Streams; dispatches HTTP payloads to tenant endpoints |

They share a PostgreSQL database and a Redis instance. The two processes have no direct network connection to each other — Redis is the coupling point.

## Request lifecycle

1. An HTTP request arrives and passes through the middleware stack: `set_request_id` → `check_uri_length` → `check_body_size` → `inject_request_context` → `access_log`
2. `set_request_id` generates a UUIDv7 and stores it in a task-local (`tokio_util::task_local`), making it available for correlation in logs
3. `inject_request_context` constructs a `RequestContext { request_id, created_by }` and inserts it into request extensions — handlers extract this to stamp `created_by` on new rows
4. The handler runs: validates input → calls repository/service → returns a typed JSON response
5. Any `AppError` is converted to a structured JSON error response via `IntoResponse`

## Shared state

`AppState` is cloned cheaply across all handlers (Arc-wrapped internals):

```
AppState {
    db:           PgPool          // connection pool, backed by deadpool
    redis:        RedisClient     // connection factory for Redis
    crypto:       TenantCrypto    // AES-256-GCM key derivation
    email:        Arc<dyn EmailService>   // currently NoopEmailService
    key_provider: Arc<dyn KeyProvider>    // encrypts API key hashes
}
```

## Middleware stack

Applied outside-in (last registered = outermost):

```
TraceLayer (tower-http)      — structured HTTP tracing
check_body_size              — rejects Content-Length > 256 KB
check_uri_length             — rejects URI > 512 bytes
set_request_id               — stamps UUIDv7 into task-local
  └─ [all /api/v1 routes]
     inject_request_context  — populates RequestContext extension
     access_log              — logs method, path, status, latency
```

## ID strategy

Two types of IDs are used throughout:

| Type | Generation | Used for |
|---|---|---|
| Internal (`id`) | `Uuid::now_v7()` | DB primary key, time-ordered, never exposed in API |
| Public (`public_id`) | `<prefix>_<NanoId>` | API-facing identifier, returned in responses |

UUIDv7 is time-ordered, which keeps B-tree index inserts efficient (no random page splits). NanoId produces a short, URL-safe random string. The prefix makes IDs self-describing in logs and client code.

## Background tasks

The API server spawns one background task at startup: a JWT key grace-period expiry checker. It runs on a 1-hour interval and calls `JwtKeyRepository::expire_grace_period_keys()`, which disables rotated keys whose grace period has elapsed.

## Error handling

All handler errors are typed as `AppError`:

```
AppError::NotFound(String)
AppError::Validation(Vec<FieldError>)
AppError::Conflict(String)
AppError::Unauthorized
AppError::PayloadTooLarge
AppError::UriTooLong
AppError::Internal(anyhow::Error)  // wraps sqlx, crypto, etc.
```

`AppError` implements `IntoResponse` and produces consistent JSON bodies:

```json
{
  "error": "not_found",
  "message": "platform webhook not found: pwh_abc123",
  "request_id": "019123...",
  "fields": null
}
```

Validation errors include a `fields` array with per-field codes and messages.
