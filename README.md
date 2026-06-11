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
│  Clients                                                         │
│  (API consumers, dashboards)                                     │
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
┌──────────────────────┐      ┌─────────────────────────────────┐
│  PostgreSQL          │◄─────│  hookly-worker                  │
│  (source of truth)   │      │  XREADGROUP · HMAC-SHA256 sign  │
│  events              │      │  HTTP delivery · retry backoff  │
│  delivery_jobs       │      │  XAUTOCLAIM recovery            │
│  delivery_attempts   │      │  Outbox poller                  │
└──────────────────────┘      └──────────────┬──────────────────┘
       ▲                                      │
       │  Fire: INSERT events                 │  XADD
       │         + delivery_jobs              ▼
┌──────┴───────────────────┐   ┌─────────────────────────────────┐
│  hookly-scheduler        │   │  Redis Streams                  │
│  Shard ownership (NX)    │   │  hookly:q:tier:{tier}           │
│  ZRANGEBYSCORE tick      │   │  hookly:q:org:{org_id}          │
│  Fire locks (NX)         │   └─────────────────────────────────┘
│  Reconciliation (2 min)  │
└──────────────────────────┘
```

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
