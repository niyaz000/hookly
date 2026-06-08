# ADR api-design/006: API versioning strategy and breaking change policy

## Status
Accepted

## Context

All HTTP APIs eventually need to evolve. The question is not whether the API will change, but whether those changes will break existing clients. This ADR defines how versions are communicated, what constitutes a breaking change, and how breaking changes are introduced when unavoidable.

Three common versioning mechanisms exist: URL path prefix (`/api/v1/`), query parameter (`?api-version=2026-06-01`), and `Accept` header versioning. Each has trade-offs in discoverability, routing complexity, and caching behavior.

## Decision

### Versioning mechanism: URL path prefix

All endpoints are prefixed with `/api/v{n}/`:
```
/api/v1/applications
/api/v1/endpoints
/api/health        (unversioned — not part of the contract)
```

The version number is a monotonically increasing integer starting at 1. It increments only when a breaking change is introduced.

### What constitutes a breaking change

A breaking change is any change that requires clients to modify existing code to continue working correctly:

| Change | Breaking? |
|---|---|
| Removing a response field | **Yes** |
| Renaming a response field | **Yes** |
| Changing a response field's type | **Yes** |
| Changing a successful HTTP status code | **Yes** |
| Adding a required request field | **Yes** |
| Tightening validation on an existing field | **Yes** |
| Removing or renaming an endpoint or HTTP method | **Yes** |
| Renaming or removing a stable `error_code` | **Yes** |
| Adding an optional response field | No |
| Adding a new endpoint | No |
| Adding a new optional request field | No |
| Relaxing validation on an existing field | No |
| Adding a new `error_code` for a new failure scenario | No |

### Deprecation process

Before a breaking change ships in v2, the deprecated behavior in v1 is marked with a `Deprecation` response header on every affected endpoint:

```
Deprecation: Sun, 01 Jan 2027 00:00:00 GMT
Sunset: Sun, 01 Jan 2027 00:00:00 GMT
Link: <https://docs.hookly.dev/migration/v1-to-v2>; rel="deprecation"
```

- Minimum notice period: **6 months** between the `Deprecation` date and the `Sunset` date
- Clients that observe `Deprecation` in a response have a machine-readable signal to act on
- The old version continues serving `200` responses until the sunset date — it does not begin returning errors

After the sunset date, v1 endpoints return `410 Gone` with:
```json
{
  "error_code": "version_sunset",
  "error_message": "API v1 was sunset on 2027-01-01. Migrate to /api/v2/.",
  "doc_url": "https://docs.hookly.dev/migration/v1-to-v2"
}
```

### Parallel version support

Two versions run simultaneously only during the deprecation window. Once v1 is sunset, it is removed from the codebase. We do not maintain more than two versions at once.

### Version negotiation

There is no content negotiation. A request to `/api/v1/` always receives the v1 response shape regardless of `Accept` headers.

## Principles upheld

- **Developer experience** — URL path versioning is immediately visible in browser address bars, curl output, and server logs; no hidden header or query param to discover
- **Two-person operations ceiling** — two versions maximum at once; a small team can reason about the full API surface without a version compatibility matrix
- **Reliability through simplicity** — URL-based routing is trivially proxied, load-balanced, and cached; no per-request version parsing at the middleware layer

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Query parameter versioning (`?api-version=2026-06-01`) | Requires every client to append the parameter to every request; easily omitted; harder to route at the proxy layer; date format creates a false impression of per-date stability guarantees |
| Header versioning (`Accept: application/vnd.hookly.v1+json`) | Invisible in logs and browser tools; cache-unfriendly unless `Vary: Accept` is set; harder for developers to reason about |
| Never break (treat all changes as non-breaking) | Impractical over a multi-year lifespan; forces conservative schema design that accumulates technical debt |
| Semantic versioning (major.minor.patch) | Minor and patch distinctions add classification overhead with no client-facing value; the only relevant distinction is "does my existing code break?" |

## Consequences

**Positive:**
- Breaking changes are explicit, deliberate, and rare — not accidents
- Deprecation headers give clients a machine-readable migration signal
- Running only two versions simultaneously caps operational complexity
- URL routing is trivial to implement in any proxy or gateway

**Negative:**
- URL path versioning means a version bump changes every URL — clients that hardcode full URLs (not just the base) must update all call sites
- The 6-month minimum notice period means a bad design decision in v1 must be carried for at least 6 months before it can be corrected
- No mechanism to preview breaking changes early (no `-preview` suffix like MS's date versioning scheme)
