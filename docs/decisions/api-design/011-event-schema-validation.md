# ADR api-design/011: Event payload schema validation strategy

## Status
Accepted

## Context

Every event published to Hookly references an `event_type`, which carries an `event_schema`
(a typed property definition stored as JSONB). At the point of `POST /api/v1/events`, the
server has both the event type's schema and the caller's payload available. The question is
what to do when they disagree.

Two options were considered:

**Option A — Hard reject (422 on mismatch)**
Refuse to accept the event if the payload doesn't satisfy the schema. Simple contract:
publish succeeds only with a conforming payload.

**Option B — Soft validation (accept event, flag mismatch)**
Always store the event; add `schema_valid: bool` and `schema_errors: string[]` to the
response and the `events` row. The event is delivered regardless; the producer sees
immediately whether its payload matched.

## Decision

**Option B — soft validation.**

`schema_valid` and `schema_errors` are stored on every `events` row and returned in every
event response. A payload that violates the schema is accepted, stored, delivered, and
flagged — not rejected.

Validation is performed using the [`jsonschema`](https://crates.io/crates/jsonschema) crate.
The `PropertyDef` struct is converted to a standard JSON Schema Draft 7 document via
`PropertyDef::to_json_schema()` before being passed to the validator. If the schema itself
fails to compile (malformed definition), the event is treated as schema-valid to avoid
blocking delivery due to a server-side configuration error.

## Rationale

### Webhooks are delivered, not interrogated

Hookly is a delivery platform, not a validation gateway. The producer publishes an event;
the consumer receives it. Silently dropping an event because a field has the wrong type is
strictly worse than delivering it with a flag — the producer at least sees the error and
can fix it.

### Schema drift is a facts-of-life problem

In practice, producers and consumers update independently. A producer deploying a new
payload shape before the event type schema is updated would have all events rejected under
Option A. This is a hard operational failure in the delivery path with no graceful recovery
other than re-publishing all events after the schema is updated — at which point idempotency
keys must be rotated.

Under Option B, those events are delivered and flagged. The operator can observe
`schema_valid = false` rows, update the schema, and the producer can continue without
re-publishing.

### The flag is immediately actionable

`schema_errors` contains the full list of JSON Schema validation errors. This is more useful
to a developer than a `422` with no context about which fields violated which constraints.
The event response surfaces this inline; no second API call is needed.

### Hard rejection belongs at the producer, not the platform

If a specific use case requires strict enforcement (e.g., a regulated pipeline that must
never process malformed events), that enforcement should live in the consumer's own
validation layer or in an upstream API gateway with explicit schema enforcement policy. The
delivery platform remains the source of truth for delivery status, not payload correctness.

## Schema validation library

`jsonschema` (Rust) was chosen over alternatives because:

- Zero boilerplate validation loop — compile the schema once with `validator_for()`, call
  `iter_errors()` to collect all violations
- Supports JSON Schema Draft 7/2019-09/2020-12 out of the box
- `PropertyDef::to_json_schema()` is a ~60-line recursive converter that maps our custom
  type system to standard JSON Schema; no coupling between the storage format and the
  validation format

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Option A (hard reject 422) | Silent event loss during schema drift; no graceful recovery without re-publishing; fails the "platform stays up, producers fix themselves" principle |
| Write a custom recursive validator over `PropertyDef` | More code, more maintenance surface, less complete (would miss edge cases covered by the JSON Schema spec) |
| `valico` crate | Less actively maintained; similar API to `jsonschema` with no compelling advantage |
| Store `event_schema` directly as JSON Schema | Would allow using the stored JSON directly without a converter, but breaks the typed `PropertyDef` model used everywhere else and requires a DB migration of existing data |

## Consequences

**Positive:**
- Events are never lost due to schema drift or misconfiguration
- Producers see `schema_valid: false` + detailed `schema_errors` immediately in the create
  response — no polling, no second call
- `schema_valid = false` is queryable in PostgreSQL for observability, alerting, and audits
- Adding strict-mode enforcement later (e.g., per-tenant flag that rejects invalid events)
  is additive — the validation infrastructure is already in place

**Negative:**
- A downstream consumer that assumes all delivered events are schema-valid needs to be
  aware that Hookly does not enforce this; consumer-side validation is still recommended
- `schema_errors` adds ~50–200 bytes per event row on average for invalid events (nil for
  valid events where the column stores `[]`)

## Implementation

- `PropertyDef::to_json_schema()` — `src/features/event_types/models.rs`
- `EventService::validate_against_schema()` — `src/features/events/service.rs`, called
  after the event type is resolved, before the INSERT
- `schema_valid`, `schema_errors` columns — migration `20260624000003_add_schema_validation_to_events.sql`
- `EventRow.schema_valid`, `EventRow.schema_errors` — `src/features/events/models.rs`
- `EventResponse.schema_valid`, `EventResponse.schema_errors` — `src/features/events/models.rs`
