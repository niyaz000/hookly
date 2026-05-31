# Envoy Sidecar Authentication

How to put Envoy proxy in front of the Hookly app as a sidecar to handle JWT, API key, and username/password authentication.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   DOCKER COMPOSE / K8S POD                  │
│                                                             │
│  INTERNET                                                   │
│     │                                                       │
│     │  :8080 (only public port)                            │
│     ▼                                                       │
│  ┌──────────────────────┐                                   │
│  │    ENVOY PROXY       │  ◄── All traffic enters here      │
│  │    (sidecar, :8080)  │                                   │
│  │                      │                                   │
│  │  Filter chain:       │                                   │
│  │  1. ext_authz ──────────────────────────┐               │
│  │  2. router           │                  │               │
│  └──────────┬───────────┘                  │               │
│             │ (after auth passes)           │               │
│             │ forward + inject headers      │               │
│             ▼                              ▼               │
│  ┌──────────────────┐         ┌──────────────────────┐     │
│  │   HOOKLY APP     │         │   AUTH SERVICE       │     │
│  │  (Rust, :3000)   │         │  (Rust, :4000)       │     │
│  │                  │         │                      │     │
│  │  internal only   │         │  /auth/check  ◄──────┘     │
│  │  not exposed     │         │  /auth/login  (public)     │
│  │                  │         │  /auth/keys               │
│  └──────────────────┘         └──────────┬───────────┘     │
│                                          │                 │
│                               ┌──────────┴──────────┐      │
│                               ▼                     ▼      │
│                         ┌──────────┐         ┌──────────┐  │
│                         │ POSTGRES │         │  REDIS   │  │
│                         │  :5432   │         │  :6379   │  │
│                         └──────────┘         └──────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**Port assignments:**

| Port | Service | Exposed? |
|------|---------|----------|
| 8080 | Envoy listener | Yes (public) |
| 9901 | Envoy admin UI | Optional (dev only) |
| 4000 | Auth service | No (internal network) |
| 3000 | Hookly app | No (internal network) |
| 5432 | PostgreSQL | No |
| 6379 | Redis | No |

---

## How Envoy Enforces Auth

Envoy uses the **`ext_authz`** HTTP filter. Before forwarding any request to the app, it calls the auth service at `/auth/check`. The auth service returns 200 (allow) or 401/403 (deny).

```
Client Request
     │
     ▼
Envoy receives request
     │
     ├── Is path /api/health or /auth/login? ──► YES ──► Forward directly (no auth)
     │
     ▼ NO
     │
Envoy calls  POST auth-service:4000/auth/check
  (passes along: Authorization header, X-Api-Key header)
     │
     ├── Auth Service returns 401/403 ──────────────────► Envoy returns error to client
     │
     ▼ Auth Service returns 200
     │
Envoy strips any client-sent X-User-Id, X-Tenant-Id    ◄── prevents header spoofing
Envoy injects auth service response headers into the request
     │
     ▼
Hookly App receives request with:
  X-User-Id:      <uuid>
  X-Tenant-Id:    <uuid>
  X-Org-Id:       <uuid>
  X-Auth-Method:  jwt | api_key | basic
```

---

## Auth Service: Three Methods

The auth service inspects the incoming headers and picks the right validation path:

```
Incoming headers to /auth/check
          │
          ├── X-Api-Key: <key> present?
          │        │
          │        ▼
          │   Hash key with SHA-256
          │   Check Redis cache (TTL 5 min)
          │   On miss → query api_keys table in Postgres
          │   Return user identity or 401
          │
          ├── Authorization: Bearer <token>?
          │        │
          │        ▼
          │   Decode JWT using shared secret (HS256)
          │   Verify signature + expiry + issuer
          │   Extract user_id, tenant_id, org_id from claims
          │   Return identity or 401   (no DB call needed)
          │
          └── Authorization: Basic <base64>?
                   │
                   ▼
              Decode base64 → email:password
              Look up user by email in Postgres
              Verify password with Argon2
              Return identity or 401
```

---

## JWT: Login Flow

Clients exchange email+password for a JWT once, then use the JWT on all subsequent calls.

