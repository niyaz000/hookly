# ADR database/003: Read/write pool split with PostgreSQL read replica

## Status
Accepted

## Context

Three separate binaries (API server, scheduler, worker) issue both reads and writes to PostgreSQL simultaneously. The read patterns differ significantly from the write patterns:

**Heavy reads**: delivery job detail (worker), endpoint configuration (worker), event payload (worker), schedule list (scheduler reconciliation), delivery attempt history (API), event list (API)

**Writes**: delivery job status updates (worker, high frequency), outbox inserts (API server + scheduler), schedule `next_run_at` updates (scheduler)

Under sustained delivery load, the worker's read queries for job details and endpoint configuration compete with its own write queries for delivery status updates on the same connection pool. This creates head-of-line blocking: a slow list query can delay a delivery status write.

Additionally, the vision document identifies traffic spikes as a primary operational concern. A read replica provides horizontal read capacity that absorbs spike traffic without affecting the primary write path.

## Decision

### Pool split

```rust
pub struct DbPool {
    pub write: PgPool,   // PostgreSQL primary — all mutations
    pub read:  PgPool,   // PostgreSQL read replica — list queries, job reads
}
```

Both pools are configured in `AppState` and passed to all handlers and background tasks.

### Read/write routing rules

| Operation | Pool | Rationale |
|---|---|---|
| INSERT / UPDATE / DELETE | write | Mutations must hit the primary |
| Auth / permission checks | write | Cannot tolerate replica lag on security decisions |
| API key validation | write | Stale read could allow a revoked key |
| Worker: read endpoint URL, signing secret | read | Seconds of lag is acceptable; endpoint config changes rarely |
| Worker: read delivery job payload | read | Immutable after creation; always consistent |
| API: list events, delivery attempts, schedules | read | Acceptable to show data that's a few seconds old |
| API: GET single resource | read | Acceptable; client just created it via the write pool |
| Scheduler: SELECT active schedules (reconciliation) | read | Consistency window of 2 minutes is already built into the reconciliation cadence |
| Scheduler: INSERT outbox + UPDATE next_run_at | write | Must be durable immediately |

### Replica failover

If the read replica is unavailable, fall back to the write pool transparently:

```rust
impl DbPool {
    pub fn read_or_fallback(&self) -> &PgPool {
        if self.read.is_closed() { &self.write } else { &self.read }
    }
}
```

This means a replica outage degrades read performance (extra load on primary) but does not cause a service outage. A metric (`hookly_db_read_replica_fallback_total`) tracks when fallback is active.

### Replication lag tolerance

Replication lag of 1–5 seconds is acceptable for all read operations routed to the replica except auth/security checks (always on primary). The delivery worker reading a 2-second-old endpoint configuration will not affect correctness — endpoints are updated rarely compared to the delivery rate.

## Principles upheld

- **Performance as a first-class concern** — read and write workloads no longer compete on the same connection pool; the worker can issue dozens of concurrent endpoint reads without blocking delivery status writes
- **Reliability through simplicity** — the failover logic is a single `if` on pool health; no connection retry loops, no circuit breaker for DB reads
- **Frugality** — a single read replica is sufficient for most traffic patterns; additional replicas can be added later (all reads go to `read` pool, which can be backed by a load-balanced replica set)

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Single pool for all reads and writes | Write-heavy delivery load and read-heavy list queries compete; at sustained throughput, this causes head-of-line blocking and elevated p99 latency |
| Read replica per service (separate replica for API vs. worker) | Adds operational complexity; a single replica with separate connection pools per binary is equivalent for our workload |
| CQRS with separate read model (denormalised views) | Significant additional complexity; a read replica of the same schema is sufficient and requires no application-layer data synchronisation |
| PgBouncer for connection pooling only (no replica) | Reduces connection overhead but does not add read capacity; the primary still handles all reads under spike traffic |

## Consequences

**Positive:**
- Write-path latency is unaffected by read spike traffic
- The replica absorbs list query load from the API during high-read periods
- Replica failover is transparent to callers
- Adding additional read replicas in the future requires only changing `read` pool configuration

**Negative:**
- Two DB pool configurations to manage (two connection strings, two pool size settings)
- Replication lag means a client that creates a resource and immediately lists it may not see it in the list response; the `GET /{id}` endpoint mitigates this (use write pool for single-resource reads on the create flow)
- Replica adds cost and an additional DB instance to monitor and maintain
