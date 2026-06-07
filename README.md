# Hookly

A multi-tenant webhook and event delivery platform, built to answer a question: *how would you build scheduling and webhook infrastructure from scratch today, with the discipline of a team that could go broke tomorrow?*

The short version: battle-tested components, two binaries, per-tenant encryption, cursor pagination, and every delivery attempt fully traceable from request ID to HTTP response. Read [Why Hookly](docs/vision.md) for the full context — where this came from, what problems it is solving, and where it is going.

## What it does

Hookly exposes a REST API that lets platform operators and tenants:

- **Manage webhook endpoints** — create, update, suspend, activate, and delete delivery targets with per-endpoint HMAC signing secrets
- **Define and deliver events** — structured events flow through a Redis-backed tiered delivery queue to reach the right endpoints
- **Control access** — role-based access control (RBAC) with scoped permissions, assignable to both users and API keys
- **Issue and rotate credentials** — API keys with environment scoping, and JWT signing keys (RS256/ES256/ES384) with rotation grace periods
- **Observe platform changes** — a system-defined catalog of 27 platform event types that tenants can subscribe to, delivered via dedicated platform webhooks

## Tech stack

| Layer | Choice |
|---|---|
| Language | Rust (edition 2021) |
| HTTP framework | Axum 0.7 |
| Database | PostgreSQL via SQLx 0.8 |
| Queue | Redis Streams |
| IDs | UUIDv7 (time-ordered) + NanoId prefixed public IDs |
| Encryption | AES-256-GCM (per-tenant derived keys) |
| Signing | HMAC-SHA256 for webhook payloads |
| JWT key types | RS256, ES256, ES384 |
| Async runtime | Tokio |

## Key design choices

- **No FK constraints** — referential integrity enforced at the application layer; enables cross-shard flexibility and simpler migrations
- **Cursor pagination** — all list endpoints use opaque cursor tokens rather than `OFFSET`, safe for high-cardinality tables
- **Two binaries** — `hookly` (API server) and `worker` (delivery processor) run independently and scale separately
- **Encrypted secrets at rest** — API key hashes and webhook signing secrets are encrypted under a per-tenant AES-256-GCM derived key

## Quick start

**Prerequisites:** Rust, PostgreSQL, Redis, `sqlx-cli`

```bash
# Install sqlx CLI
make install

# Configure environment
cp .env.example .env   # fill in DATABASE_URL, REDIS_URL, CRYPTO_MASTER_KEY, etc.

# Run migrations
make migrate

# Start the API server
make run

# In another terminal, start the delivery worker
cargo run --bin worker
```

The API is available at `http://localhost:3000/api/v1`. Health check: `GET /api/health`.

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
