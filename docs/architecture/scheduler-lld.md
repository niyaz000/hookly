# Scheduler — Low-Level Design

---

## How it works — full picture

```
                       hookly-scheduler  (one or more instances)
┌──────────────────────────────────────────────────────────────────────┐
│                                                                      │
│  Worker count = SCHEDULER_WORKER_COUNT (default 4, static config)    │
│  Each instance spawns N worker tasks that pick shards dynamically.   │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                │
│  │  worker 0    │  │  worker 1    │  │  worker N    │  ← N tasks,    │
│  │              │  │              │  │              │    no fixed    │
│  │ loop:        │  │ loop:        │  │ loop:        │    shard       │
│  │  ① pick     │  │  ① pick      │   │  ① pick     │               │
│  │  ② lock NX  │  │  ② lock NX   │   │  ② lock NX  │               │
│  │  ③ poll due │  │  ③ poll due  │   │  ③ poll due │               │
│  │  ④ fire or  │  │  ④ fire or   │   │  ④ fire or  │               │
│  │    remove    │  │    remove    │  │    remove    │                │
│  │  ⑤ unlock   │  │  ⑤ unlock    │  │  ⑤ unlock   │                │
│  └──────┬───────┘  └──────────────┘  └──────────────┘                │
│         │                                                            │
│  ┌──────▼──────────────────────────────────────────────────────┐     │
│  │ reconcile task  (full bootstrap at startup, delta every 2m) │     │
│  │  SELECT changed schedules from PG → ZADD NX into sorted sets│     │
│  │  Also: ZADD GT sched:shards for each active shard           │     │
│  └─────────────────────────────────────────────────────────────┘     │
└────────────────────────┬─────────────────────────────────────────────┘
                         │
       ┌─────────────────┴──────────────────┐
       ▼                                    ▼
 Redis (Scheduler)                    PostgreSQL
 ───────────────                      ──────────
 sched:shards          ZSET           schedules
   score = last_added_unix_ms         delivery_jobs
   (dynamic; API + reconciler write)  events
                                      schedule_executions
 sched:pending:{shard} ZSET
   score = next_run_at (unix)

 sched:lock:{shard}    STRING TTL 30s
   value = instance_id, SET NX
                         │
                         │  after fire: XADD into delivery streams
                         ▼
                   hookly-worker
```

---

## Shard discovery (sched:shards)

`sched:shards` is a ZSET where each member is a shard ID and the score is the
unix millisecond timestamp at which the shard was last confirmed to have active
schedules. Workers read from it with `ZRANDMEMBER`; the score serves as an
optimistic version for the removal race described below.

**Who writes to sched:shards:**
- API: `ZADD GT sched:shards {now_ms} {shard_id}` on every schedule create, update
  (if next_run_at changed), restore, and resume.
- Reconciler: `ZADD GT sched:shards {now_ms} {shard_id}` for every active shard it
  finds in the DB — this is the safety net.

**GT flag:** only updates the score if the new value is strictly higher. Monotonically
increasing timestamps prevent clock skew from downgrading a score.

**Removal race:**
```
  t=0  Worker reads score S from sched:shards for shard X.
  t=1  Worker sees ZRANGEBYSCORE sched:pending:X → empty.
  t=2  API adds new schedule → ZADD GT sched:shards {now_ms > S}   ← score bumped
  t=3  Worker: Lua check → ZSCORE shard X ≠ S → no-op, shard stays ✓

  If t=2 happened before t=0 (score was already bumped):
       Worker: Lua check → ZSCORE == S → ZREM shard X
       BUT pending is now non-empty, so ZRANGEBYSCORE would have returned it at t=1.
       → The schedule is still in sched:pending; reconciler re-adds shard within 2 min.
```

---

## Per-worker loop

