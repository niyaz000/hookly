# API reference

Base URL: `http://localhost:3000/api/v1`

All request bodies are JSON. All responses are JSON. Errors follow the format:

```json
{
  "error": "not_found",
  "message": "...",
  "request_id": "019...",
  "fields": null
}
```

Validation errors include a `fields` array:
```json
{
  "error": "validation_error",
  "fields": [
    { "field": "url", "code": "invalid_format", "message": "url must use HTTPS" }
  ]
}
```

---

## Health

| Method | Path | Response |
|---|---|---|
| `GET` | `/api/health` | `200 OK` — plain `"OK"` |

---

## Organizations

| Method | Path | Description |
|---|---|---|
| `POST` | `/organizations` | Create an organization |
| `GET` | `/organizations` | List organizations |
| `GET` | `/organizations/:id` | Get an organization |
| `PATCH` | `/organizations/:id` | Update name |
| `DELETE` | `/organizations/:id` | Delete |

---

## Tenants

| Method | Path | Description |
|---|---|---|
| `POST` | `/tenants` | Create a tenant |
| `GET` | `/tenants` | List tenants |
| `GET` | `/tenants/:id` | Get a tenant |
| `PATCH` | `/tenants/:id` | Update |
| `DELETE` | `/tenants/:id` | Delete |

---

## Users

| Method | Path | Description |
|---|---|---|
| `POST` | `/users` | Create a user |
| `GET` | `/users` | List users |
| `GET` | `/users/:id` | Get a user |
| `PATCH` | `/users/:id` | Update |
| `DELETE` | `/users/:id` | Delete |

---

## Teams

| Method | Path | Description |
|---|---|---|
| `POST` | `/teams` | Create a team |
| `GET` | `/teams` | List teams |
| `GET` | `/teams/:id` | Get a team |
| `PATCH` | `/teams/:id` | Update |
| `DELETE` | `/teams/:id` | Delete |

---

## Invites

| Method | Path | Description |
|---|---|---|
| `POST` | `/invites` | Send an invite |
| `GET` | `/invites` | List invites |
| `GET` | `/invites/:id` | Get an invite |
| `POST` | `/invites/:id/accept` | Accept an invite |
| `DELETE` | `/invites/:id` | Revoke |

---

## Applications

| Method | Path | Description |
|---|---|---|
| `POST` | `/applications` | Create an application |
| `GET` | `/applications` | List |
| `GET` | `/applications/:id` | Get |
| `PATCH` | `/applications/:id` | Update |
| `DELETE` | `/applications/:id` | Soft delete |
| `POST` | `/applications/:id/restore` | Restore a soft-deleted application |

---

## Event types

| Method | Path | Description |
|---|---|---|
| `POST` | `/event-types` | Create an event type |
| `GET` | `/event-types` | List (filterable by `application_id`, `tags`) |
| `GET` | `/event-types/:id` | Get |
| `PATCH` | `/event-types/:id` | Update |
| `DELETE` | `/event-types/:id` | Delete |

---

## Endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/endpoints` | Create a webhook endpoint |
| `GET` | `/endpoints` | List |
| `GET` | `/endpoints/:id` | Get |
| `PATCH` | `/endpoints/:id` | Update |
| `DELETE` | `/endpoints/:id` | Soft delete |
| `POST` | `/endpoints/:id/suspend` | Suspend delivery |
| `POST` | `/endpoints/:id/activate` | Resume delivery |

---

## Events

| Method | Path | Description |
|---|---|---|
| `POST` | `/events` | Submit an event |
| `GET` | `/events` | List events |
| `GET` | `/events/:id` | Get an event with delivery status |

---

## Schedules

| Method | Path | Description |
|---|---|---|
| `POST` | `/schedules` | Create a schedule |
| `GET` | `/schedules` | List |
| `GET` | `/schedules/:id` | Get |
| `PATCH` | `/schedules/:id` | Update |
| `DELETE` | `/schedules/:id` | Delete |

