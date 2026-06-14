# ADR api-design/010: Entity tags — structured key-value metadata on all resources

## Status
Accepted

## Context

Every resource in Hookly (applications, endpoints, event types, events, schedules) represents infrastructure-level concepts. But callers operate in application domains with their own taxonomies that Hookly does not and should not understand natively. Without an escape hatch, any metadata a caller needs to associate with a Hookly resource must be stored in the caller's own database, maintained in sync, and joined on every read. This is exactly the kind of accidental coupling a webhook platform should eliminate.

The broader pattern is well-established: Stripe, AWS, GitHub, and Datadog all expose a `metadata` or `tags` field on every resource. The recurring reason is the same — the platform cannot enumerate every dimension its customers will need to filter, search, or report on, so it provides a structured slot for the caller to fill in.

### Specific use cases that surface the need

**Legacy migration and coexistence**  
When migrating from a previous webhook provider or from a homegrown delivery system, the caller's existing records carry IDs that predate Hookly. A tag like `{ "legacy_id": "wh_0039128" }` on the new endpoint lets the caller's migration scripts and rollback paths look up Hookly resources by the ID the rest of the codebase already knows. Without tags this mapping requires an external cross-reference table. With tags it is a single API filter call.

**Multi-environment fan-out**  
A platform team may share a single Hookly organization across environments while routing differently per environment. Tags like `{ "env": "staging", "region": "us-east-1" }` on endpoints allow the team to query `endpoints?tags[env]=staging` instead of maintaining separate Hookly organizations per environment.

**Team and ownership attribution**  
In large organizations multiple teams register endpoints under the same application. Tags like `{ "team": "payments", "owner": "alice@example.com" }` provide discoverable ownership without introducing a Hookly-native ownership concept that would vary by customer org chart.

**Cost center and billing attribution**  
SaaS platforms that resell or internally chargeback infrastructure costs tag events with `{ "cost_center": "enterprise-tier", "customer_id": "cust_7842" }` to drive downstream billing aggregation without coupling the billing system to Hookly's internal schema.

**A/B experiments and feature flags**  
A/B infrastructure can tag events with `{ "experiment_id": "exp_42", "variant": "b" }` to correlate delivery outcomes with experiment results without storing experiment state inside Hookly.

**Data classification and compliance**  
PII routing policies sometimes require tagging resources with `{ "data_class": "pii", "gdpr_region": "eu" }` so that data-plane enforcement logic (which stream to route to, which encryption key to use) can read the tag rather than reconstructing classification from payload content.

**Customer-specific metadata in multi-tenant SaaS**  
A platform that exposes Hookly to its own end-customers can attach `{ "customer_id": "cust_99", "plan": "enterprise" }` to endpoints so that a single API key can serve multiple downstream customers while still maintaining traceable per-customer audit trails.

**Operational runbook links**  
Incident response often requires knowing which service owns an endpoint. Tags like `{ "runbook": "https://wiki.internal/runbooks/payments-webhooks", "pagerduty": "P3K9X1" }` embed operational context directly on the resource, reducing mean time to resolve.

### Why filtering matters as much as storage

Storing tags without being able to filter on them is of limited value. A caller who tags 5,000 endpoints with `{ "team": "payments" }` needs `GET /endpoints?tags[team]=payments` to work efficiently. This drives the storage choice: the tag payload must be queryable server-side without a full scan.

## Decision

### Structure

Tags are a **flat JSON object** (string keys, string values) attached to every major resource:

```json
{
  "tags": {
    "legacy_id": "wh_0039128",
    "env": "production",
    "team": "payments"
  }
}
```

An array-of-pairs alternative was considered:
```json
{ "tags": [{ "key": "env", "value": "production" }] }
```
The object form was chosen because it is easier to read and write at the call site (no array traversal), enforces key uniqueness at the representation level, and maps directly to JSONB in PostgreSQL.

### Constraints

| Dimension | Limit | Reason |
|---|---|---|
| Max tags per resource | 5 | Keeps the index compact; prevents tags from becoming a free-form document store |
| Max key length | 64 characters | Long enough for descriptive keys, short enough to stay indexed efficiently |
| Max value length | 255 characters | Matches the common VARCHAR(255) convention; long values belong in external systems |
| Key characters | Printable ASCII, no leading/trailing whitespace | Prevents invisible-character bugs in filter queries |
| Value type | String only | Avoids ambiguity between `1` (number) and `"1"` (string) in filter comparisons |

