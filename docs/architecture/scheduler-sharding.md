# Scheduler sharding

This document covers how the schedule space is divided across shards, how the API server picks a shard at schedule create time, how shards map to Redis instances, how tenants can be pinned to dedicated shards, and how shards are added, drained, paused, and decommissioned.

For the per-shard tick loop mechanics — heartbeat, fire lock, outbox write, and outbox cost analysis — see [scheduled event flow](scheduled-event-flow.md).

---

## Overview

A cron schedule stored in a single Redis sorted set is a hot key: every scheduler instance polling every 5 seconds contends on the same key. Sharding splits the sorted set into N independent sets. Each scheduler instance owns a subset of shards and polls only its own.

Key invariant: **a schedule's shard is assigned once at create time and stored in the database as `schedules.assigned_shard`.** No component re-derives the shard from the current configuration. This has two consequences:

- Changing the active shard set does not retroactively move existing schedules.
- A shard can be drained without touching existing entries — they fire out naturally.

---

## Routing sorted set

The API server maintains a routing sorted set in the Coordinator Redis:

```
sched:routing   sorted set
  score  = number of schedules currently assigned to this shard
  member = shard_id (integer string)
```

Only shards in state `active` are members of this set. When a shard is drained, paused, or decommissioned its member is removed from the set — no new schedules can land on it.

This gives a load-aware assignment: the API server always picks the shard with the fewest schedules, producing a round-robin effect that distributes load evenly without any explicit rebalancing.

### Atomic pick-and-increment

Picking the lowest-score shard and incrementing its counter must be atomic. The API server uses a Lua script to avoid a TOCTOU race between two concurrent schedule creates:

```lua
-- KEYS[1] = sched:routing
local shard = redis.call('ZRANGE', KEYS[1], 0, 0)[1]   -- member with lowest score
if not shard then
    return redis.error_reply('no active shards')
end
redis.call('ZINCRBY', KEYS[1], 1, shard)
return shard
```

`ZRANGE ... 0 0` returns the single member with the smallest score. `ZINCRBY` increments it atomically in the same script execution. Redis executes Lua scripts atomically (single-threaded), so no two concurrent creates can pick the same shard at score 0 and both leave it at score 1 — one will see score 1 and the other score 2.

### Decrement on schedule delete

When a schedule is soft-deleted or hard-deleted:

```
ZINCRBY sched:routing -1 {assigned_shard}
```

This keeps the routing scores accurate over the lifetime of the system. A score floor of 0 is enforced — a Lua guard prevents decrementing below zero in case of reconciliation drift.

---

## Shard assignment — the API server path

Every schedule create goes through a two-path assignment function:

```
fn assign_shard(tenant_id, shard_config, redis) -> u16 {
    // Enterprise path: tenant has a pinned shard
    if let Some(dedicated) = shard_config.tenant_affinity(tenant_id) {
        // Increment the routing score for the dedicated shard directly —
        // does not affect min-score selection for other tenants.
        redis.zincrby("sched:routing", 1, dedicated);
        return dedicated;
    }

    // Standard path: pick lowest-score active shard, increment atomically
    return redis.eval(PICK_AND_INCREMENT_LUA, ["sched:routing"]);
}
```

The result is stored as `schedules.assigned_shard` before the 201 is returned. Every downstream operation — the tick loop's post-fire ZADD, the reconciliation task's rebuild — reads `assigned_shard` from the database rather than recomputing it.

---

## System topology

```
                        SCHEDULE CREATE
                             │
              ┌──────────────▼───────────────────┐
              │           API Server             │
              │                                  │
              │  1. tenant has shard affinity?   │
              │       yes ──► dedicated shard    │
              │              ZINCRBY score       │
              │       no  ──► Lua: ZRANGE 0 0    │
              │              lowest-score shard  │
              │              ZINCRBY score       │
              │                                  │
              │  2. INSERT schedules             │
              │     assigned_shard = S           │
              │                                  │
              │  3. ZADD sched:pending:{S}       │
              └────────────┬─────────────────────┘
                           │
         ┌─────────────────┼──────────────────┐
         │                 │                  │
    shard 0, 1         shard 2, 3         shard 4
         │                 │                  │
         ▼                 ▼                  ▼
     Redis-A           Redis-B           Redis-C
 ┌───────────┐      ┌───────────┐      ┌───────────┐
 │pending:0  │      │pending:2  │      │pending:4  │
 │pending:1  │      │pending:3  │◄─┐   └───────────┘
 └─────┬─────┘      └─────┬─────┘  │
       │                  │        └── Acme Corp
 Scheduler-1        Scheduler-2        (affinity: 3)
 owns: 0, 1         owns: 2, 3
 reads: Redis-A     reads: Redis-B

 Scheduler-3 (if added):
 owns: 4
 reads: Redis-C

 All scheduler instances connect to:
 ┌─────────────────────────────────────┐
 │         Coordinator Redis           │
 │  sched:routing         sorted set  │
 │    score = schedule count           │
 │    member = shard_id               │
 │  sched:owner:{shard}   TTL 30s     │
 │  sched:fire:{id}:{min} TTL 120s    │
 └─────────────────────────────────────┘

 All schedulers write to, API server reads from:
 ┌─────────────────────────────────────┐
 │            PostgreSQL               │
 │  schedules.assigned_shard           │
 │  schedules.next_run_at              │
 │  scheduler_shards (state table)     │
 │  tenant_shard_affinity              │
 └─────────────────────────────────────┘
```

