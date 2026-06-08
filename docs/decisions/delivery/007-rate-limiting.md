# ADR delivery/007: Per-endpoint and per-tenant rate limiting

## Status
Accepted

## Context

Tenant webhook endpoints enforce their own rate limits and signal them via HTTP 429 responses, optionally with a `Retry-After` header. Without rate-limit awareness, Hookly would continue dequeuing and attempting delivery against a rate-limited endpoint, consuming worker slots only to receive 429 responses, re-enqueue the job, and immediately pick it up again in a tight loop.

Two distinct limits need enforcement:

1. **Endpoint-level rate limit** — the endpoint explicitly signalled "stop sending for X seconds" via 429/Retry-After. Respect the signal; do not send more to this endpoint until the window expires.
2. **Tenant global concurrency cap** — a platform-level limit on simultaneous in-flight deliveries across all of a tenant's endpoints. Prevents a single tenant from consuming the majority of worker slots during a burst and degrading delivery for other tenants.
3. **Endpoint concurrency cap** — a per-endpoint limit on simultaneous in-flight requests. Protects endpoints that cannot handle concurrent webhooks (e.g., single-threaded listeners).

## Decision

### Endpoint-level rate limit (driven by 429 response)

On receiving a 429 response:
```
Retry-After header present:  SET ratelimit:ep:{endpoint_id} blocked EX {retry_after_seconds}
No Retry-After header:       SET ratelimit:ep:{endpoint_id} blocked EX 60  (default backoff)
```

All pending jobs for this endpoint skip immediately once the endpoint state cache reflects the blocked state (within 100ms of the 429 being received).

A skipped job costs 2 Redis writes (ZADD delayed + XACK) and no worker slot beyond microseconds.

### Per-tenant global concurrency cap

```
Redis key:   inflight:tenant:{tenant_id}   INT counter
Max value:   stored on tenants.max_concurrent_deliveries (default: 50)
```

Before starting any delivery for a tenant, the worker atomically checks and increments (Lua script):
```lua
local current = redis.call("get", KEYS[1])
if current and tonumber(current) >= tonumber(ARGV[1]) then return 0 end
redis.call("incr", KEYS[1])
redis.call("expire", KEYS[1], 300)
return 1
```
If the cap is reached: job is pushed to `hookly:delayed` with a short delay (5 seconds), and `XACK` is called. This prevents thundering-herd re-enqueue when the cap is just barely saturated.

### Per-endpoint concurrency cap

```
Redis key:   inflight:ep:{endpoint_id}   INT counter
Max value:   stored on endpoints.max_concurrent_deliveries (default: 5)
```

Same Lua check-and-increment pattern. Same short-delay requeue on cap hit.

### In-memory endpoint state cache

All three blocking states (rate-limited, max-inflight-tenant, max-inflight-endpoint) are reflected in a process-local `DashMap<EndpointId, BlockedUntil>` refreshed every 100ms from Redis State. The check before dequeue is O(1) with no network round trip.

After a 429, the worker that received it updates the local cache immediately before writing to Redis, so the effect is instantaneous within that worker process and propagates to other workers within one cache cycle.

## Principles upheld

- **Tenant isolation** — one tenant cannot consume all worker slots by flooding a burst of events; the per-tenant concurrency cap ensures fair allocation across tenants
- **Frugality** — the in-memory cache eliminates a Redis round trip per job for the blocked-endpoint check; the skip path costs 2 pipelined Redis writes with no worker slot consumption
- **Performance as a first-class concern** — the Lua atomic check-and-increment prevents the race condition that would require a slower compare-and-swap retry loop
- **Developer experience** — tenants can observe their rate limit and inflight state via the API; the `Retry-After` value is surfaced in the delivery attempt record

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Token bucket rate limiter in Redis | Overkill for webhook delivery — we are respecting the endpoint's own signal, not enforcing a Hookly-defined rate; a simple TTL key is sufficient |
| Per-tenant rate limit only (no per-endpoint) | A tenant with 50 endpoints where one is slow would be throttled globally; per-endpoint is the correct isolation boundary |
| Global worker-level rate limit | Too coarse — a broken endpoint from tenant A should not slow delivery for tenant B |
| Re-check Redis on every dequeue (no local cache) | At 200 concurrent workers each processing one job: 200 Redis round trips per dequeue cycle; at 10,000 jobs/sec, this is 10,000 unnecessary Redis reads/sec |

## Consequences

**Positive:**
- Endpoints that signal rate limits are respected immediately; no wasted delivery attempts
- Tenant concurrency caps enforce fair sharing without operator intervention
- Endpoint state cache makes the hot path (unblocked endpoints) zero-cost at the check level

**Negative:**
- The 100ms cache refresh window means a rate limit set on one worker takes up to 100ms to propagate to other workers; at most one additional delivery attempt per worker per endpoint within that window
- Per-tenant and per-endpoint inflight counters in Redis add two writes per job (increment on start, decrement on completion); at high throughput this is measurable Redis write load
- Counter leak risk: if a worker crashes after incrementing but before decrementing, the counter is inflated until the 300s TTL expires; this is the intended safety net and is acceptable