Validation is enforced at the API boundary on every create and update. An attempt to exceed any limit returns `400 Bad Request` with a field-level error pointing to the specific violation.

### Storage

Tags are stored as **JSONB** in PostgreSQL on each resource table:

```sql
tags JSONB NOT NULL DEFAULT '{}'
```

A GIN index with `jsonb_path_ops` supports efficient equality lookups across all key-value pairs:

```sql
CREATE INDEX idx_endpoints_tags ON endpoints USING GIN (tags jsonb_path_ops);
```

The query for `tags[env]=production` becomes:

```sql
WHERE tags @> '{"env": "production"}'::jsonb
```

This uses the GIN index and is O(log N) regardless of the number of endpoints.

### API surface

Tags are accepted on create and update for all resources. On create, omitting the field is equivalent to `{}`. On update, the entire tags map is replaced (last-write-wins), not merged — callers who want to add a single tag must include the full current set plus the new key.

```
POST /api/v1/endpoints
{
  "url": "https://example.com/hook",
  "tags": { "team": "payments", "env": "production" }
}

PATCH /api/v1/endpoints/:id
{
  "tags": { "team": "payments", "env": "production", "legacy_id": "wh_0039128" }
}
```

Filtering on list endpoints:

```
GET /api/v1/endpoints?tags[team]=payments&tags[env]=production
```

Multiple tag filters are combined with AND semantics (return resources that match all specified tags).

### Entities that carry tags

| Resource | Typical tag use cases |
|---|---|
| Application | Environment, business unit, deployment region |
| Endpoint | Team ownership, legacy ID, runbook link, customer ID |
| Event type | Data classification, compliance region, schema version |
| Event | Experiment ID, request correlation ID, customer segment |
| Schedule | Cost center, originating service, cron job name |

## Principles upheld

- **Developer experience** — tags remove the need for callers to maintain an external cross-reference table for any metadata Hookly does not natively model
- **No accidental platform coupling** — Hookly remains ignorant of what the tags mean; the caller owns the taxonomy entirely
- **Queryability by default** — GIN index on every tagged table means filtering is a first-class operation, not a post-fetch client-side scan

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Custom columns per entity (`team`, `owner`, `env`) | Platform would need to enumerate every dimension upfront; immediately wrong for every customer with different terminology; schema migrations required to add new dimensions |
| Separate `tags` join table (`resource_type`, `resource_id`, `key`, `value`) | Extra JOIN on every read; harder to atomically update tags with the parent resource; more complex index design for multi-tag AND queries |
| Free-form text labels (array of strings, no values) | No structure for key-value relationships; `"env:production"` and `"env:staging"` are strings with no machine-readable distinction; filtering on key alone is not possible without string parsing |
| Arbitrary nested JSON | Removes depth guarantee; complex values belong in the caller's system; query complexity scales non-linearly |
| HSTORE instead of JSONB | JSONB is a superset of HSTORE; JSONB has better ecosystem support (operators, GIN, driver serialization); HSTORE values must be strings but JSONB can evolve if the type constraint is ever relaxed |

## Consequences

**Positive:**
- Callers attach any metadata they need without schema changes or support requests
- Legacy ID mapping is first-class, enabling clean provider migrations
- Multi-environment and multi-team Hookly installations are operationally tractable without separate organizations
- Filtering by tag is index-backed and composable with other filters (status, created_at ranges)
- Tags are returned on every read response — no extra round trip to fetch metadata

**Negative:**
- Tags are opaque to Hookly — the platform cannot enforce semantic constraints (e.g., "env must be one of prod/staging/dev"); callers enforce their own taxonomy
- Replace-on-update semantics mean a caller who omits tags on a PATCH accidentally clears them; clients must read-before-write when doing partial updates
- GIN indexes add write overhead; on very high-volume event tables the index maintenance cost is a factor to monitor
- 5-tag limit may be insufficient for some enterprise use cases; the limit is a tunable tradeoff, not a fundamental constraint
