# JWT keys

Hookly allows tenants to manage their own JWT signing keys. This enables tenants to issue JWTs signed with keys that Hookly verifies — for example, authenticating webhook payloads or securing API-to-API calls within the tenant's own system.

---

## Key types

| Type | Algorithm | Notes |
|---|---|---|
| `rs256` | RSA-PKCS1 2048-bit + SHA-256 | Widest compatibility |
| `es256` | ECDSA P-256 + SHA-256 | Smaller signatures, faster verification |
| `es384` | ECDSA P-384 + SHA-384 | Higher security margin |

Keys are generated server-side using the `p256`, `p384`, and `rsa` crates. The private key is stored **encrypted at rest** (AES-256-GCM, same `TenantCrypto` pattern as webhook signing secrets). The public key is stored in PEM format for easy export.

---

## Key lifecycle

```
         create
           ↓
        [active]  ── rotate ──→ [rotating]  ── grace period ends ──→ [disabled]
           │                        │
        delete                   (old key still verifies during grace)
           │
        [disabled]
```

### Rotation with grace period

When a key is rotated:
1. The current `active` key transitions to `rotating` status
2. A new `active` key is generated and returned
3. The `rotating` key remains usable for verification until `grace_period_ends_at`
4. A background task (hourly) calls `expire_grace_period_keys()` to disable keys past their grace period

This ensures a zero-downtime rotation: JWTs signed with the old key continue to verify for the duration of the grace period while systems migrate to the new key.

---

## JWKS endpoint

Active and rotating keys are exposed via the JWKS (JSON Web Key Set) endpoint:

```http
GET /api/v1/jwt-keys/jwks?tenant_id=<uuid>
```

Response:
```json
{
  "keys": [
    {
      "kty": "EC",
      "kid": "jwk_aB3kL9...",
      "use": "sig",
      "alg": "ES256",
      "crv": "P-256",
      "x": "...",
      "y": "..."
    }
  ]
}
```

The JWKS response includes all keys in `active` or `rotating` status for the tenant. Consumers of the tenant's JWTs should fetch the JWKS periodically (or on `kid` miss) to stay current.

---

## API

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/v1/jwt-keys` | Generate a new key pair |
| `GET` | `/api/v1/jwt-keys` | List keys for a tenant |
| `GET` | `/api/v1/jwt-keys/:id` | Get a single key (public key only) |
| `POST` | `/api/v1/jwt-keys/:id/rotate` | Rotate — new key created, old enters grace period |
| `DELETE` | `/api/v1/jwt-keys/:id` | Disable immediately (no grace period) |
| `GET` | `/api/v1/jwt-keys/jwks` | JWKS endpoint (public keys only) |

---

## Create a key

```http
POST /api/v1/jwt-keys
{
  "tenant_id": "...",
  "key_type": "es256",
  "grace_period_seconds": 86400
}
```

Response:
```json
{
  "id": "jwk_xYz...",
  "tenant_id": "...",
  "key_type": "es256",
  "status": "active",
  "public_key_pem": "-----BEGIN PUBLIC KEY-----\n...",
  "created_at": "2026-06-07T12:00:00Z"
}
```

The private key is never returned. It is stored encrypted in the database and used internally by the signing service.

---

## Design notes

- **Server-side generation only.** Tenants cannot import their own private keys. This simplifies the security model — Hookly is the single source of truth for the private key material.
- **Separate `grace_period_ends_at` per key.** Grace periods are set at rotation time, not at key creation. Different keys can have different grace durations.
- **Background expiry, not real-time.** The grace period expiry check runs hourly. A key's grace period may be exceeded by up to 1 hour before it is disabled. This is acceptable — the grace period is a "safe overlap" window, not a hard deadline.
- **JWKS `kid` matches `public_id`.** Consumers can use the `kid` claim in a JWT to look up the specific key without fetching all keys.
