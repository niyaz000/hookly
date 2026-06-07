# ADR-003: Per-tenant AES-256-GCM encrypted signing secrets

## Status
Accepted

## Context

Each platform webhook endpoint has a signing secret used to produce an HMAC-SHA256 signature over the delivered payload. Tenants use this signature to verify that a delivery originated from Hookly and was not tampered with.

The signing secret must be:
1. **Unguessable** — generated from a CSPRNG
2. **Stored durably** — needed every time a payload is delivered
3. **Protected at rest** — a database dump should not expose usable secrets
4. **Revocable** — tenants can rotate the secret at any time

Options for storage:

| Approach | Notes |
|---|---|
| Plaintext in DB | Simplest; unacceptable — a DB dump exposes all secrets |
| Hashed in DB | Cannot be used for signing; works for API key verification but not signing secrets |
| Single global encryption key | Simpler; a single compromised key exposes all secrets |
| **Per-tenant derived key** | More complex; blast radius of a compromised tenant key is bounded to that tenant |

## Decision

Signing secrets are encrypted using AES-256-GCM with a **per-tenant derived key**. The `TenantCrypto` struct (in `src/common/crypto.rs`) derives a tenant-specific encryption key from a master key using HKDF, keyed by the tenant's UUID. The encrypted ciphertext (with nonce) is stored as a Base64-encoded string in the `signing_secret_enc` column.

```
derive_key(master_key, tenant_id) → tenant_key
AES-256-GCM(tenant_key, nonce, plaintext) → ciphertext
store: base64(nonce || ciphertext)
```

The raw secret is generated as `whsec_<base64url(32 bytes)>` and is **only returned once**: on create and on explicit rotate-secret. Subsequent GET requests return the webhook without the secret field (`#[serde(skip_serializing_if = "Option::is_none")]`).

## Consequences

**Positive:**
- A database dump reveals no usable secrets
- Compromise of one tenant's derived key does not expose other tenants' secrets
- Secret rotation is a single UPDATE — no cascading changes needed
- The `whsec_` prefix makes secrets machine-identifiable in logs and config

**Negative:**
- `CRYPTO_MASTER_KEY` becomes an extremely sensitive secret that must be protected in key management (HSM, Vault, AWS KMS envelope encryption recommended in production)
- Key derivation adds a small overhead to every secret generation and decryption call
- The `TenantCrypto` struct must be available in AppState for the delivery worker — workers must have the same master key as the API server
