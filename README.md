# Hookly

A multi-tenant webhook and event delivery platform, built to answer a question: *how would you build scheduling and webhook infrastructure from scratch today, with the discipline of a team that could go broke tomorrow?*

The short version: battle-tested components, tenant isolation, simple abstractions, encryption, exceptional debuggability. Read [Why Hookly](docs/vision.md) for the full context — where this came from, what problems it is solving, and where it is going.

## What it does

Hookly exposes a REST API that lets platform operators and tenants:

- **Manage webhook endpoints** — create, update, suspend, activate, and delete delivery targets with per-endpoint HMAC signing secrets
- **Define and deliver events** — structured events flow through a Redis-backed tiered delivery queue to reach the right endpoints
- **Control access** — role-based access control (RBAC) with scoped permissions, assignable to both users and API keys
- **Issue and rotate credentials** — API keys with environment scoping, and JWT signing keys (RS256/ES256/ES384) with rotation grace periods
- **Observe platform changes** — a system-defined catalog of 27 platform event types that tenants can subscribe to, delivered via dedicated platform webhooks

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Clients                                                        │
│  (API consumers, dashboards)                                    │
└────────────────────┬────────────────────────────────────────────┘
                     │  HTTPS / Bearer token (API key)
                     ▼
┌────────────────────────────────────┐
│  hookly  (API server)              │
│  Axum 0.7 · Tokio                  │
│  Auth · RBAC · Idempotency         │
└──────┬─────────────────────────────┘
       │  INSERT events + delivery_jobs
       │  XADD Redis Stream (best-effort)
       ▼
┌──────────────────────┐      ┌─────────────────────────────────┐      ┌──────────────────────────┐
│  PostgreSQL          │◄─────│  hookly-worker                  │─────►│  Customer HTTP Endpoints │
│  (source of truth)   │      │  XREADGROUP · HMAC-SHA256 sign  │      │  POST /your-webhook      │
│  events              │      │  HTTP delivery · retry backoff  │      │  HMAC-SHA256 signature   │
│  delivery_jobs       │      │  XAUTOCLAIM recovery            │      │  verified by recipient   │
│  delivery_attempts   │      │  Outbox poller                  │      └──────────────────────────┘
└──────────────────────┘      └──────────────┬──────────────────┘
       ▲                                     │
       │  Fire: INSERT events                │  XADD
       │         + delivery_jobs             ▼
┌──────┴───────────────────┐   ┌─────────────────────────────────┐
│  hookly-scheduler        │   │  Redis Streams                  │
│  Shard ownership (NX)    │   │  hookly:q:tier:{tier}           │
│  ZRANGEBYSCORE tick      │   │  hookly:q:org:{org_id}          │
│  Fire locks (NX)         │   └─────────────────────────────────┘
│  Reconciliation (2 min)  │
└──────────────────────────┘
```

## Data model

The schema is organized into seven domains. Every table carries `tenant_id` so data from one tenant never touches another. The event delivery pipeline is at the center; multi-tenancy, identity, RBAC, credentials, scheduling, and platform webhooks build around it.

```
┌──────────────────────────────────────────────────────────────────────────┐
│  multi-tenancy                                                           │
│  organizations ──► tenants ──► applications                              │
│                       └──► environments                                  │
└─────────────────────────────┬────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼──────────────────────┐
        ▼                     ▼                       ▼
┌─────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
│ identity & teams    │ │ event delivery       │ │ scheduling           │
│ ──────────────────  │ │ ──────────────────── │ │ ──────────────────── │
│ users               │ │ event_types          │ │ schedules            │
│ teams               │ │ endpoints            │ │ schedule_endpoints   │
│ team_members        │ │ endpoint_secrets     │ │ schedule_executions  │
│ invites             │ │ events               │ └──────────────────────┘
│ tenant_members      │ │ delivery_jobs        │
└─────────────────────┘ │ delivery_attempts    │
                        └──────────────────────┘
