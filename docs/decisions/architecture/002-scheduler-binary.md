# ADR architecture/002: Scheduler as a separate binary

## Status
Accepted

## Context

Hookly supports tenant-defined cron schedules: a tenant registers a cron expression, an event type, a payload, and a set of endpoint targets. At each scheduled tick, the scheduler must evaluate which schedules are due, create the corresponding events and delivery jobs in the database, and enqueue them for the delivery worker.

This workload has a distinct resource profile from both the API server and the delivery worker:

- **CPU-light**: the scheduler computes next-fire times and does batch DB writes — no heavy computation
- **Timing-sensitive**: ticks must run within a few seconds of the cron minute boundary; scheduler lag is a user-visible metric
- **Low concurrency**: unlike the delivery worker (hundreds of concurrent outbound HTTP calls), the scheduler runs a small number of sequential DB operations per tick
- **Stateful ownership**: each scheduler instance owns a subset of schedule shards; this requires a shard heartbeat and leader-election mechanism that does not belong inside the API server or worker

Embedding the scheduler into the delivery worker would mean the worker's concurrency tuning (connection pool sizes, Tokio thread counts) is shared with a process that has opposite requirements.

## Decision

A third binary, `hookly-scheduler`, runs independently of both the API server and the delivery worker. It shares the same PostgreSQL database and Redis.

The scheduler's responsibilities are strictly bounded:

1. Own one or more schedule shards (via Redis heartbeat)
2. On each 5-second tick: query the Redis sorted set for due schedules, acquire per-fire dedup locks, write events + delivery jobs + outbox entries to PostgreSQL in a batch, and update `next_run_at`
3. Run a reconciliation task every 2 minutes to rebuild the Redis sorted sets from the PostgreSQL source of truth
4. Respect the `sys:scheduler_paused` maintenance flag

The scheduler **does not deliver webhooks**. It produces outbox entries. The outbox relay (running inside the worker binary) picks up from there.

Shard ownership uses a Redis heartbeat: `sched:owner:{shard}` key with a 30-second TTL, refreshed every 10 seconds. If an instance's heartbeat expires, any other scheduler instance can claim the shard via `SET NX`. This gives automatic failover without a separate leader-election protocol.

## Principles upheld

- **Reliability through simplicity** — the scheduler has one job; it cannot be starved by worker concurrency pressure or API request spikes
- **Two-person operations ceiling** — each binary has a single, well-defined role; an operator can restart the scheduler independently without worrying about delivery state
- **Automation and self-healing** — shard heartbeat + `SET NX` reclaim means scheduler failover is automatic; no manual intervention required when an instance goes down

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Embed scheduler into the delivery worker | Shared connection pools and Tokio runtime settings conflict; a worker deploy triggers scheduler downtime; scheduler lag metrics are conflated with delivery metrics |
| Embed scheduler into the API server | API server is latency-sensitive; background tick work competes with request handling; scheduler state (shard ownership) does not belong in a stateless API tier |
| External cron service (e.g., Kubernetes CronJob per schedule) | One K8s CronJob per tenant schedule is operationally infeasible at scale; no sub-minute flexibility; removes visibility into the scheduler from the Hookly codebase |

## Consequences

**Positive:**
- Scheduler can be scaled independently (typically 2–4 instances for HA, not hundreds)
- Scheduler deploys do not interrupt delivery or API traffic
- Scheduler lag is a cleanly separable metric (`hookly_scheduler_lag_seconds`)
- Shard failover is automatic and sub-30-second

**Negative:**
- Third binary to build, deploy, and monitor
- All three binaries must stay schema-compatible during deploy windows
- Shard ownership configuration (`SCHEDULER_OWNED_SHARDS`) must be coordinated across instances in static-assignment mode
