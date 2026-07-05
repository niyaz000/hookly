# ADR-001: No FK constraints in the database

## Status
Accepted

## Problem

FK constraints couple the schema to a single-database topology. Hookly needs flexibility to shard, partition, or move entities across service boundaries — constraints would need to be dropped at any of those inflection points.

## Decision

No FK constraints in any migration. Referential integrity is enforced at the application layer.

## Reasons

**Architectural (tipping point)**
- Sharding/partitioning cannot enforce cross-shard FK constraints
- Entities moving to separate subsystems or microservices will live in independent databases; FKs block that boundary

**Operational**
- Soft deletes break FK semantics — a "deleted" parent row still exists, so constraints don't fire but the relationship is logically severed
- Test fixtures can insert in any order

## Tradeoffs

- Bugs in the app layer can produce orphaned rows; no DB-level self-healing
- Relationships aren't inferrable from DDL alone — the data-model doc is the reference
