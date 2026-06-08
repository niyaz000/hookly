# ADR api-design/003: Error response shape, status codes, and auth failure handling

## Status
Accepted

## Context

Every endpoint needs a consistent contract for failures. Inconsistent error shapes force clients to write per-endpoint error handling; inconsistent status codes create guesswork about what happened. Three questions drive this decision:

1. What does an error response body look like?
2. Which HTTP status code maps to which scenario?
3. How are authentication and authorization failures distinguished?

The Microsoft Azure API guidelines define a nested `error.code` / `error.message` envelope with an optional `details` array. We reviewed this but chose a flatter shape for reasons documented below.

## Decision

### Error response body

```json
{
  "error_code": "validation_error",
  "error_message": "Request validation failed",
  "errors": [
    {
      "field": "name",
      "value": "",
      "code": "required",
      "message": "Name must not be blank"
    }
  ],
  "request_id": "0190e3f4-3c7e-7b5a-bc2a-4d8e9f0a1b2c",
  "doc_url": "https://docs.hookly.dev/errors/validation_error"
}
```

**Fields:**

| Field | Type | Notes |
|---|---|---|
| `error_code` | string | Machine-readable snake_case code; stable API contract |
| `error_message` | string | Human-readable summary for the developer |
| `errors` | array | Present only when there are field-level errors (validation); otherwise omitted |
| `request_id` | string | UUIDv7 generated per request; correlates to structured logs |
| `doc_url` | string | Direct link to the error's documentation page |

**Field error shape** (each element of `errors`):

| Field | Type | Notes |
|---|---|---|
| `field` | string | Dot-notation path for nested fields (e.g. `address.city`) |
| `value` | string | The rejected value; omitted if absent or sensitive |
| `code` | string | Machine-readable code (e.g. `required`, `max_length`, `invalid_format`) |
| `message` | string | Human-readable explanation |

### Status code table

| Scenario | HTTP code |
|---|---|
| POST — resource created | 201 Created |
| GET — resource found | 200 OK |
| PUT / PATCH — resource updated | 200 OK |
| DELETE — resource deleted | 204 No Content |
| POST — action executed (non-create) | 200 OK |
| Field validation failed | 422 Unprocessable Entity |
| Resource not found | 404 Not Found |
| Conflicting unique value | 409 Conflict |
| Idempotency key reused with different body | 409 Conflict |
| Concurrent request with the same idempotency key | 409 Conflict |
| Missing or invalid credentials | 401 Unauthorized |
| Valid credentials, insufficient permissions | 403 Forbidden |
| Request body exceeds 256 KB | 413 Payload Too Large |
| URI exceeds 512 characters | 414 URI Too Long |
| Malformed request (syntax, unknown fields) | 400 Bad Request |
| Server-side failure | 500 Internal Server Error |

**422 vs 400:** The request body was syntactically valid JSON but failed semantic validation (e.g., a required field was blank, a URL was malformed). 422 signals "I understood your request; your data is wrong." 400 is reserved for malformed requests that couldn't be parsed or unknown query parameters.

### Authentication vs. authorization

- **401 Unauthorized** — the request carries no credentials, expired credentials, or a credential that cannot be verified. The client must authenticate before retrying.
- **403 Forbidden** — the credentials are valid but the principal does not have permission for this resource or action. Re-authenticating will not help.
- **Never use 404 to hide resource existence** — Hookly is not a system where resource existence is itself sensitive. Prefer the honest 403.

### `error_code` stability

`error_code` is part of the API contract. Once published, an error code cannot be renamed, removed, or have its meaning changed without a version bump. New error codes may be added in non-breaking releases.

## Principles upheld

- **Developer experience** — flat structure parses in a single destructure; `doc_url` leads to the exact error page; `request_id` correlates to logs without a support ticket
- **Observability for everyone** — every error carries a request ID that traces directly to structured log output; clients can self-serve incident diagnosis
- **Reliability through simplicity** — one consistent shape across all endpoints; no per-resource error format variations

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Microsoft nested `error.code` inside an `error` object | Extra nesting with no benefit for our use case; clients have to write `response.error.code` instead of `response.error_code` |
| No `doc_url` | Removes self-serve discoverability; support burden increases |
| 403 for all auth failures (MS recommendation) | Violates RFC 7235: 401 is defined as "unauthenticated" — returning 403 when credentials are absent misleads clients into thinking they're logged in but lack permission |
| 400 for validation errors | 400 semantically means "I couldn't parse your request"; 422 more precisely means "I parsed it but the data is wrong" — the distinction matters for client-side error handling |

## Consequences

**Positive:**
- Uniform client-side error handling across all endpoints
- `error_code` stability contract allows clients to branch on error codes confidently
- `request_id` in every error body enables zero-friction log correlation without response headers

**Negative:**
- `error_code` values must be documented and treated as immutable API surface — adding a new code is easy, but renaming one requires a version bump
- 422 vs. 400 distinction requires discipline: validation errors must always go through the `Validation` variant, not `BadRequest`