The Coordinator Redis holds the routing set, ownership keys, and fire locks — it is small and does not hold any sorted set data. Each data Redis node holds only the `sched:pending:{shard}` sorted sets for its assigned shards.

---

## Shard → Redis config

Shards and their Redis nodes are declared in static config. Each scheduler instance reads this at startup to resolve which Redis connection to open for each owned shard.

```toml
[[scheduler.shards]]
id    = 0
redis = "redis://data-a:6379"
state = "active"

[[scheduler.shards]]
id    = 1
redis = "redis://data-a:6379"
state = "active"

[[scheduler.shards]]
id    = 2
redis = "redis://data-b:6379"
state = "active"

[[scheduler.shards]]
id    = 3
redis = "redis://data-b:6379"
state = "active"

[scheduler.coordinator_redis]
url = "redis://coordinator:6379"
```

Connection resolution on startup:
1. Open one persistent connection to the Coordinator Redis.
2. For each owned shard, look up its `redis` URL. Multiple shards on the same Redis node share one connection pool.
3. Start one tick loop task per owned shard, each using its resolved data Redis connection.

The API server reads shard config at startup to populate the assignment function's `tenant_affinity` cache. An admin API call to change shard state refreshes the routing sorted set — no restart required.

---

## Tenant-dedicated shards

An enterprise tenant can be pinned to a specific shard. All schedules created under that tenant go to that shard, regardless of the routing sorted set.

```
tenant_shard_affinity table:
  tenant_id   UUID      references tenants.id
  shard_id    SMALLINT
  note        TEXT      (optional: reason or tier label)
```

The routing sorted set score for the dedicated shard is still incremented on each create — this keeps the score accurate for display and reconciliation purposes, and means the dedicated shard naturally appears higher-loaded to the standard assignment path, which is the desired behavior (other tenants should not be routed there).

Admin API:

```
# Pin a tenant to a dedicated shard
PUT /admin/v1/tenants/{tenant_id}/shard-affinity
    {"shard_id": 3, "note": "Acme Corp — enterprise SLA"}

# Remove the pin (new schedules revert to routing set assignment)
DELETE /admin/v1/tenants/{tenant_id}/shard-affinity
```

Removing the affinity does not move existing schedules. Their `assigned_shard` stays at the dedicated shard until they are deleted, expire, or are explicitly migrated via the shard migration endpoint.

Multiple enterprise tenants can share one dedicated shard, or each get their own. A dedicated shard gains full isolation when its data Redis node is also dedicated — the tick loop for that shard runs independently and is unaffected by load on other tenants' shards.

---

## Scaling up — adding shards

1. Add new shard entries to config with `state = "active"`.
2. Assign them to a scheduler instance via static config (`SCHEDULER_OWNED_SHARDS=4,5`) or leave unclaimed for dynamic takeover.
3. Add the new shards to the routing sorted set with score 0:
   ```
   ZADD sched:routing 0 4
   ZADD sched:routing 0 5
   ```
   The assignment function immediately starts routing new schedules to these shards because they have the lowest score.
4. The scheduler instance that picks up a new shard finds an empty sorted set. The reconciliation task populates only schedules already assigned there:
   ```sql
   SELECT id, next_run_at FROM schedules
   WHERE assigned_shard = 4 AND status = 'active'
   ```
   Existing schedules on other shards are unaffected.

New shards fill quickly because they start at score 0 and will be picked for every new schedule until their score catches up with the existing shards. There is no automatic migration of existing schedules.

---

## Shard lifecycle — states

Each shard has one of four states, persisted in a `scheduler_shards` table and cached in the API server:

| State | In routing set | Tick loop | New schedules | Existing schedules |
|---|---|---|---|---|
| `active` | yes | running | assigned here | firing normally |
| `draining` | no | running | not assigned | continue firing until empty |
| `paused` | no | stopped | not assigned | accumulate (will fire when resumed) |
| `drained` | no | stopped | not assigned | none remaining |

### Active

Normal operating state. The shard is a member of `sched:routing` and receives new schedule assignments. The tick loop runs and fires due schedules.

### Draining

Stop routing new schedules to a shard without disrupting existing ones:

