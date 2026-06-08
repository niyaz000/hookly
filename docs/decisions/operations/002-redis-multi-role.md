# ADR operations/002: Redis split by operational role

## Status
Accepted

## Context

A single Redis instance serves multiple workloads with very different characteristics:

| Workload | Access pattern | Persistence requirement | Memory profile |
|---|---|---|---|
| Delivery queue (Streams) | High write throughput; large message payloads; consumer groups | **Critical** — loss = lost delivery jobs | Grows with queue depth |
| Rate limits, CB state, inflight counters | Many small reads/writes; short TTLs | **Ephemeral** — loss = brief fail-open period | Small, bounded |
| Scheduler sorted sets | Bulk ZADD on reconciliation; ZRANGEBYSCORE on tick | **Important** — loss = 2-minute sorted set rebuild from DB | Proportional to active schedule count |
| Idempotency keys, endpoint state cache | Short TTL; cache-semantics; loss tolerable | **Ephemeral** — loss = cold cache + idempotency reset | Bounded by 24h TTL window |

Running all four workloads on a single Redis instance creates several problems:
- A large reconciliation `ZADD` (bulk sorted set rebuild) blocks other Redis operations (Redis is single-threaded per shard)
- A queue depth spike inflates Redis memory, potentially evicting rate-limit TTL keys
- Enabling AOF persistence for the queue also adds write latency to the ephemeral workloads that don't need it
- A Redis instance resize (for queue capacity) requires a restart that interrupts all four workloads simultaneously

## Decision

Four logical Redis roles, each independently configurable:

```rust
pub struct AppRedis {
    pub queue:     Arc<dyn Queue>,   // Delivery queue — Redis Streams
    pub state:     RedisClient,      // CB, rate limits, inflight, maintenance flags
    pub scheduler: RedisClient,      // Scheduler sorted sets + fire locks
    pub ephemeral: RedisClient,      // Idempotency, endpoint state cache
}
```

In small deployments, all four roles can point to the same Redis instance (one connection string, four logical namespaces). In large deployments, each role can be a separate Redis instance or cluster.

### Persistence policy per role

| Role | Persistence | Rationale |
|---|---|---|
| `queue` | **AOF always** (`appendfsync always`) | Loss = lost delivery jobs; write latency overhead is acceptable for this correctness guarantee |
| `scheduler` | **AOF everysec** (`appendfsync everysec`) | Loss = sorted set rebuild in 2 minutes; 1-second data loss window is acceptable |
| `state` | **None** (no persistence) | Loss = CB/RL/inflight reset; self-heals from actual responses within minutes; fail-open is the correct safety mode |
| `ephemeral` | **None** (no persistence) | Loss = idempotency keys reset + cache cold; acceptable |

### Failure isolation

| Redis role down | Impact | Recovery |
|---|---|---|
| `queue` | Delivery stops; outbox relay cannot publish | Relay retries; recovery begins when Redis restarts |
| `state` | Fail-open: rate limits bypass, CBs reset to CLOSED | Self-heals from endpoint responses; brief delivery burst to struggling endpoints |
| `scheduler` | Sorted sets unavailable; scheduler falls back to DB polling | Reconciliation rebuilds sorted sets on restart; 2-minute max disruption |
| `ephemeral` | Idempotency window resets; endpoint state cache cold | Cache warms within 100ms of restart; idempotency window loss is logged |

The critical path for delivery is `queue` and `state`. `scheduler` and `ephemeral` failures degrade gracefully.

## Principles upheld

- **Reliability through simplicity** — splitting by role means a `state` Redis restart does not interrupt queue delivery; failures are isolated rather than cascading
- **Frugality** — small deployments use one instance; the split costs nothing until it is needed; large deployments pay for separation only where it matters (`queue` gets expensive AOF, `ephemeral` gets no persistence)
- **Performance as a first-class concern** — the reconciliation task's bulk `ZADD` no longer competes with delivery queue reads/writes; each workload is sized independently

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Single Redis instance for all roles | A queue memory spike evicts rate-limit keys; AOF overhead applies to ephemeral workloads unnecessarily; a single restart interrupts all four workloads |
| Redis Cluster (single logical cluster, multiple shards) | More complex to operate than separate instances; cross-slot Lua scripts are not supported in cluster mode, blocking some of the Lua-based atomic operations used in rate limiting |
| Separate Redis per binary (API, scheduler, worker) | Three Redis instances instead of four logical roles; does not align isolation with workload characteristics; worker would need to access scheduler state |

## Consequences

**Positive:**
- `queue` Redis can be sized, persisted, and upgraded independently of `state` Redis
- A `state` Redis crash does not interrupt delivery (fail-open with self-healing)
- Persistence overhead (`appendfsync always`) is only paid by the workload that needs it
- Role-specific connection pools allow independent timeout and retry tuning

**Negative:**
- Up to four Redis connection strings to manage in production configuration
- The `AppRedis` struct must be plumbed through all components that previously used a single Redis client
- Small deployments that use one Redis instance still pay the code complexity of the role split (mitigated by the fact that the split is purely configuration)
