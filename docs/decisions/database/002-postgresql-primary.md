# ADR database/002: PostgreSQL as the primary data store

## Status
Accepted

## Context

Hookly requires a durable, ACID-compliant relational database for:
- All resource state (schedules, endpoints, events, delivery jobs, tenants, users)
- The outbox table (atomicity between event creation and job enqueue)
- Delivery attempt history (audit trail)
- Schedule execution records

The database is written by three separate binaries (API server, scheduler, worker) simultaneously and queried by the same three. It is the single source of truth for all durable state.

Alternatives to PostgreSQL exist along two axes:
- **Relational vs. document**: MongoDB, DynamoDB, Cassandra
- **Managed complexity vs. self-hosted**: PlanetScale, CockroachDB, Neon

## Decision

PostgreSQL is the primary and only relational data store. No other database engine is used.

Key properties that make PostgreSQL the right choice for Hookly:

**`SELECT FOR UPDATE SKIP LOCKED`** — the outbox relay pattern requires row-level locking with skip semantics. This is a PostgreSQL-native feature, widely understood, and sufficient to replace a dedicated job queue database.

**UUIDv7 primary keys** — time-ordered UUIDs keep B-tree index inserts sequential, eliminating the page-split problem that random UUIDs cause. PostgreSQL's `uuid` type stores these efficiently.

**`TIMESTAMPTZ`** — all timestamps are stored with timezone. PostgreSQL normalises these to UTC on write and returns them with timezone offset on read. Cron timezone handling is correct by construction.

**`JSONB`** — event payloads and schedule payloads are stored as `JSONB`, enabling future indexed queries on payload fields without a schema migration.

**`CREATE INDEX CONCURRENTLY`** — adding indexes without locking the table is critical for zero-downtime schema evolution on a live production database.

**SQLx** — Rust's `sqlx` crate provides compile-time query verification against a live PostgreSQL schema. Type mismatches between Rust structs and DB columns are caught at compile time, not at runtime.

**Advisory locks** — used for distributed locking patterns (e.g., long-running migrations) without adding Redis as a dependency for this use case.

## Principles upheld

- **Battle-tested components** — PostgreSQL is the most widely deployed open-source relational database; the operational runbook is available in every cloud provider's documentation; there is no learning curve for an engineer joining the team
- **Minimal external dependencies** — one database for all persistent state; no separate document store, cache store (Redis is used only for queuing and ephemeral state), or search index
- **Two-person operations ceiling** — every major cloud provider offers managed PostgreSQL (RDS, Cloud SQL, Azure Database for PostgreSQL, Supabase, Neon); a two-person team can rely on managed service operations without a dedicated DBA

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| MySQL / MariaDB | Weaker JSON support; `SELECT FOR UPDATE SKIP LOCKED` available only in MySQL 8+; less natural UUIDv7 handling; PostgreSQL has a stronger ecosystem for the Rust/SQLx stack |
| CockroachDB | Distributed SQL with horizontal scaling; operational complexity is significantly higher; not battle-tested at the scale Hookly targets in v1; serialisable isolation adds latency overhead |
| MongoDB | Document model fits event payloads but not the relational data (users, roles, tenants, assignments); multi-document transactions are available but less mature than PG; SQLx type safety not available |
| DynamoDB / Cassandra | Excellent write throughput but poor for the complex queries Hookly needs (list with filtering, cursor pagination across multiple columns, joins); schema-on-read makes compile-time verification impossible |
| PlanetScale (MySQL-compatible) | No foreign key support by design (acceptable given ADR database/001, but not the reason to choose PlanetScale); proprietary branching workflow adds tooling coupling |

## Consequences

**Positive:**
- Compile-time query verification via SQLx catches schema/code drift before deployment
- ACID transactions make the outbox pattern correct without a distributed transaction protocol
- Managed PostgreSQL options are available on every major cloud provider with automated backups, point-in-time recovery, and failover
- The team can hire engineers with PostgreSQL experience — no specialised knowledge required

**Negative:**
- Single-node PostgreSQL is a vertical scaling ceiling; horizontal write scaling requires read replicas (reads) or sharding (writes), which adds complexity
- At very high event ingestion rates (>10K events/sec), the outbox table becomes a write hotspot; partitioning by `created_at` is the mitigation, deferred until needed
- PostgreSQL's connection model (one OS process per connection) requires connection pooling (PgBouncer or SQLx's built-in pool); at very high concurrency, this must be tuned carefully