---

## API keys

| Method | Path | Description |
|---|---|---|
| `POST` | `/api-keys` | Create an API key (raw key returned once) |
| `GET` | `/api-keys` | List |
| `GET` | `/api-keys/:id` | Get (no raw key) |
| `PATCH` | `/api-keys/:id` | Update name / expiry / scopes |
| `DELETE` | `/api-keys/:id` | Revoke |
| `POST` | `/api-keys/:id/rotate` | Generate a new key value |

---

## Environments

| Method | Path | Description |
|---|---|---|
| `POST` | `/environments` | Create an environment |
| `GET` | `/environments` | List |
| `GET` | `/environments/:id` | Get |
| `PATCH` | `/environments/:id` | Update |
| `DELETE` | `/environments/:id` | Disable |

---

## Permissions

| Method | Path | Description |
|---|---|---|
| `GET` | `/permissions` | List all system permissions |
| `GET` | `/permissions/:id` | Get a permission |

---

## Roles

| Method | Path | Description |
|---|---|---|
| `POST` | `/roles` | Create a custom role |
| `GET` | `/roles` | List (filter by `is_system`) |
| `GET` | `/roles/:id` | Get with permissions |
| `PATCH` | `/roles/:id` | Update name / permissions |
| `DELETE` | `/roles/:id` | Delete (fails if active assignments exist) |

---

## Assignments

| Method | Path | Description |
|---|---|---|
| `POST` | `/assignments` | Create an assignment |
| `GET` | `/assignments` | List (filter by principal, role, scope) |
| `DELETE` | `/assignments/:id` | Revoke |

---

## JWT keys

| Method | Path | Description |
|---|---|---|
| `POST` | `/jwt-keys` | Generate a key pair |
| `GET` | `/jwt-keys` | List |
| `GET` | `/jwt-keys/:id` | Get (public key only) |
| `POST` | `/jwt-keys/:id/rotate` | Rotate with grace period |
| `DELETE` | `/jwt-keys/:id` | Disable immediately |
| `GET` | `/jwt-keys/jwks` | JWKS (public keys, active + rotating) |

---

## Platform event types

| Method | Path | Description |
|---|---|---|
| `GET` | `/platform-event-types` | List (filter by `resource`; cursor paginated) |
| `GET` | `/platform-event-types/:id` | Get a single event type |

---

## Platform webhooks

| Method | Path | Description |
|---|---|---|
| `POST` | `/platform-webhooks` | Create (signing secret returned once) |
| `GET` | `/platform-webhooks` | List (filter by `tenant_id`, `status`) |
| `GET` | `/platform-webhooks/:id` | Get |
| `PATCH` | `/platform-webhooks/:id` | Update |
| `DELETE` | `/platform-webhooks/:id` | Soft delete |
| `POST` | `/platform-webhooks/:id/suspend` | Suspend |
| `POST` | `/platform-webhooks/:id/activate` | Activate |
| `POST` | `/platform-webhooks/:id/rotate-secret` | New signing secret (returned once) |

---

## Platform subscriptions

| Method | Path | Description |
|---|---|---|
| `GET` | `/platform-subscriptions?tenant_id=` | List subscriptions for a tenant |
| `POST` | `/platform-subscriptions` | Add subscriptions (partial success) |
| `PUT` | `/platform-subscriptions` | Replace all subscriptions atomically |
| `DELETE` | `/platform-subscriptions?tenant_id=&event_type_id=` | Unsubscribe one event type |

---

## Pagination

All list endpoints support cursor-based pagination:

| Query param | Default | Max | Description |
|---|---|---|---|
| `limit` | `20` | `100` | Number of items per page |
| `cursor` | — | — | Opaque cursor from previous response's `next_cursor` |

Response shape:
```json
{
  "items": [...],
  "next_cursor": "pwh_aB3kL9mXz",
  "limit": 20
}
```

`next_cursor` is `null` on the last page.
