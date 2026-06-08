# ADR operations/003: Redis crash recovery

## Status
Accepted

## Context

Despite AOF persistence on the queue Redis role, a catastrophic failure (corrupted AOF, hardware loss, misconfigured instance) can result in total data loss for the Redis queue. Without a recovery mechanism, any delivery jobs that were in Redis Streams but not yet processed would be permanently lost.

Additionally, even with AOF, the Pending Entries List (PEL) — which tracks in-flight messages that have been dequeued but not yet ACKed — may not survive a Redis crash if the worker that held those messages also crashed simultaneously. The PEL is Redis-side state; if Redis loses it, the worker has no knowledge of what it was processing.

The question is: how does Hookly recover from Redis queue data loss without operator intervention?

## Context: what is at risk

| Data | In Redis | Recoverable from DB? | Recovery mechanism |
|---|---|---|---|
| Queued jobs (not yet dequeued) | Streams | Yes — outbox table has all pending entries | Outbox relay replays on startup |
| In-flight jobs (dequeued, not ACKed) | PEL | Partial — DB has `status='delivering'` rows | Recovery task re-enqueues stuck delivering jobs |
| Scheduler sorted sets | Sorted sets | Yes — DB has `next_run_at` for all schedules | Reconciliation task rebuilds on startup |
| Rate limits / CB state | State Redis (ephemeral) | No — intentionally ephemeral | Fail-open; self-heals from endpoint responses |
| Idempotency keys | Ephemeral Redis | No — intentionally ephemeral | 24h window resets; logged as a metric |
| Inflight counters | State Redis | No — intentionally ephemeral | Reset to 0; brief burst risk |

## Decision

### Layer 1: Outbox pattern (primary recovery)

The outbox table (see [ADR architecture/003](../architecture/003-outbox-pattern.md)) is the authoritative record of every job that needs to be enqueued. On Redis startup, the outbox relay task immediately runs:

```sql
SELECT id, queue, payload FROM outbox
WHERE status = 'pending'
ORDER BY created_at
LIMIT 200
FOR UPDATE SKIP LOCKED
```

All pending outbox entries are replayed to Redis Streams. No jobs that entered the outbox are lost, regardless of Redis state. This covers all jobs that were not yet dequeued when Redis crashed.

### Layer 2: Recovery task (in-flight job recovery)

Jobs that were dequeued from Redis (removed from the stream into the PEL) but not yet completed are tracked in PostgreSQL as `delivery_jobs WHERE status = 'delivering'`. If the worker that held them crashed alongside Redis, these rows are permanently `delivering` — a lie.

A recovery task runs:
1. On every worker startup
2. Every 10 minutes as a background sweep

```sql
SELECT id FROM delivery_jobs
WHERE status = 'delivering'
  AND updated_at < NOW() - INTERVAL '5 minutes'
```

For each result:
1. `UPDATE delivery_jobs SET status = 'pending', started_at = NULL`
2. `INSERT INTO outbox (queue, payload)` — re-enqueue via outbox

The 5-minute threshold is intentionally conservative: a delivery attempt in progress for more than 5 minutes (the maximum HTTP timeout is 30 seconds) is definitively stuck. False positives (a legitimately slow but alive worker) are impossible with this threshold given the configured timeouts.

This re-delivery may produce duplicate deliveries: if the original worker succeeded at the HTTP call but crashed before writing the `succeeded` status, the endpoint will be called again. This is the at-least-once guarantee — tenants use `X-Hookly-Delivery` for deduplication (see [ADR delivery/003](../delivery/003-at-least-once-delivery.md)).

### Layer 3: State Redis (fail-open)

Rate limits, circuit breaker state, and inflight counters live in ephemeral Redis (no AOF). On a crash:

- Rate limits reset: endpoints that were rate-limited may receive one additional burst of requests before the CB re-opens or the endpoint sends a new 429. This is expected and logged.
- Circuit breakers reset to CLOSED: a broken endpoint receives probe attempts until its failure threshold is reached again and the CB reopens. This may produce N additional failed deliveries (where N = `cb_failure_threshold`, default 5) before the CB re-opens.
- Inflight counters reset to 0: a brief burst of concurrent deliveries may exceed the configured caps until the counters rebuild. This is bounded by the number of in-flight jobs at the time of the crash.

These are acceptable trade-offs given that state Redis is explicitly designated as ephemeral in [ADR operations/002](002-redis-multi-role.md).

### Recovery checklist (automated)

On worker startup, the worker binary runs the following in order:
1. Verify DB connectivity (write pool)
2. Run recovery task (re-enqueue stuck `delivering` jobs)
3. Verify Redis queue connectivity
4. Start outbox relay (drains pending outbox → Redis Streams)
5. Start work-stealing pool (begins processing)
6. Log `hookly_redis_recovery_replayed_total` metric with count of replayed jobs

The scheduler binary runs on startup:
1. Verify DB connectivity
2. Verify Redis scheduler connectivity
3. Run reconciliation task (rebuild sorted sets from DB)
4. Start tick loop

## Principles upheld

- **Reliability through simplicity** — the outbox table eliminates the "fire-and-forget XADD" failure mode; recovery is a SQL query and a relay loop, not a distributed consensus protocol
- **Auditing as a core feature** — the outbox table provides a complete record of every job; the recovery task's re-enqueue count is a metric; operators know exactly how many jobs were recovered after a crash
- **Automation and self-healing** — recovery runs automatically on startup without operator intervention; the state Redis self-heals from new endpoint responses

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Redis AOF only (no outbox) | AOF covers Redis-side persistence but not the crash window between DB write and XADD; the outbox closes this gap |
| Redis replication + AOF (no recovery task) | Replication does not protect against corrupted AOF or both primary and replica failing; the recovery task handles the PEL loss that replication cannot recover |
| Re-query DB on every dequeue (no PEL reliance) | Eliminates in-flight state from Redis but removes the throughput benefit of stream-based consumer groups; the recovery task's 10-minute sweep is a cheaper safety net |
| Operator-triggered recovery script | Requires a human to identify and run recovery after every Redis crash; violates the automation and self-healing principle |

## Consequences

**Positive:**
- A complete Redis queue crash (including AOF corruption) can be recovered automatically from PostgreSQL
- In-flight job recovery is bounded: only jobs stuck for > 5 minutes are re-enqueued; legitimate in-progress deliveries are not disturbed
- Operators receive a metric on how many jobs were recovered, giving visibility into the impact of the crash

**Negative:**
- The recovery task may produce duplicate deliveries for jobs that succeeded before the crash; this is inherent to at-least-once delivery and mitigated by the `X-Hookly-Delivery` dedup header
- State Redis (rate limits, CBs) self-heals but the brief fail-open window may produce a burst of delivery attempts to rate-limited or circuit-broken endpoints immediately after recovery
- The 5-minute stuck-job threshold means a legitimate extremely slow delivery (longer than the configured HTTP timeout due to infrastructure issues) could be incorrectly re-enqueued; the configured `read_timeout_ms` on endpoints prevents this in practice
