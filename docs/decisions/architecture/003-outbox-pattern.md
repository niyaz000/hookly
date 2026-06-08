# ADR architecture/003: Outbox pattern for reliable job enqueuing

## Status
Accepted

## Context

Both the API server (on event submission) and the scheduler (on cron tick) must write to the PostgreSQL database and then enqueue a job to Redis Streams. These are two separate operations with no distributed transaction spanning both.

If the process crashes, is killed, or loses network connectivity between the database write and the Redis `XADD`, the delivery job is permanently lost: the database has the event record, but no worker will ever process it. This is a silent correctness failure — the tenant submitted an event and received a 202 Accepted response, but their webhook endpoint is never called.

At scale with many concurrent submissions, even a low crash probability produces a steady background rate of missed deliveries. The existing delivery pipeline documentation acknowledges this as a known gap: "If the Redis call fails, the event is not re-tried by the handler — the primary operation has already succeeded."

## Decision

All job enqueues are written to an `outbox` table in the same PostgreSQL transaction as the primary write (event creation, delivery job creation). A relay task inside the worker binary drains the outbox to Redis Streams on a 100ms interval.

```sql
CREATE TABLE outbox (
    id          UUID PRIMARY KEY,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    queue       VARCHAR     NOT NULL,   -- target stream name
    payload     JSONB       NOT NULL,   -- serialized job reference
    status      VARCHAR     NOT NULL DEFAULT 'pending',  -- pending | published
    published_at TIMESTAMPTZ,
    attempts    INT         NOT NULL DEFAULT 0
);
```

The relay task uses `SELECT ... FOR UPDATE SKIP LOCKED` so multiple relay instances (one per worker process) never collide on the same rows. This allows the worker binary to run multiple replicas without coordination.

Redis Streams is now a **fast read cache** of the outbox, not the authoritative store. A Redis crash does not lose queued jobs — the relay replays from `outbox WHERE status = 'pending'` on startup.

The outbox is also the recovery mechanism for in-flight jobs after a Redis crash: a separate recovery task scans for `delivery_jobs WHERE status = 'delivering' AND updated_at < NOW() - INTERVAL '5 minutes'`, re-inserts them into the outbox, and resets their status to `pending`. See [ADR operations/003](../operations/003-redis-crash-recovery.md).

## Principles upheld

- **Reliability through simplicity** — a single atomic transaction replaces a two-phase operation with a crash window; the relay is a simple read-and-publish loop with no novel consensus logic
- **Battle-tested components** — `SELECT FOR UPDATE SKIP LOCKED` is a well-understood PostgreSQL pattern for job queues; no new infrastructure is required
- **Auditing as a core feature** — the outbox table provides a complete record of every job enqueued, with timestamps and publish status; delivery gaps are immediately visible without log spelunking
- **Automation and self-healing** — the relay and recovery tasks handle Redis unavailability automatically; no human intervention is needed to re-enqueue lost jobs

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Fire-and-forget XADD (current state) | Silent delivery loss on crash between DB write and XADD; risk grows linearly with write volume |
| Two-phase commit (PG + Redis XA) | Redis does not support XA; not a battle-tested pattern for this combination |
| Saga / compensating transaction | Requires inverse operations and significant application complexity; overkill for a publish step |
| Polling PostgreSQL directly (no Redis queue) | Eliminates the gap but puts polling load on the primary DB; Redis Streams provide back-pressure and fan-out that direct PG polling lacks |
| Kafka transactional producer | Exactly-once semantics available but adds Kafka as a dependency, violating the minimal external dependencies principle |

## Consequences

**Positive:**
- Job enqueue is now atomic with the primary write — no silent delivery loss on crash
- Redis is a disposable fast path; full recovery is possible from PostgreSQL alone
- The outbox table gives operators a real-time view of publish lag
- Multiple relay instances scale horizontally without coordination via `SKIP LOCKED`

**Negative:**
- One additional table write per job (the outbox row) adds a small write amplification overhead
- The 100ms relay interval introduces a maximum 100ms additional latency between event submission and queue availability (acceptable for webhook delivery; not a synchronous operation)
- Outbox table must be cleaned up periodically (`DELETE WHERE status = 'published' AND published_at < NOW() - INTERVAL '24h'`)