```
Client                    Envoy               Auth Service         Postgres
  │                         │                      │                  │
  │─── POST /auth/login ───►│                      │                  │
  │    { email, password }  │  (auth bypassed)     │                  │
  │                         │──────────────────────►                  │
  │                         │                      │─── SELECT user ─►│
  │                         │                      │◄── user row ─────│
  │                         │                      │  verify Argon2   │
  │                         │                      │  sign JWT        │
  │                         │◄─────────────────────│                  │
  │◄────────────────────────│  { token: "eyJ..." } │                  │
  │                         │                      │                  │
  │  (all subsequent calls) │                      │                  │
  │─── GET /api/v1/applications/:id ──────────────►│                  │
  │    Authorization: Bearer eyJ...                │                  │
  │                         │── POST /auth/check ─►│                  │
  │                         │                      │  decode JWT      │
  │                         │◄── 200 + X-User-Id ──│  (no DB call)    │
  │                         │──► hookly app        │                  │
  │◄── 200 { application }──│                      │                  │
```

---

## Component Responsibilities

| Component | Responsibility |
|-----------|---------------|
| **Envoy** | Traffic gating, calls auth service before every request, strips/injects identity headers, bypasses auth for public routes |
| **Auth Service** | Validates JWT / API key / Basic auth credentials, issues JWTs on login, manages API key lifecycle |
| **Hookly App** | Business logic only — trusts `X-User-Id` / `X-Tenant-Id` headers; never sees unauthenticated requests |
| **Postgres** | Stores users (with Argon2 password hash), api_keys table |
| **Redis** | Caches API key lookups (~5 min TTL) to avoid a DB hit on every request |

---

## Database Changes Needed

### Add `password_hash` to users

```sql
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS password_hash TEXT;
-- Nullable: existing users have no password until set.
-- Basic auth and login reject users without a password_hash.
```

### New `api_keys` table

```sql
CREATE TABLE api_keys (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    key_hash     TEXT NOT NULL UNIQUE,  -- SHA-256 hex of raw key (never store plaintext)
    prefix       VARCHAR(8) NOT NULL,  -- first 8 chars for display, e.g. "hk_ab12cd"
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at   TIMESTAMPTZ,          -- NULL = never expires
    revoked_at   TIMESTAMPTZ,          -- NULL = active
    last_used_at TIMESTAMPTZ
);
```

API keys work like GitHub personal access tokens: the raw key is shown once at creation and never stored. Only the SHA-256 hash lives in the DB.

---

## Envoy Config Sketch

```yaml
http_filters:
  - name: envoy.filters.http.ext_authz
    typed_config:
      http_service:
        server_uri:
          uri: http://auth-service:4000
          timeout: 5s
        authorization_request:
          allowed_headers:   # forward these to the auth service
            - authorization
            - x-api-key
        authorization_response:
          allowed_upstream_headers:  # inject these into the app request after 200
            - x-user-id
            - x-tenant-id
            - x-org-id
            - x-auth-method

routes:
  # Public — auth bypassed
  - match: { path: /api/health }
    typed_per_filter_config:
      envoy.filters.http.ext_authz:
        disabled: true

  - match: { path: /auth/login }
    typed_per_filter_config:
      envoy.filters.http.ext_authz:
        disabled: true

  # Everything else requires auth
  - match: { prefix: /api/ }
    route: { cluster: hookly_app }

# Strip client-sent identity headers before routing (prevents spoofing)
request_headers_to_remove:
  - x-user-id
  - x-tenant-id
  - x-org-id
  - x-auth-method
```

---

## Key Design Decisions

**Why `ext_authz` for all three methods instead of Envoy's native `jwt_authn`?**
Envoy's built-in JWT filter validates signatures but cannot extract claims into custom headers (like `X-User-Id`) without additional Lua scripting. A single auth service handles all three methods consistently and produces the same normalized identity headers regardless of how the client authenticated.

**Why a separate auth service and not middleware in the app?**
The app never sees unauthenticated traffic — Envoy rejects it before it arrives. App code stays clean with no auth middleware. The auth service can also be deployed, scaled, and upgraded independently.

**Why hash API keys with SHA-256?**
Only the hash is stored in Postgres, so a DB breach doesn't expose live keys. The raw key is shown to the user once at creation time — same model as GitHub, npm tokens, etc.

**Why Redis for API key caching?**
API key validation requires a DB lookup on every request (unlike JWT which is self-contained). Caching in Redis with a short TTL keeps latency low while still allowing near-real-time revocation (revoked keys fall out of cache within 5 minutes).

**How does the app know who the user is?**
Envoy injects `X-User-Id`, `X-Tenant-Id`, `X-Org-Id`, and `X-Auth-Method` headers into every request it forwards to the app. The app reads these headers instead of doing its own auth. Envoy strips any client-sent versions of these headers on ingress to prevent spoofing.
