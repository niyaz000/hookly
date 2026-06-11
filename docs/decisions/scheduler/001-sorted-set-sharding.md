# ADR scheduler/001: Redis sorted set sharding for cron schedule evaluation

## Status
Accepted

## Context

Hookly supports tenant-defined cron schedules. Each schedule has a `cron_expression`, a `timezone`, and a `next_run_at` timestamp. The scheduler must evaluate which schedules are due on each tick and enqueue their delivery jobs.

At scale, the number of active schedules can reach hundreds of thousands. Two evaluation strategies exist:

**Database polling**: `SELECT * FROM schedules WHERE next_run_at <= NOW() AND status = 'active' ORDER BY next_run_at LIMIT 500` every 5 seconds. Simple and correct, but at 500K active schedules, even with an index on `next_run_at`, each poll returns a small result set after scanning the index — acceptable, but this puts polling load on the primary database at a frequency that grows with the number of scheduler instances.

**Redis sorted set hot path**: Store `(next_run_at_unix, schedule_id)` pairs in a sorted set. On each tick: `ZRANGEBYSCORE sched:pending:{shard} 0 {now_unix} LIMIT 500`. This is O(log N + M) where M is the number of due schedules, with no DB involvement on the hot path. The database is updated after firing (writing `next_run_at` and outbox entries), and periodically reconciled as a safety net.

At 50K+ active schedules, the sorted set approach is significantly more efficient. The database remains the authoritative source of truth; Redis is the fast evaluation path.

## Decision

### Sorted sets

```
sched:pending:{shard}   sorted set
  score  = next_run_at unix timestamp
  member = schedule_id (UUID string)
```

`num_shards` is configurable (default: 4). Shard assignment uses a **routing sorted set** in Coordinator Redis:

```
sched:routing   sorted set
  score  = number of schedules currently assigned to this shard
  member = shard_id
```

The API server picks the lowest-score member (fewest current schedules) atomically via a Lua script, then increments that shard's score. This produces a round-robin distribution without a hash function. The computed shard is stored as `schedules.assigned_shard` at create time and never recomputed — no coordination needed for placement.

For the full assignment model — tenant affinity, multi-Redis topology, shard states, drain and decommission protocol — see [scheduler sharding](../../architecture/scheduler-sharding.md).

### Shard ownership

Each scheduler instance owns one or more shards:
```
sched:owner:{shard}   → instance_id, TTL = 30s (refreshed every 10s)
```

Shard assignment is configured via `SCHEDULER_OWNED_SHARDS=0,1` (static) or claimed dynamically via `SET NX` when an owner's TTL expires (automatic failover). If an instance goes down, another instance claims its shards within 30 seconds.

### Per-fire dedup lock

Multiple scheduler instances (or a recovering instance reprocessing a shard) must not fire the same schedule twice in the same cron minute. Before enqueuing a due schedule:

```
SET sched:fire:{schedule_id}:{minute_bucket} 1 NX EX 120
```

If `NX` fails: another instance already fired this schedule in this minute. Skip it. The 120-second TTL ensures the lock expires before the next possible tick for a 1-minute minimum cron resolution.

### Tick loop

Every 5 seconds per owned shard:
1. `ZRANGEBYSCORE sched:pending:{shard} 0 {now_unix} LIMIT 500`
2. For each due schedule: attempt dedup lock, batch-insert events + delivery_jobs + outbox into PostgreSQL, update `schedules.next_run_at`
3. `ZADD sched:pending:{shard} {new_next_run_at} {schedule_id}` (update score)

### Reconciliation

Every 2 minutes, the reconciliation task queries PostgreSQL (`SELECT id, next_run_at FROM schedules WHERE status = 'active'`) via the read replica and rebuilds the sorted sets. This catches:
- Schedules created via API between reconciliation cycles (the API server also does a best-effort `ZADD` on create)
- Sorted set entries lost due to a Redis crash
- Score drift caused by clock skew between scheduler instances

### Minimum granularity

Cron expressions with sub-minute resolution (e.g., `*/30 * * * * *`) are not supported in v1. Minimum granularity is 1 minute. This keeps the tick interval (5 seconds) well below the minimum fire interval (60 seconds), ensuring no fires are missed due to tick timing.

## Principles upheld

- **Performance as a first-class concern** — `ZRANGEBYSCORE` on a sorted set is O(log N + M); evaluating 1M schedules for 100 due items costs O(log 1M + 100), not a full table scan
- **Reliability through simplicity** — the database remains the source of truth; Redis is a recoverable cache; the reconciliation task and the dedup lock together prevent both missed fires and duplicate fires
- **Automation and self-healing** — shard failover is automatic (TTL + SET NX); sorted set recovery is automatic (reconciliation task); no operator action required when a scheduler instance goes down
- **Frugality** — one sorted set per shard (4 by default) replaces O(n) DB queries per tick; the reconciliation task runs infrequently; scheduler memory footprint is proportional to the number of active schedules, not their payload

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Database polling only (no sorted sets) | At 500K active schedules, polling the DB every 5 seconds per scheduler instance is 12 DB queries/minute per instance; with 4 scheduler instances this is 48 DB reads/minute on a table with 500K rows; the index makes each cheap, but sorted sets are cheaper still |
| In-memory heap (no Redis) | State is lost on scheduler restart; the heap must be rebuilt from the DB; correctness requires the same dedup mechanism anyway — the sorted set approach eliminates the in-memory/Redis divergence |
| One sorted set (no sharding) | A single sorted set is a hot key; multiple scheduler instances all `ZRANGEBYSCORE` the same key; Redis is single-threaded, so this becomes a bottleneck at high schedule counts |
| Hash-based shard assignment (FNV-1a % N) | Distributes evenly in theory but cannot account for schedule density differences across tenants; all schedules with the same ID prefix land on the same shard. The routing sorted set approach uses actual schedule count as the assignment signal, producing better real-world balance and allowing tenants to be pinned to dedicated shards without touching the hash |
| Kafka topic partitioned by shard | Adds Kafka as a dependency; sorted sets give equivalent sharding semantics without the operational overhead |

## Consequences

**Positive:**
- Scheduler evaluation time is O(log N + M) where M is typically small (schedules due in the current 5-second window)
- Shard failover is automatic and sub-30-second
- Reconciliation guarantees eventual consistency with the database
- Dedup lock prevents duplicate fires under any concurrent-scheduler scenario

**Negative:**
- The routing sorted set must be bootstrapped on first deployment and kept in sync; the reconciliation task corrects drift every 2 minutes
- Adding shards requires manually adding them to `sched:routing` (or restarting to pick up new config); this is a one-time operator step
- The dedup lock has a 120-second TTL; in the unlikely scenario of a scheduler instance holding a lock for > 120 seconds (severe GC pause or hang), the next instance will fire — producing a duplicate delivery
