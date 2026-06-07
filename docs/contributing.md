# Contributing

## Prerequisites

- Rust (stable, edition 2021)
- PostgreSQL 14+
- Redis 7+
- [`sqlx-cli`](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli)

Install `sqlx-cli`:

```bash
make install
# equivalent to: cargo install sqlx-cli --no-default-features --features postgres
```

## Environment setup

Copy the example env file and fill in values:

```bash
cp .env.example .env
```

Required variables:

| Variable | Description |
|---|---|
| `DATABASE_URL` | PostgreSQL connection string, e.g. `postgres://user:pass@localhost/hookly` |
| `REDIS_URL` | Redis connection string, e.g. `redis://127.0.0.1:6379` |
| `CRYPTO_MASTER_KEY` | Base64-encoded 32-byte key used to derive per-tenant AES-256-GCM keys |
| `CRYPTO_API_KEY_ENCRYPTION_KEY` | Base64-encoded 32-byte key for encrypting API key hashes |
| `SERVER_HOST` | Bind host, default `127.0.0.1` |
| `SERVER_PORT` | Bind port, default `3000` |

Generate random keys:

```bash
openssl rand -base64 32   # use output for CRYPTO_MASTER_KEY and CRYPTO_API_KEY_ENCRYPTION_KEY
```

## Running migrations

```bash
make migrate
# equivalent to: sqlx migrate run
```

Migrations live in `migrations/` and are numbered sequentially. SQLx tracks applied migrations in a `_sqlx_migrations` table. Never edit an already-applied migration; always add a new one.

## Starting the server

```bash
make run
# equivalent to: cargo run --bin hookly

# Set log level
RUST_LOG=debug cargo run --bin hookly
```

The API server starts on `http://localhost:3000`. Verify:

```bash
curl http://localhost:3000/api/health
# → OK
```

## Starting the delivery worker

```bash
cargo run --bin worker
```

The worker consumes from the Redis delivery streams and dispatches webhook payloads. It runs independently from the API server and can be scaled horizontally.

## Running tests

```bash
make test
# equivalent to: cargo test
```

Integration tests live in `tests/` and require a running PostgreSQL instance (they create and tear down their own data). The `DATABASE_URL` env var must be set.

## Code style

```bash
make fmt     # format all code
make lint    # fmt check + clippy (errors on warnings)
```

Clippy is configured with `-D warnings` — all warnings are errors. Fix them before committing.

## Project layout

```
src/
├── main.rs                  # API server entry point
├── worker/main.rs           # Delivery worker entry point
├── config.rs                # Environment variable loading
├── state.rs                 # AppState (shared across all handlers)
├── router.rs                # Route registration and middleware stack
├── error.rs                 # AppError + IntoResponse impl
├── common/                  # Shared utilities
│   ├── crypto.rs            # TenantCrypto (AES-256-GCM)
│   ├── key_provider.rs      # KeyProvider trait + EnvKeyProvider
│   ├── nano_id.rs           # NanoId wrapper
│   ├── types.rs             # ValidatedJson extractor, RequestContext
│   ├── validators.rs        # Common field validators
│   └── ...
└── features/                # One module per domain
    ├── applications/
    ├── api_keys/
    ├── endpoints/
    ├── events/
    ├── jwt_keys/
    ├── platform_event_types/
    ├── platform_webhooks/
    ├── platform_subscriptions/
    ├── rbac/  (permissions, roles, assignments)
    └── ...
```

Each feature module follows the same structure:

```
features/<name>/
├── mod.rs
├── models.rs      # DB structs, request/response types, validation
├── repository.rs  # SQL queries via SQLx
├── handlers.rs    # Axum handler functions
├── routes.rs      # Route registration with SetHandlerName layers
└── service.rs     # (where business logic spans multiple repos)
```

## Adding a new feature

1. Create `src/features/<name>/` with the above structure
2. Add `pub mod <name>;` to `src/features/mod.rs`
3. Merge routes in `src/router.rs` inside `v1_routes()`
4. Write a migration in `migrations/` with the next sequence number
5. Run `make migrate` and `make lint`

## ID conventions

| Type | Format | Example |
|---|---|---|
| Internal DB primary key | UUIDv7 (time-ordered) | `019123...` |
| External / API-facing ID | `<prefix>_<NanoId>` | `pwh_aB3kL9mXz` |

Public ID prefixes used across the codebase:

| Prefix | Resource |
|---|---|
| `pwh_` | Platform webhook |
| `pet_` | Platform event type |
| `apk_` | API key |
| `env_` | Environment |
| `rol_` | Role |
| `jwk_` | JWT key |