```
POST /admin/v1/scheduler/shards/{N}/drain
```

Effect:
- Shard state → `draining` in DB
- `ZREM sched:routing {N}` — removed from routing set; no new schedules land here
- Tick loop continues — due entries fire and are re-added to `sched:pending:{N}` on reschedule
- One-off schedules fire and are not re-added; the sorted set empties naturally over time
- Recurring schedules fire indefinitely until explicitly migrated

```
  Shard 3 — drain example
  ─────────────────────────────────────────────────────────

  Before drain:                     After drain:
  sched:routing:                    sched:routing:
    shard 0 → score 250               shard 0 → score 250
    shard 1 → score 248               shard 1 → score 248
    shard 2 → score 251               shard 2 → score 251
    shard 3 → score 249               (shard 3 removed)

  New schedules: distributed          New schedules: distributed
  across 0, 1, 2, 3                   across 0, 1, 2 only

  Acme Corp (affinity: 3):            Acme Corp (affinity: 3):
  still routed to shard 3             still routed to shard 3
  (affinity bypasses routing set)     (affinity bypasses routing set)
```

### Paused

Temporarily halt all activity on a shard — useful during Redis maintenance or incident investigation:

```
POST /admin/v1/scheduler/shards/{N}/pause
```

Effect:
- Shard state → `paused` in DB
- `ZREM sched:routing {N}` — no new schedules
- Tick loop stops for this shard — no fires, no heartbeat renewals
- Shard ownership key expires naturally after 30s; another scheduler instance will not claim it while state is `paused` (the claim sweep skips paused shards)
- Schedules accumulate as overdue in the sorted set

```
POST /admin/v1/scheduler/shards/{N}/resume
```

Effect:
- Shard state → `active`
- `ZADD sched:routing {current_score} {N}` — re-added to routing set with its current schedule count
- Scheduler instance re-claims the shard and starts the tick loop
- Overdue schedules in the sorted set are processed on the next tick; fire locks prevent duplicates

### Drained

Permanently remove a shard including all recurring schedules:

1. **Drain** first (above) to stop new assignments.
2. **Migrate** existing schedules off:
   ```
   POST /admin/v1/scheduler/shards/{N}/migrate
   ```
   Runs a batched migration pass:
   - `SELECT id, assigned_shard FROM schedules WHERE assigned_shard = N AND status = 'active'` (batches of 500)
   - For each schedule: pick new shard via routing set Lua script, `UPDATE assigned_shard` in DB, `ZADD` to new sorted set, `ZREM` from shard N's sorted set
   - Runs until no rows remain with `assigned_shard = N`

3. **Decommission** once the sorted set is empty:
   ```
   POST /admin/v1/scheduler/shards/{N}/decommission
   ```
   - Shard state → `drained`
   - Scheduler instance releases ownership of the shard
   - Coordinator Redis key expires naturally
   - Shard N's data Redis sorted set is now empty

---

## Shard state machine

```
          ┌──────────┐
          │  active  │◄──────────────────────────────┐
          └──┬───────┘                               │
             │                                       │ POST .../resume
             │ POST .../drain   POST .../pause       │ POST .../reactivate
             │                       │               │
             ▼                       ▼               │
       ┌──────────┐           ┌──────────┐           │
       │ draining │           │  paused  ├───────────┘
       └────┬─────┘           └──────────┘
            │
            │ POST .../migrate
            │ + POST .../decommission
            ▼
       ┌──────────┐
       │  drained │
       └──────────┘

  active   — in routing set; receives new schedules; tick loop runs
  draining — not in routing set; tick loop runs; one-offs drain naturally;
             recurring schedules stay until explicitly migrated
  paused   — not in routing set; tick loop stopped; schedules accumulate;
             resumes cleanly on POST .../resume
  drained  — not in routing set; tick loop stopped; sorted set empty;
             can be reactivated via POST .../reactivate (ZADD score 0)
```

A drained shard can be reactivated without a scheduler restart, as long as its config entry and data Redis connection remain in place. Reactivation adds it back to `sched:routing` at score 0, making it the highest-priority target for new schedules until its load equalizes.

---

## Routing set reconciliation

The routing sorted set scores can drift if the API server crashes mid-create (after DB insert, before `ZINCRBY`) or if a schedule delete fails to decrement. The reconciliation task (runs every 2 minutes) corrects this:

```sql
SELECT assigned_shard, COUNT(*) as n
FROM schedules
WHERE status = 'active'
GROUP BY assigned_shard
```

For each active shard, the reconciliation task sets the routing score to the DB count:

```
ZADD sched:routing {count} {shard_id}   -- NX not set, unconditional update
```

This is a ZADD with an exact score override, not an increment — it self-corrects any accumulated drift from missed increments or decrements. Only shards in state `active` or `draining` are touched; `paused` and `drained` shards remain absent from the set.
