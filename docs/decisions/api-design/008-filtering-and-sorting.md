# ADR api-design/008: Filtering and sorting query parameters

## Status
Accepted

## Context

All list endpoints support filtering and sorting to let clients find specific subsets of resources without fetching every page. The two common approaches are:

**OData-style expression syntax:**
```
GET /events?$filter=status eq 'failed' and created_at gt 2026-01-01T00:00:00Z
GET /endpoints?$orderby=name desc,created_at
```
Flexible and composable, but requires a filter expression parser, defines operator precedence rules, and introduces a non-trivial attack surface (injection, denial-of-service via pathological expressions).

**Simple key-value params:**
```
GET /events?status=failed&created_at_after=2026-01-01T00:00:00Z
GET /endpoints?sort_by=name&sort_order=desc
```
Each filter is a named query parameter. No parser needed. Limited to pre-defined filters per endpoint, but sufficient for all observable use cases.

## Decision

### Filtering: simple key-value query parameters

Each list endpoint documents the specific filter parameters it accepts. There is no generic filter expression language.

```
GET /api/v1/events?status=pending
GET /api/v1/events?status=failed&application_id=app_Xk9...
GET /api/v1/endpoints?state=active
```

**Unknown query parameters return `400 Bad Request`:**
```json
{
  "error_code": "bad_request",
  "error_message": "Unknown query parameter: 'statsu'"
}
```

This surfaces typos immediately rather than silently returning unfiltered results. A client passing `statsu=failed` expecting filtered output would otherwise receive all records and not notice the bug.

**Multi-value filters:** Repeated parameters express OR conditions within a field:
```
GET /api/v1/events?status=pending&status=failed
→ WHERE status IN ('pending', 'failed')
```

**Range filters:** Date and numeric fields support `_before` and `_after` suffixes:
```
GET /api/v1/events?created_at_after=2026-01-01T00:00:00Z
GET /api/v1/events?created_at_before=2026-06-01T00:00:00Z
```

**DateTime format:** RFC 3339 with timezone (`YYYY-MM-DDTHH:mm:ssZ`). Values that cannot be parsed return `400 Bad Request` with code `invalid_format`.

### Sorting: `sort_by` and `sort_order`

```
GET /api/v1/events?sort_by=created_at&sort_order=asc
GET /api/v1/endpoints?sort_by=name&sort_order=desc
```

| Parameter | Values | Default |
|---|---|---|
| `sort_by` | Field name (documented per endpoint) | `created_at` |
| `sort_order` | `asc` \| `desc` | `desc` |

- `sort_order` without `sort_by` is ignored (default field and specified direction apply).
- `sort_by` with an unsupported field name returns `400 Bad Request` with code `invalid_value`.
- `sort_order` with an invalid value (`ascending`, `1`, etc.) returns `400 Bad Request` with code `invalid_value`.
- Multi-column sort is not supported in v1. Single-column sort is sufficient and keeps the cursor implementation simple (see [ADR api-design/001](001-cursor-pagination.md)).

### Interaction with cursor pagination

Sorting and filtering interact with cursor pagination: the cursor encodes position in the current sort order. A client must not change `sort_by` or `sort_order` mid-pagination — doing so produces undefined results. The cursor is opaque and will not decode correctly if the sort axis changes.

Filters can change between pages — a filter applied on page 1 must be re-applied on page 2 (the cursor does not encode filter state).

### Validation summary

| Scenario | Response |
|---|---|
| Unknown query parameter | `400 Bad Request`, `error_code: bad_request` |
| Unsupported `sort_by` value | `400 Bad Request`, `error_code: bad_request`, field error on `sort_by` with code `invalid_value` |
| Invalid `sort_order` value | `400 Bad Request`, `error_code: bad_request`, field error on `sort_order` with code `invalid_value` |
| Unparseable date value | `400 Bad Request`, `error_code: bad_request`, field error with code `invalid_format` |
| Valid but empty result set | `200 OK`, `data: []`, `next_cursor: null` |

## Principles upheld

- **Developer experience** — unknown parameter → immediate 400; typos are caught at the call site, not silently masked by unfiltered results
- **Frugality** — no OData parser: no attack surface, no dependency, no maintenance overhead; filter logic is simple SQL `WHERE` clause construction
- **Reliability through simplicity** — pre-defined filter params per endpoint are documented, testable, and immune to injection via expression evaluation

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| OData `$filter` expression syntax | Requires an expression parser; defines operator precedence rules; significantly larger attack surface; no observed use case in Hookly that requires arbitrary expression filtering |
| Silently ignore unknown params (MS recommendation) | A client with a typo in a filter name silently receives unfiltered results — looks correct, is wrong; fails the north star of making bugs immediately visible |
| Expose `sort_by` and `sort_order` as a single combined param (`sort=created_at:asc`) | Slightly more compact but requires splitting on `:` and handling malformed values; two named params are clearer and easier to document |
| Multi-column sort | Increases cursor complexity; no current use case; can be added in a non-breaking way later |

## Consequences

**Positive:**
- Filter and sort parameters are per-endpoint, documented, and statically validated — no runtime expression parsing
- Unknown parameter detection catches client bugs immediately
- Simple implementation: filter params map directly to SQL `WHERE` clauses; no query builder abstraction needed

**Negative:**
- Pre-defined filter params require a code change to add a new filter to an endpoint — not dynamic like OData
- Multi-value filters via repeated params are not intuitive to all clients (some HTTP clients serialize arrays differently)
- Filter state is not encoded in the cursor — clients paginating a filtered set must re-send filter params on every page request