┌─────────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
│ rbac                │ │ credentials          │ │ platform webhooks    │
│ ──────────────────  │ │ ──────────────────── │ │ ──────────────────── │
│ permissions         │ │ api_keys             │ │ platform_event_types │
│ roles               │ │ jwt_keys             │ │ platform_webhooks    │
│ role_permissions    │ └──────────────────────┘ │ platform_webhook_    │
│ user_roles          │                          │   subscriptions      │
│ user_permissions    │                          └──────────────────────┘
│ api_key_roles       │
│ api_key_permissions │
└─────────────────────┘
```

### Multi-tenancy

| Table | Purpose |
|---|---|
| `organizations` | Top-level account. Holds billing email, plan, and Stripe customer ID. One org can contain many tenants. |
| `tenants` | An isolated workspace within an org — the primary unit of data isolation. All other tables carry `tenant_id`. |
| `applications` | A logical grouping within a tenant (e.g. "production", "staging"). Events are published through an application; endpoints belong to one. |
| `environments` | Named runtime scopes within a tenant (e.g. `live`, `sandbox`). API keys are issued per environment so tenants can maintain separate credential sets without separate tenants. |

### Identity & Teams

| Table | Purpose |
|---|---|
| `users` | A person who belongs to a tenant. Stores credentials, status, and login metadata. Lives in the `identity` schema. |
| `teams` | A named group of users within a tenant. |
| `team_members` | Join table linking a user to a team. |
| `invites` | A time-limited, token-gated invitation to join a tenant at a given role. Transitions through `sent → opened → accepted` (or `expired / revoked`). |
| `tenant_members` | Created when an invite is accepted. Records the active membership of a user in a tenant. |

### Event Delivery

| Table | Purpose |
|---|---|
| `event_types` | Schema definition for a class of events (name, JSON schema, version). Events reference this at publish time. |
| `endpoints` | An HTTP URL that receives webhook deliveries. Subscribes to specific event types. Carries a rate limit. |
| `endpoint_secrets` | AES-256-GCM encrypted HMAC-SHA256 signing secret for an endpoint. Supports rotation grace periods — two secrets can be active simultaneously during a rollover. |
| `events` | Immutable record of something that happened. Created at publish time; never updated or deleted. |
| `delivery_jobs` | Mutable state for one `(event, endpoint)` delivery pair — tracks status, attempt count, and queue enqueue time. Serves as the outbox record. |
| `delivery_attempts` | Append-only log row for each HTTP call. Records HTTP status, response body, latency, and outcome. |

### Scheduling

| Table | Purpose |
|---|---|
| `schedules` | A cron expression + event type + payload that fires automatically. The scheduler binary reads `next_run_at` from a Redis sorted set, fires, then recomputes and re-scores the entry. |
| `schedule_endpoints` | Join table: which endpoints a schedule fans out to when it fires. |
| `schedule_executions` | One row per schedule fire. Records trigger time, completion time, and outcome for debuggability. |

### RBAC

| Table | Purpose |
|---|---|
| `permissions` | A `(resource, action)` pair. System permissions are seeded at startup; tenants can define custom ones. |
| `roles` | A named collection of permissions. Can be system-defined or tenant-owned. |
| `role_permissions` | Join table linking roles to permissions. |
| `user_roles` / `user_permissions` | Direct role and permission assignments to users, with optional expiry. |
| `api_key_roles` / `api_key_permissions` | Same as above, scoped to an API key rather than a user. |

### Credentials

| Table | Purpose |
|---|---|
| `api_keys` | Bearer token for API access. Scoped to an environment. Stored as a hash; the raw key is shown exactly once at creation. |
| `jwt_keys` | RS256, ES256, or ES384 key pair for JWT signing or webhook signature verification. Supports rotation with a grace period during which both old and new keys are valid. |

### Platform Webhooks

Platform webhooks are a system-level notification channel — separate from the tenant-level event delivery pipeline. They let tenants receive structured notifications when Hookly itself changes (keys rotated, users invited, endpoints created, etc.).

| Table | Purpose |
|---|---|
| `platform_event_types` | System-defined catalog of 27 event types covering API keys, users, roles, endpoints, environments, and JWT keys. Read-only; seeded at startup. |
| `platform_webhooks` | A tenant-owned HTTP endpoint that receives platform notifications. Has its own encrypted signing secret. |
| `platform_webhook_subscriptions` | Opt-in table: which platform event types a tenant's webhook is subscribed to. |

---

## Tech stack

| Layer | Choice |
|---|---|
| Language | Rust (edition 2021) |
| HTTP framework | Axum 0.7 |
| Database | PostgreSQL via SQLx 0.8 |
| Queue | Redis Streams |
| IDs | UUIDv7 (time-ordered) + NanoId prefixed public IDs |
| Encryption | AES-256-GCM (per-tenant derived keys) |
| Signing | HMAC-SHA256 for webhook payloads (Svix-compatible) |
| JWT key types | RS256, ES256, ES384 |
| Async runtime | Tokio |
| Observability | tracing + OpenTelemetry (OTLP/gRPC, opt-in) |

## Key design choices

- **Three binaries, independently scalable** — `hookly` (API), `worker` (delivery), `scheduler` (cron)
- **Outbox pattern** — delivery jobs survive Redis restarts; PostgreSQL is the durable source
- **Per-tenant encryption** — AES-256-GCM keys derived per tenant from a single master key
- **Circuit breaker** — per-endpoint state machine prevents a broken target from consuming worker capacity
- **Cursor pagination** — all list endpoints use opaque cursor tokens; safe at any table scale
- **No FK constraints** — referential integrity at the application layer; enables soft deletes and future sharding
- **API key auth** — all routes protected by bearer-token API keys; verify/accept invite flows are intentionally public

## Quick start

**With Docker Compose (recommended):**

```bash
cp .env.example .env        # fill in CRYPTO_MASTER_KEY and CRYPTO_API_KEY_ENCRYPTION_KEY
docker compose up -d        # starts PostgreSQL, Redis, hookly, worker, scheduler
```

**From source:**

```bash
# Prerequisites: Rust, PostgreSQL, Redis, sqlx-cli
make install   # installs sqlx-cli

cp .env.example .env        # configure DATABASE_URL, REDIS_URL, CRYPTO_* keys
make migrate                # run all migrations

make run                    # API server on :3000
cargo run --bin worker      # delivery worker
cargo run --bin scheduler   # cron scheduler
```

The API is available at `http://localhost:3000/api/v1`. Health check: `GET /api/health`.

**Optional — export telemetry (Jaeger, Grafana OTLP):**

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 \
OTEL_SERVICE_NAME=hookly \
cargo run --bin hookly
```

## Documentation

| Document | Description |
|---|---|
| [Why Hookly](docs/vision.md) | Motivation, north star, guiding principles, and roadmap |
| [Architecture overview](docs/architecture/overview.md) | System components and how they connect |
| [Data model](docs/architecture/data-model.md) | Database schema by domain |
| [Delivery pipeline](docs/architecture/delivery-pipeline.md) | Event → queue → endpoint flow |
| [Platform webhooks](docs/features/platform-webhooks.md) | Platform-level notification system |
| [RBAC](docs/features/rbac.md) | Roles, permissions, and assignments |
| [JWT keys](docs/features/jwt-keys.md) | Key generation, rotation, and JWKS |
| [API reference](docs/api-reference.md) | Endpoint index grouped by resource |
| [Contributing](docs/contributing.md) | Local setup, migrations, testing |
| [Decision log](docs/decisions/) | Architecture Decision Records |
