# ADR delivery/004: Retry policy and dead-letter design

## Status
Accepted

## Context

Webhook endpoints fail. Network errors, deployment restarts, and misconfigured endpoints are the normal operating environment for a webhook delivery platform. The retry policy determines how Hookly behaves in the face of these failures — how long it waits before retrying, how many times it tries, and what happens when it gives up.

Two competing concerns:
- **Tenant reliability**: a transient failure should not permanently drop a delivery; retrying with increasing delays gives the endpoint time to recover
- **System health**: retrying aggressively against a broken endpoint wastes worker slots and compounds load on a system already under stress

Redis Streams has no native message delay. A message added to a stream is immediately visible to consumers. Delayed retries (backoff) require a separate mechanism.

## Decision

### Backoff schedule

Exponential backoff with fixed delays, per-endpoint configurable `max_retries` (default: 6):

| Attempt | Delay before retry |
|---|---|
| 1 (immediate) | 0 — first attempt is immediate after enqueue |
| 2 | 30 seconds |
| 3 | 5 minutes |
| 4 | 30 minutes |
| 5 | 2 hours |
| ≥ max_retries | dead-lettered |

`next_retry_at` is stored on the `delivery_jobs` row. `max_retries` is configurable per endpoint (stored on the `endpoints` table).

### 4xx responses

Non-retryable: a 4xx response (excluding 429) indicates a client-side configuration error (invalid URL, missing auth, malformed payload). Retrying will not succeed. The job is immediately dead-lettered. The tenant must fix their endpoint configuration and use the manual retry API.

429 is handled separately — see [ADR delivery/007](007-rate-limiting.md).

### Delayed requeue mechanism

Redis Streams has no native visibility timeout or message delay. Delayed retries use a separate sorted set:

```
hookly:delayed   sorted set
  score = deliver_at_unix_timestamp
  member = serialized {delivery_job_id, queue, priority}
```

A **promoter task** (a single lightweight Tokio task inside the worker binary) runs every second:
```
ZRANGEBYSCORE hookly:delayed 0 {now_unix} LIMIT 100
→ XADD hookly:delivery:{priority} {job}  (pipelined)
→ ZREM hookly:delayed {members}
```

This keeps delayed jobs out of the main queue until they are ready, preventing workers from spinning on jobs that cannot yet be retried.

### Dead-letter handling

Dead-lettered jobs are not stored in a separate Redis structure — they are a state on the `delivery_jobs` row (`status = 'dead_lettered'`). No Redis entry is maintained. The DB row holds the full audit trail (all attempt timestamps, response codes, error messages).

Manual retry via `POST /api/v1/delivery-jobs/{id}/retry`:
1. Resets `attempts = 0`, `status = 'pending'`, `next_retry_at = NULL`
2. Inserts a new outbox entry
3. The job is enqueued to `hookly:delivery:critical` for immediate processing

## Principles upheld

- **Reliability through simplicity** — the backoff schedule is a fixed table, not a formula with configurable parameters that operators must reason about; the promoter task is a single read-loop with no coordination requirements
- **Observability for everyone** — every retry attempt is recorded in `delivery_attempts` with response code, latency, and error message; tenants can see exactly why their webhook failed and when the next retry will occur
- **Automation and self-healing** — retries are automatic up to `max_retries`; dead-lettered jobs surface in the tenant dashboard and can be bulk-retried without operator involvement

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Jitter added to each backoff delay | Reduces thundering herd on recovery but complicates the tenant-visible "next retry at" time; predictable delays are better for developer experience |
| Re-enqueue to main stream with metadata (no delayed sorted set) | Worker would dequeue the message immediately, check `next_retry_at`, and re-enqueue in a tight loop — wastes worker slots and hammers Redis |
| PostgreSQL polling for retries (no Redis sorted set) | Adds DB polling load; Redis sorted set ZRANGEBYSCORE is O(log N + M) and significantly cheaper per operation |
| Separate dead-letter Redis stream | Dead-lettered jobs need DB-level metadata (attempt history, endpoint details); a separate stream adds complexity with no benefit over DB state + manual retry API |
| Platform-wide fixed max_retries | Enterprise customers may need higher retry counts for critical events; free tier may warrant fewer retries to limit resource consumption |

## Consequences

**Positive:**
- Predictable, auditable retry schedule that tenants can reason about
- 4xx immediate dead-letter prevents wasted retries on configuration errors
- Per-endpoint `max_retries` allows tier-differentiated SLAs
- Promoter task is a single lightweight loop — no additional infrastructure

**Negative:**
- Maximum delivery window is ~2.5 hours (0 + 30s + 5m + 30m + 2h) at default settings; events older than this are dead-lettered if the endpoint never recovers
- The delayed sorted set is a single Redis key — a hot key for high-volume systems; at very high retry volumes, consider sharding by minute bucket
- Promoter task is a single point of failure within the worker binary; if it hangs, retries stall (mitigated by process restart and the recovery task)
