# Platform webhooks

Platform webhooks let tenants receive HTTP notifications when administrative configuration changes occur on the Hookly platform — for example, when an API key is rotated, an environment is deleted, or a user's role is removed.

This is distinct from the tenant-facing webhook delivery system (which delivers events from tenant applications to their customers). Platform webhooks observe the platform itself.

---

## Concepts

### Platform event types

The platform defines a fixed catalog of 27 observable event types. Tenants cannot create or modify them. Each event type has:

- A stable `public_id` (e.g., `pet_apk_rotated`)
- A human-readable `name` in `<resource>.<action>` format (e.g., `api_key.rotated`)
- A `resource` and `action` for filtering

Full catalog by resource:

| Resource | Events |
|---|---|
| `api_key` | `created`, `updated`, `deleted`, `rotated` |
| `environment` | `created`, `updated`, `deleted`, `disabled` |
| `jwt_key` | `created`, `rotated`, `disabled`, `deleted` |
| `role` | `created`, `updated`, `deleted` |
| `user` | `invited`, `joined`, `deleted`, `role_assigned`, `role_removed` |
| `endpoint` | `created`, `updated`, `deleted`, `disabled` |
| `application` | `created`, `updated`, `deleted` |

### Platform webhooks

A platform webhook is a tenant-owned HTTPS endpoint that receives event notifications. Each webhook has:

- A name and optional description
- A target URL (must be `https://`)
- A signing secret for payload verification
- A status: `active`, `suspended`, or `disabled`
- Optional metadata (arbitrary JSON)

Max 10 webhooks per tenant (active + suspended; disabled webhooks don't count toward the limit).

### Subscriptions

A tenant subscribes globally to event types. When an event occurs, **all active webhooks for that tenant** receive a notification. Subscriptions are not per-webhook — if you want webhook A to receive `api_key.deleted` but not webhook B, manage this at the receiver side.

---

## API

### Webhook endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/v1/platform-webhooks` | Create a webhook |
| `GET` | `/api/v1/platform-webhooks` | List webhooks (filterable by `tenant_id`, `status`) |
| `GET` | `/api/v1/platform-webhooks/:id` | Get a single webhook |
| `PATCH` | `/api/v1/platform-webhooks/:id` | Update name / description / url / metadata |
| `DELETE` | `/api/v1/platform-webhooks/:id` | Soft delete |
| `POST` | `/api/v1/platform-webhooks/:id/suspend` | Suspend (pauses delivery) |
| `POST` | `/api/v1/platform-webhooks/:id/activate` | Activate |
| `POST` | `/api/v1/platform-webhooks/:id/rotate-secret` | Generate a new signing secret |

### Subscription endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/v1/platform-subscriptions?tenant_id=` | List subscriptions for a tenant |
| `POST` | `/api/v1/platform-subscriptions` | Add subscriptions (partial success on unknown IDs) |
| `PUT` | `/api/v1/platform-subscriptions` | Replace all subscriptions atomically |
| `DELETE` | `/api/v1/platform-subscriptions?tenant_id=&event_type_id=` | Remove one subscription |

### Event type endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/v1/platform-event-types` | List event types (filterable by `resource`) |
| `GET` | `/api/v1/platform-event-types/:id` | Get a single event type |

---

## Create webhook

```http
POST /api/v1/platform-webhooks
Content-Type: application/json

{
  "tenant_id": "...",
  "name": "prod-ops-listener",
  "url": "https://ops.example.com/hooks/hookly",
  "description": "Receives security-critical events",
  "metadata": { "team": "infra" }
}
```

Response `200 OK`:
```json
{
  "id": "pwh_aB3kL9mXzQ",
  "tenant_id": "...",
  "name": "prod-ops-listener",
  "url": "https://ops.example.com/hooks/hookly",
  "signing_secret": "whsec_dGhpcyBpcyBhIHRlc3Q...",
  "status": "active",
  "metadata": { "team": "infra" },
  "created_at": "2026-06-07T12:00:00Z",
  "updated_at": "2026-06-07T12:00:00Z"
}
```

**`signing_secret` is only returned on this response and on `rotate-secret`.** Store it immediately — it cannot be retrieved again.

---

## Subscribe to event types

```http
POST /api/v1/platform-subscriptions
Content-Type: application/json

{
  "tenant_id": "...",
  "event_type_ids": ["pet_apk_rotated", "pet_apk_deleted", "pet_usr_rol_rmvd"]
}
```

Response `200 OK`:
```json
{
  "tenant_id": "...",
  "subscribed": 3,
  "already_present": 0,
  "invalid_event_type_ids": []
}
```

Unknown event type IDs do not cause a failure — they are returned in `invalid_event_type_ids` and valid IDs are still subscribed.

---

## Signing secret lifecycle

```
Create webhook        → signing_secret returned once
       ↓
Normal operation      → signing_secret absent from all GET responses
       ↓
rotate-secret         → new signing_secret returned once, old secret immediately invalid
```

Rotation replaces the secret atomically. There is no dual-secret grace period — if you need zero-downtime rotation, keep the old webhook active while transitioning receivers to a new webhook.

---

## Webhook status lifecycle

```
         create
           ↓
        [active] ←──────────────┐
           │                     │
        suspend               activate
           │                     │
       [suspended] ─────────────┘
           │
         (admin) disable
           │
        [disabled]  (terminal — cannot be re-activated)
```

Suspended webhooks do not receive deliveries. The count toward the per-tenant limit continues while suspended. `disabled` is a terminal state (used for soft-delete semantics) and does not count toward the limit.

---

## Payload signature verification

Every delivery includes an `X-Hookly-Signature` header:

```
X-Hookly-Signature: sha256=<hex_hmac_sha256>
```

Computed as `HMAC-SHA256(signing_secret_raw_bytes, request_body_bytes)`.

To verify in Python:
```python
import hmac, hashlib, base64

def verify(body: bytes, header: str, whsec: str) -> bool:
    key = base64.urlsafe_b64decode(whsec[len("whsec_"):] + "==")
    expected = "sha256=" + hmac.new(key, body, hashlib.sha256).hexdigest()
    return hmac.compare_digest(expected, header)
```

Always use a constant-time comparison (`hmac.compare_digest` / `crypto.timingSafeEqual`) to prevent timing attacks.
