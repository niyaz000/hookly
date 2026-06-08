# ADR delivery/009: Tenant tiering and dedicated queues

## Status
Accepted

## Context

Hookly serves tenants across a range of scale and criticality:

- **Free / new signups**: low volume, tolerant of shared resources, price-sensitive
- **Growth / professional**: moderate volume, expect reasonable throughput guarantees
- **Enterprise dedicated**: high volume, contractual SLA, must be isolated from other tenants' behaviour

Without explicit tiering, a single tenant generating a burst of high-volume events can saturate the shared delivery queue, degrading delivery latency for every other tenant on the same cluster. This violates the core principle that "one bad actor cannot wreck others."

The solution must also allow enterprise customers to be deployed on entirely separate infrastructure without changing the application code.

## Decision

### Queue naming encodes tier and tenant identity

```
Dedicated enterprise tenant:
  hookly:delivery:{tenant_id}:critical
  hookly:delivery:{tenant_id}:high
  hookly:delivery:{tenant_id}:default
  hookly:delivery:{tenant_id}:slow

Shared tier (growth, free):
  hookly:delivery:growth:high
  hookly:delivery:growth:default
  hookly:delivery:free:high
  hookly:delivery:free:default
```

Queue selection at enqueue time is determined by the tenant's `tier` and `cluster` columns:

```rust
fn queue_for(tenant: &Tenant, priority: Priority) -> String {
    match tenant.tier {
        Tier::EnterpriseDedicated => format!("hookly:delivery:{}:{}", tenant.id, priority),
        Tier::Growth => format!("hookly:delivery:growth:{}", priority),
        Tier::Free   => format!("hookly:delivery:free:{}", priority),
    }
}
```

### Worker pool assignment via configuration

Workers are configured with the set of queue name patterns they consume from. This is the only thing that differs between a dedicated-enterprise worker fleet and a shared-tier worker fleet — the binary is identical:

```toml
# Enterprise dedicated worker (runs on isolated fleet)
[pool]
slots = 200
queues = ["hookly:delivery:tenant_acme:*", "hookly:delivery:tenant_globex:*"]

# Shared growth worker
[pool]
slots = 300
queues = ["hookly:delivery:growth:*"]

# Shared free-tier worker
[pool]
slots = 100
queues = ["hookly:delivery:free:*"]
```

### Cluster-level isolation (deployment model)

| Cluster | Tenants | Redis | Worker fleet | PostgreSQL |
|---|---|---|---|---|
| `cluster-enterprise` | Named enterprise tenants | Dedicated Redis instance | Dedicated fleet | Shared PG primary, dedicated read replica |
| `cluster-growth` | Growth / Pro tenants | Shared Redis | Shared pool (higher resources) | Shared PG |
| `cluster-free` | Free tier, new signups | Shared Redis | Shared pool (resource-constrained) | Shared PG |

The API server, scheduler, and worker all run identically across clusters — the difference is configuration: which Redis, which DB pool sizes, which queue patterns, how many slots.

### Tenant migration between tiers

When a tenant upgrades (e.g., free → growth → enterprise):
1. Update `tenants.tier` in the database
2. New delivery jobs are enqueued to the new tier's queues immediately
3. Old tier queue drains naturally at its own pace (existing jobs are not moved)
4. Monitor drain with `hookly_queue_depth{queue="hookly:delivery:free:*"}` → 0
5. No application code change; no coordination between services required

## Principles upheld

- **Tenant isolation** — one tenant's burst cannot degrade another's delivery; enterprise tenants have dedicated queues, dedicated workers, and optionally dedicated infrastructure
- **Frugality** — the binary is identical across all tiers; the tiering is pure configuration; no additional code paths, no feature flags in the delivery logic
- **Two-person operations ceiling** — adding a new enterprise tenant is: update the DB row, add queue patterns to the worker config, deploy; no schema changes, no code changes
- **Reliability through simplicity** — queue naming is deterministic from tenant tier; workers are stateless and interchangeable; cluster isolation is infrastructure configuration, not application complexity

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Single shared queue for all tenants | One high-volume tenant degrades all others; violates the tenant isolation principle |
| Per-tenant queues for all tiers | Creates thousands of Redis streams for free-tier tenants; XLEN scanning across thousands of streams is expensive; management overhead grows with tenant count |
| Separate application codebase per tier | Code drift, deployment complexity, inability to share fixes; the principle is "same code, different config" |
| Rate-limit tenants instead of separating queues | Rate limiting reduces throughput for the offending tenant but doesn't give other tenants a guaranteed floor |

## Consequences

**Positive:**
- Enterprise tenants are completely isolated from free-tier noise
- Adding a new enterprise tenant requires no code change — only config and a DB update
- Cluster-level isolation satisfies enterprise security requirements (dedicated Redis, dedicated workers)
- Worker fleets scale independently per tier based on queue depth, not tenant count

**Negative:**
- Dedicated enterprise queues mean the worker must consume from many stream patterns — `XREADGROUP` must be called per pattern, not per stream; this is the expected cost of per-tenant isolation
- Tenant migration leaves the old queue to drain; during the drain window, jobs may be delivered from different worker fleets (acceptable — delivery order is not guaranteed across tiers anyway)
- New enterprise tenant onboarding requires a worker config change and redeploy (adding the new queue pattern); this is operationally lightweight but not zero-touch