```
each worker task (continuous loop):

  ┌─────────────────────────────────────────────────────┐
  │ ① Pick shard                                       │
  │   ZRANDMEMBER sched:shards                          │
  │   Empty → sleep SCHEDULER_IDLE_SLEEP_MS, retry      │
  └───────────────────────┬─────────────────────────────┘
                          │
  ┌───────────────────────▼─────────────────────────────┐
  │ ② Read score                                        │
  │   ZSCORE sched:shards {shard_id} → S               │
  │   (used for race-safe removal at step ④)           │
  └───────────────────────┬─────────────────────────────┘
                          │
  ┌───────────────────────▼─────────────────────────────┐
  │ ③ Acquire lock                                      │
  │   SET sched:lock:{shard} {instance_id} NX EX 30    │
  │   Fail → skip (another worker owns it)             │
  └───────────────────────┬─────────────────────────────┘
                          │
  ┌───────────────────────▼─────────────────────────────┐
  │ ④ Poll due schedules                               │
  │   ZRANGEBYSCORE sched:pending:{shard}               │
  │                 0  {now_unix}  LIMIT 50            │
  │                                                     │
  │   Empty →                                          │
  │     Lua: if ZSCORE sched:shards shard == S         │
  │           then ZREM sched:shards shard             │
  │     DEL lock → back to ①                           │
  │                                                     │
  │   Non-empty → fire_schedule for each               │
  └───────────────────────┬─────────────────────────────┘
                          │
  ┌───────────────────────▼─────────────────────────────┐
  │ ⑤ Release lock (Lua: DEL only if still owner)     │
  │   Back to ①                                        │
  └─────────────────────────────────────────────────────┘
```

---

## fire_schedule

```
  Fetch schedule + active endpoints from PG
  Compute next_run_at from cron expression + timezone
        │
        ▼
  ┌─────────────────────────────────┐
  │     PostgreSQL transaction      │
  │                                 │
  │  INSERT events       (1 row)    │
  │  INSERT delivery_jobs (M rows,  │
  │         one per endpoint)       │
  │  INSERT schedule_executions     │
  │  UPDATE schedules               │
  │    next_run_at = {computed}     │
  │    last_run_at = now            │
  │  COMMIT                         │
  └────────────────┬────────────────┘
                   │
        ┌──────────▼───────────┐
        │  best-effort Redis   │
        │                      │
        │  ZADD sched:pending  │  ← reschedule for next fire
        │  XADD delivery queue │  ← hand off to worker
        │  register_stream     │  ← worker stream discovery
        └──────────────────────┘

If XADD fails → delivery_job stays in PG.
Worker outbox poller picks it up within 10s.
```

**Stream routing:**
```
  org.tier == "enterprise"  →  hookly:q:org:{org_id}   (isolated)
  otherwise                 →  hookly:q:tier:{tier}     (shared)
```

**M = number of active endpoints on the schedule.**

---

## Reconciliation

```
Runs once at startup (full bootstrap), then delta every 2 min:

  Startup (last_reconciled_at = None):
    SELECT id, assigned_shard, next_run_at
    FROM   schedules  WHERE  status = 'active'
           │
           ▼
    ZADD NX sched:pending:{shard} for every row
    ZADD GT sched:shards {now_ms} for every unique shard

  Delta (last_reconciled_at = T):
    SELECT id, assigned_shard, next_run_at
    FROM   schedules
    WHERE  status = 'active' AND updated_at > T - 1s
           │
           ▼
    ZADD NX for changed rows only
    ZADD GT sched:shards {now_ms} for unique shards in this delta

  NX → never overwrites a score the fire loop already set.
  GT → score only goes up, preventing clock-skew downgrades.
  last_reconciled_at is updated to now() after each successful run.

  Repairs:
    • Scheduler downtime → missed creates/updates caught on next delta
    • Redis crash        → restart scheduler to trigger full bootstrap
    • Shard accidentally removed by worker → reconciler re-adds it
    • Schedule created while scheduler offline → caught by delta
```

---

## Redis keys at a glance

```
Key                          Type    TTL      Purpose
───────────────────────────  ──────  ───────  ───────────────────────────────────────
sched:shards                 ZSET    none     Active shards; score = last_added_unix_ms
sched:pending:{shard}        ZSET    none     Due schedules; score = next_run_at (unix)
sched:lock:{shard}           STRING  30s      Processing lock; value = instance_id (NX)
hookly:q:streams             ZSET    none     Worker stream discovery; score = last_claimed_ms
hookly:q:tier:{tier}         Stream  —        Delivery queue (shared tenants)
hookly:q:org:{org_id}        Stream  —        Delivery queue (enterprise, isolated)
```
