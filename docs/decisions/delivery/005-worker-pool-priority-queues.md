# ADR delivery/005: Work-stealing worker pool and priority queues

## Status
Accepted

## Context

The delivery worker must process jobs from multiple priority levels simultaneously — manual retries (critical), first delivery attempts (high), later retry attempts (default and slow) — without allowing lower-priority work to starve, and without dedicating fixed resources to queues that may be empty most of the time.

Two concurrency models were considered:

**Per-queue dedicated pools**: each queue gets a fixed number of Tokio tasks. Simple to reason about but wastes resources when higher-priority queues are empty and lower-priority queues have backlog. Scaling one queue means reconfiguring all pools.

**Work-stealing shared pool**: a single pool of N Tokio tasks pulls from queues in priority order, with fairness floors to prevent starvation. Higher utilization, more complex to implement but fits the "frugality" principle — idle capacity is reused wherever work exists.

## Decision

### Queue structure

Four priority tiers, each a separate Redis Stream:

| Queue | Stream key pattern | Used for |
|---|---|---|
| critical | `hookly:delivery:{tier}:critical` | Manual retries, security-sensitive events |
| high | `hookly:delivery:{tier}:high` | First delivery attempt |
| default | `hookly:delivery:{tier}:default` | Retry attempts 1–3 |
| slow | `hookly:delivery:{tier}:slow` | Retry attempts 4+ |

The `{tier}` segment encodes the tenant tier (e.g., a tenant ID for dedicated enterprise queues, or a tier name like `growth` for shared queues). See [ADR delivery/009](009-tenant-tiering.md).

### Work-stealing pool

```
Total slots: N  (env: NUM_WORKER_SLOTS, default: 200)

Minimum fairness floors (always reserved):
  critical:  5%  (≥ 10 slots)
  high:      15% (≥ 30 slots)
  default:   40% (≥ 80 slots)
  slow:      10% (≥ 20 slots)
  unassigned: 30% — stolen by whichever queue has work
```

Each worker task loop:
1. Acquire a semaphore slot (capacity = N)
2. **Starvation check**: if any queue's `in_flight` count is below its floor AND that queue has pending items → select that queue first
3. Otherwise: try queues in priority order (critical → high → default → slow)
4. `XREADGROUP` from the selected queue (count: 1)
5. If all queues empty: release slot, `XREADGROUP BLOCK 5000ms` on the highest-priority non-empty queue, re-enter loop
6. Execute job; release slot on completion

### Endpoint state cache

Before any HTTP attempt, each worker task checks a process-local `DashMap<EndpointId, BlockedUntil>`. This cache is refreshed from Redis State every 100ms by a background task.

If the endpoint is blocked (rate-limited, circuit-open, max-inflight reached):
- `ZADD hookly:delayed {unblock_at} {job}` (1 Redis write)
- `XACK` (1 Redis write)
- Slot is released in microseconds — no HTTP attempt, no DB read

This means a burst of 10,000 pending jobs for a rate-limited endpoint costs only 2 Redis round trips per job (pipelined), not a full delivery cycle.

## Principles upheld

- **Frugality** — idle slots are reused across queues; no capacity is permanently reserved for a queue that may be empty; 30% of capacity adapts to demand
- **Performance as a first-class concern** — Tokio tasks are cheap; N=200 concurrent deliveries is achievable with minimal memory overhead (each task is an async state machine, not an OS thread); the in-memory endpoint state cache eliminates per-job Redis round trips for the common case
- **Tenant isolation** — fairness floors prevent critical/manual retries from being starved by a flood of slow retries; the starvation check ensures lower-priority queues always make progress
- **Reliability through simplicity** — the pool is a fixed set of Tokio tasks with a semaphore; no dynamic task spawning, no backpressure propagation to manage

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Per-queue dedicated pools | Wastes capacity when a queue is empty; requires manual rebalancing when queue depths shift; doesn't adapt to workload shape |
| OS thread-per-job | Memory overhead of hundreds of OS threads; Tokio tasks are 10–100× cheaper per concurrent unit |
| Single queue with priority field | Requires reading and discarding low-priority messages to get to high-priority ones; no O(1) priority access |
| Weighted random queue selection | Probabilistic — cannot guarantee fairness floors; a unlucky run can starve a queue for seconds |

## Consequences

**Positive:**
- Critical work (manual retries) is processed within seconds even under a full slow-retry flood
- Worker capacity adapts to actual queue depths — no idle reservation waste
- The endpoint state cache makes blocked-endpoint skips essentially free
- N is a single tuning knob; operators adjust one number, not four

**Negative:**
- The starvation check adds a small coordination overhead per slot acquisition (reading `in_flight` atomics) — negligible in practice
- Work-stealing is harder to reason about than dedicated pools; debugging "why is my slow job still pending?" requires looking at the full pool state, not one queue
- The 30ms cache staleness window means a newly opened circuit breaker or newly set rate limit may let through one more delivery attempt before the cache refreshes
