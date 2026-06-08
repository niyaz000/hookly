# ADR api-design/004: Soft delete as the default mutation pattern

## Status
Accepted

## Context

When a tenant deletes a resource — an application, an endpoint, an event type — two competing concerns arise:

1. **Audit and history:** delivery records, events, and audit logs reference the deleted resource. A hard delete breaks those references and corrupts the historical record.
2. **Operational simplicity:** clients expect DELETE to mean "gone." Returning deleted resources in list responses causes confusion and bugs.

The decision is how to reconcile these: make deletion permanent (hard delete) or keep the record and filter it from normal views (soft delete).

## Decision

All mutable resources that can be independently referenced use soft delete:
- A `deleted_at TIMESTAMPTZ` column marks when the record was deleted.
- All read queries (`SELECT`, list endpoints, get-by-id) filter `WHERE deleted_at IS NULL` — deleted records are invisible to the API.
- A `deleted_by UUID` column records which principal performed the deletion.

**DELETE endpoint behavior:**
- Returns `204 No Content` with no response body.
- Calling DELETE on an already-deleted resource is idempotent — returns `204` without error.
- Calling DELETE on a non-existent `public_id` returns `404 Not Found`.

**Restore endpoint:**
Resources that support soft delete expose a restore action:
```
POST /api/v1/{resource}/{id}/restore
```
Returns `200 OK` with the full resource body in the current state (with `deleted_at` and `deleted_by` cleared).

**Hard delete:**
Not exposed via the API. Purging records is an operator-level database operation, outside the application layer.

**Response fields:**
`deleted_at` and `deleted_by` are not included in API responses. They are internal state, not part of the resource contract.

**Scope:**
Not all tables use soft delete. Append-only or immutable records (events, delivery jobs) are never deleted via the API and do not need a `deleted_at` column.

## Principles upheld

- **Auditing as a core feature** — deleted resources remain in the database; audit trails, delivery records, and event history that reference the resource remain intact and queryable at the database level
- **Developer experience** — idempotent DELETE removes the need for clients to check existence before deleting; predictable 204 response in all delete-success cases
- **Reliability through simplicity** — no cascade logic; no orphan-reference edge cases in the application layer

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Hard delete | Breaks historical references in audit logs and delivery records; irrecoverable |
| Expose `deleted_at` in responses | Leaks implementation detail; forces every client to filter deleted resources themselves |
| Soft delete with a `status` field (e.g. `status: deleted`) | More complex state machine; `deleted_at` is simpler, widely understood, and directly queryable with a null check |
| Return 404 on delete of already-deleted resource | Non-idempotent behavior; client retries after network failure would see a spurious 404 |

## Consequences

**Positive:**
- Audit trail is preserved at the database level without additional logging infrastructure
- Historical joins between events/delivery jobs and deleted resources remain valid
- Restore is possible without data loss

**Negative:**
- Tables grow over time; periodic operator-level purge is needed for very high-churn resources
- Queries must always carry `WHERE deleted_at IS NULL` — a missing filter silently returns deleted records (enforced by convention, not the database)
- Idempotent DELETE means "resource not found" and "already deleted" are indistinguishable to the client — intentional, but worth noting
