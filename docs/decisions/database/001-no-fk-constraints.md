# ADR-001: No FK constraints in the database

## Status
Accepted

## Context

Relational FK constraints are the standard way to enforce referential integrity in PostgreSQL. They prevent orphaned rows, give the query planner join hints, and make schema intent explicit. For a system with a single PostgreSQL instance and a straightforward data model, they are usually the right default.

However, Hookly is a multi-tenant platform where:

- Tenants, applications, endpoints, events, and delivery attempts form a deep ownership hierarchy
- Deletes are almost always soft deletes (`deleted_at`), meaning a "deleted" parent row still exists — FK constraints would not fire, but the semantic relationship is broken in a way constraints don't model
- Future migration to a sharded or partitioned PostgreSQL topology (or distributing specific tables to separate schemas) would require dropping FK constraints anyway; adding them now would create upgrade friction
- Integration tests need to insert data in arbitrary order without wrestling with constraint ordering; the absence of FKs simplifies test setup significantly

## Decision

No foreign key constraints are defined in any migration. Referential integrity is enforced exclusively at the application layer:

- Handlers look up parent resources before inserting child rows
- Soft-delete operations propagate logically (e.g., deleting a tenant disables its webhooks) rather than via `ON DELETE CASCADE`
- Repository methods return `Option<T>` for lookups, and handlers convert `None` to `AppError::NotFound`

Unique constraints and check constraints are still used freely — they enforce within-row and within-column invariants that don't require cross-table joins.

## Consequences

**Positive:**
- Schema migrations are simpler — no constraint drop/re-add ordering required
- Test fixtures can be inserted in any order
- No risk of accidental cascade deletes from a missed `ON DELETE` clause
- Schema is portable across topologies (single PG, Citus, PlanetScale-style sharding)

**Negative:**
- The database cannot self-heal orphaned rows; a bug in the application layer can produce inconsistent state
- No automatic query planner hints from FK metadata (compensated by explicit indexes)
- Engineers reading the schema cannot infer relationships from DDL alone — the `docs/architecture/data-model.md` serves as the substitute reference
