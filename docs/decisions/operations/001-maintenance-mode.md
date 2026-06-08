# ADR operations/001: Maintenance mode and safe pause/resume

## Status
Accepted

## Context

Planned maintenance operations — database version upgrades, Redis instance migrations, instance resizing, certificate rotation — require that no in-flight deliveries are writing to the database and no new jobs are being dequeued from Redis during the maintenance window.

The naive approach is to stop all worker and scheduler processes (SIGTERM). This has three problems:
1. In-flight HTTP delivery requests are abandoned mid-flight; if the delivery succeeded but the status write did not, the job is re-delivered later (at-least-once guarantee, but unnecessary duplication)
2. Process restart triggers pod scheduling, health check delays, and consumer group re-registration — all adding latency to recovery
3. There is no operator-visible signal of "maintenance is in progress" — the system just looks down

A maintenance mode must allow processes to complete their current work, idle gracefully, and resume automatically when maintenance ends — without restarting.

## Decision

### Control plane signals

Two independent flags in Redis State, with a PostgreSQL `system_flags` table as fallback if Redis is unavailable:

```
Redis:
  sys:workers_paused    → "1" | (absent = running)
  sys:scheduler_paused  → "1" | (absent = running)

PostgreSQL fallback:
  system_flags table: (key VARCHAR PK, value VARCHAR, updated_at TIMESTAMPTZ)
  Checked if Redis is unreachable during pause check.
```

Flags can be set independently: pausing the scheduler without pausing workers allows existing queued jobs to drain while no new scheduled fires are added.

### Worker behaviour under pause

Every worker task polls `sys:workers_paused` before each `XREADGROUP` call:
```
if GET sys:workers_paused == "1":
    sleep 2s
    continue (do not dequeue)
```

In-flight jobs complete normally. The worker process does not exit. Recovery is instant — the flag is cleared and workers resume within 2 seconds, no restart required.

### Scheduler behaviour under pause

The tick loop polls `sys:scheduler_paused` before each tick:
```
if GET sys:scheduler_paused == "1":
    sleep 5s
    continue (do not tick)
```

The scheduler stays alive, the shard heartbeat (`sched:owner:{shard}`) continues to refresh so shard ownership is not lost during the maintenance window.

### Admin API

```
POST   /admin/v1/maintenance          { scope: "workers" | "scheduler" | "all" }
DELETE /admin/v1/maintenance          { scope: "workers" | "scheduler" | "all" }
GET    /admin/v1/maintenance/status
```

Status response:
```json
{
  "workers_paused": true,
  "scheduler_paused": true,
  "worker_in_flight": 3,
  "estimated_drain_seconds": 8,
  "queue_depths": {
    "critical": 0,
    "high": 12,
    "default": 847,
    "slow": 203
  }
}
```

`estimated_drain_seconds` is computed from `worker_in_flight * avg_delivery_latency_p95` (from recent Prometheus metrics). This gives operators a signal before they commit to the maintenance window.

### Difference from SIGTERM

| Signal | Worker exits? | In-flight completion | Recovery |
|---|---|---|---|
| SIGTERM (graceful shutdown) | Yes (after drain) | Yes (30s deadline) | Restart required |
| Maintenance flag | No | Yes (no deadline) | Flag clear → resume in 2s |

SIGTERM is for deployments and crashes. Maintenance flag is for planned operational windows where the process must stay alive.

## Principles upheld

- **Reliability through simplicity** — a Redis flag and a 2-second poll loop require no new infrastructure; processes stay alive and resume without re-registration
- **Two-person operations ceiling** — a solo operator can pause, perform maintenance, and resume without coordinating across services; the status endpoint shows exactly what is in-flight before they begin
- **Observability for everyone** — the maintenance state is visible via the admin API; queue depths and in-flight counts are surfaced so the operator knows when it is safe to proceed
- **Automation and self-healing** — if Redis is available, the flag is the control plane; the PostgreSQL fallback means a Redis-down maintenance event (the common case for Redis upgrades) still has a control mechanism

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| SIGTERM + restart | Process restart adds 10–30 seconds of unavailability; in-flight requests that survive the graceful drain deadline are aborted; unnecessary for planned maintenance |
| Kubernetes rolling restart | Same as SIGTERM + restart at the deployment level; adds orchestration complexity for what is a simple flag |
| Manual stop/start of individual pods | Not reliably atomic; some pods may be restarting while others are still processing; no clear "all workers idle" signal |
| Feature flag service (LaunchDarkly, Flagsmith) | External dependency for a core operational feature; a Redis key is simpler, faster, and always available when Redis is available |

## Consequences

**Positive:**
- Workers and scheduler pause within 2–5 seconds of the flag being set
- No process restarts — zero cold-start overhead on resume
- The status API gives operators confidence before and during maintenance
- PostgreSQL fallback means Redis upgrade (the most common use case for maintenance mode) can itself be performed safely

**Negative:**
- If both Redis and PostgreSQL are unavailable simultaneously, the maintenance flag cannot be read; workers will continue processing (fail-open); this scenario should not occur in practice as they cannot work without PostgreSQL anyway
- The 2-second polling interval means workers may start one additional job after the flag is set; operators should wait for `worker_in_flight == 0` before beginning maintenance
