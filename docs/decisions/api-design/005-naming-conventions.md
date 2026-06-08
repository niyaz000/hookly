# ADR api-design/005: Naming conventions, URL structure, and request limits

## Status
Accepted

## Context

Consistency in naming and URL structure is not aesthetic — it is the difference between an API a developer can navigate by intuition and one they have to look up for every operation. Inconsistent casing, deep URL nesting, and unpredictable request sizes all create friction. This ADR establishes the rules that apply uniformly across all endpoints.

## Decision

### JSON field naming: `snake_case`

All JSON request and response fields use `snake_case`:
```json
{
  "public_id": "app_Xk9...",
  "created_at": "2026-06-07T10:00:00Z",
  "updated_by": "usr_Yq2..."
}
```

This applies to: resource fields, error fields, metadata fields, and query parameters.

### URL path segments: `kebab-case`, plural nouns for collections

```
/api/v1/event-types          ✓
/api/v1/eventTypes           ✗
/api/v1/event_types          ✗
/api/v1/event-type           ✗  (singular)
```

Resource identifiers in path segments use the `public_id` value:
```
/api/v1/applications/app_Xk9mN4...
```

### URL structure

```
/api/v{n}/{resource}
/api/v{n}/{resource}/{id}
/api/v{n}/{resource}/{id}/{action}
```

**Actions** (sub-resources that mutate state but aren't resources themselves):
```
POST /api/v1/applications/app_Xk9.../restore
POST /api/v1/jwt-keys/jwk_Yq2.../rotate
```

**No deep nesting.** URLs must not nest more than one resource level deep:
```
/api/v1/applications/app_Xk9.../event-types    ✓  (scoped list)
/api/v1/orgs/org_.../tenants/ten_...            ✗  (two levels — use /tenants?org_id=... instead)
```

Deep nesting produces long, fragile URLs and forces clients to track parent IDs at every level. Scoping is expressed via query parameters or request body fields when needed.

### Request ID: `X-Request-Id` on all responses

Every response — success and error alike — carries an `X-Request-Id` header containing the request's UUIDv7:

```
X-Request-Id: 0190e3f4-3c7e-7b5a-bc2a-4d8e9f0a1b2c
```

On error responses, the same value also appears as `request_id` in the JSON body for clients that consume only the body. The two values are always identical for a given request.

### URI length limit: 512 characters

Requests with a URI exceeding 512 characters (path + query string) are rejected with `414 URI Too Long`. This is stricter than the HTTP standard (8 KB) and tighter than Microsoft's guideline (2 083 characters). The rationale:

- 512 characters is sufficient for any legitimate API call given flat URL structure
- Long URIs are a reliable indicator of a client bug (e.g., accidentally encoding a full payload in the query string)
- Shorter limits reduce scan surface for path-based injection attempts

### Request body size limit: 256 KB

Request bodies exceeding 256 KB are rejected with `413 Payload Too Large`. Legitimate webhook configurations and event payloads fit well within this bound. The limit prevents memory pressure on the Axum worker under burst conditions without requiring streaming buffering logic.

### Summary of naming rules

| Location | Convention | Example |
|---|---|---|
| JSON fields | `snake_case` | `created_at`, `event_type_id` |
| Query parameters | `snake_case` | `sort_by`, `sort_order` |
| URL path segments | `kebab-case` | `/event-types`, `/jwt-keys` |
| HTTP response headers | `Title-Kebab-Case` | `X-Request-Id`, `Deprecation` |
| Error codes | `snake_case` | `validation_error`, `not_found` |
| Field error codes | `snake_case` | `required`, `max_length`, `invalid_format` |

## Principles upheld

- **Developer experience** — `snake_case` matches PostgreSQL column names, Go and Rust struct field conventions, and Python variable names; no cognitive switching when reading logs or mapping to database rows
- **Two-person operations ceiling** — predictable URL shapes let any engineer guess the endpoint for a resource without reading the route table; no tribal knowledge needed
- **Frugality** — 512-char URI limit and 256 KB body limit prevent a class of abuse without additional rate-limiting infrastructure; O(1) enforcement at the middleware layer

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| `camelCase` JSON fields (MS recommendation) | Mismatches PostgreSQL column names and Rust/Go struct fields; requires a dedicated serialization mapping layer that adds complexity for no developer benefit in a backend-first SDK |
| Deeply nested URLs (e.g. `/orgs/:id/tenants/:id/apps/:id`) | Fragile; forces clients to carry parent IDs; long paths exhaust the URI limit; flat structure with query param scoping is simpler |
| URI limit of 2083 (MS recommendation) | Unnecessarily permissive; any real API call fits in 512 characters; longer limits provide no benefit and widen the attack surface |
| `X-Request-Id` header only, not in error body | Some clients only parse the response body; having it in both adds zero cost and eliminates a friction point during debugging |

## Consequences

**Positive:**
- One casing rule for all fields — no per-resource exceptions
- URL shape is learnable in minutes; any resource follows the same pattern
- Request size enforcement at the middleware layer — no handler needs to guard against oversized payloads individually

**Negative:**
- `snake_case` JSON diverges from the Microsoft API guidelines and some JavaScript client conventions; clients that auto-map JSON fields to camelCase properties need a transformation step
- 512-character URI limit could constrain complex filter expressions embedded in query strings — use of the filtering spec (ADR 008) mitigates this by keeping query strings short
