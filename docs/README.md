# Hookly — Documentation

## Start here

- [Why Hookly](vision.md) — motivation, north star, guiding principles, and phased roadmap. Read this first.

## Navigation

### Architecture
- [System overview](architecture/overview.md) — components, runtime topology, request lifecycle
- [Data model](architecture/data-model.md) — schema by domain with design notes
- [Delivery pipeline](architecture/delivery-pipeline.md) — event emission → Redis queue → endpoint delivery

### Features
- [Platform webhooks](features/platform-webhooks.md) — platform event catalog, subscriptions, signing
- [RBAC](features/rbac.md) — permissions, roles, user and API key assignments
- [JWT keys](features/jwt-keys.md) — key generation, rotation, grace periods, JWKS endpoint

### Reference
- [API reference](api-reference.md) — all endpoints grouped by resource
- [Contributing](contributing.md) — local setup, environment variables, running tests

### Decision Log
Architecture Decision Records capture the *why* behind non-obvious choices, grouped by concern.

See the [full decision index](decisions/README.md) for all accepted and planned records.

| Area | Accepted decisions |
|---|---|
| [api-design](decisions/api-design/) | Cursor-based pagination |
| [database](decisions/database/) | No FK constraints |
| [delivery](decisions/delivery/) | Redis Streams queue · Platform webhooks design |
| [security](decisions/security/) | Per-tenant signing secrets · RBAC model |
| [architecture](decisions/architecture/) | *(planned)* |
| [auditing](decisions/auditing/) | *(planned)* |
| [language](decisions/language/) | *(planned)* |
| [logging](decisions/logging/) | *(planned)* |
| [multi-tenancy](decisions/multi-tenancy/) | *(planned)* |
| [observability](decisions/observability/) | *(planned)* |
