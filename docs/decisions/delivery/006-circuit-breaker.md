# ADR delivery/006: Per-endpoint circuit breaker

## Status
Accepted

## Context

Rate limiting (see [ADR delivery/007](007-rate-limiting.md)) handles explicit signaling from an endpoint: the endpoint returns 429 and tells Hookly to back off. But endpoints also fail silently — returning 5xx, dropping connections, or timing out — without sending a 429.

Without a circuit breaker, the retry policy (see [ADR delivery/004](004-retry-policy-dead-letter.md)) means Hookly will continue attempting delivery with exponential backoff until `max_retries` is exhausted. For a sustained outage, this means thousands of delivery attempts hitting a broken endpoint over hours, consuming worker slots and generating noise in the tenant's audit trail.

The circuit breaker pattern addresses this: after N consecutive failures, stop trying. Probe periodically. Resume when the endpoint recovers. This reduces load on both the failing endpoint and the Hookly worker.

## Decision

Each endpoint has a per-endpoint circuit breaker with three states: **CLOSED**, **OPEN**, and **HALF_OPEN**.

### State machine

```
CLOSED  →  (failures ≥ threshold within window)  →  OPEN
OPEN    →  (probe_interval elapsed)               →  HALF_OPEN
HALF_OPEN  →  (probe succeeds)  →  CLOSED  (failure counter reset)
HALF_OPEN  →  (probe fails)     →  OPEN    (probe timer reset)
```

### Redis keys

```
cb:{endpoint_id}:failures   INT, TTL = failure_window_s  (default: 300)
cb:{endpoint_id}:state      "open" | "half_open"         (absent = CLOSED)
cb:{endpoint_id}:opens_at   unix timestamp when OPEN was entered
```

### Default configuration (per endpoint, overridable)

| Parameter | Default | Meaning |
|---|---|---|
| `cb_failure_threshold` | 5 | consecutive failures to open |
| `cb_probe_interval_s` | 60 | seconds in OPEN before probing |
| `cb_failure_window_s` | 300 | TTL on failure counter (auto-resets if no failures for this long) |

### What counts as a failure

- 5xx response (server error)
- HTTP connection timeout
- HTTP read timeout

**Does not count as a failure:**
- 4xx responses (including 429) — these are client-side signals, not server health indicators
- 2xx responses with unexpected bodies — the endpoint is reachable; content validation is the tenant's concern

### OPEN state behaviour

When a job is dequeued and the endpoint state cache indicates OPEN:
- No HTTP attempt is made
- The job is pushed to `hookly:delayed` with `deliver_at = now + probe_interval_s`
- `XACK` is called — the message is consumed, not re-queued in the stream
- Worker slot is released immediately

The endpoint state cache (DashMap, refreshed every 100ms) propagates the OPEN state to all worker tasks within one cache cycle, preventing the thundering herd of jobs hitting the circuit check simultaneously.

### HALF_OPEN probe

When the probe interval elapses, the promoter task re-enqueues the job to `hookly:delivery:{tier}:critical` (highest priority, to minimise probe latency). Exactly one probe is allowed through. The worker uses a Redis `SET NX` to claim the probe slot:
```
SET cb:{endpoint_id}:probing 1 EX {probe_interval_s}  NX
```
If `NX` fails, another worker already has the probe — all other jobs for this endpoint are re-delayed.

## Principles upheld

- **Tenant isolation** — a broken endpoint consumes at most one probe slot per minute in OPEN state; it cannot monopolise worker capacity through repeated failed retries
- **Reliability through simplicity** — three states, Redis keys with TTLs, a DashMap cache; no novel distributed systems primitive
- **Observability for everyone** — `cb:{endpoint_id}:state` is queryable; the API surfaces circuit breaker state on the endpoint resource; `hookly_circuit_breaker_open_endpoints_total` is a Prometheus metric
- **Automation and self-healing** — the circuit automatically probes and closes without operator intervention; no manual "un-pause endpoint" step required

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Exponential backoff only (no circuit breaker) | Does not prevent the worker from continuing to attempt delivery during a sustained outage; a max_retries of 6 over 2.5 hours is 6 failed attempts — acceptable for short outages but poor signal for operators |
| Per-tenant circuit breaker (not per-endpoint) | Too coarse — a tenant may have 50 endpoints; one broken endpoint should not circuit-break the other 49 |
| Circuit breaker in application memory only | State is lost on worker restart; the circuit would re-enter CLOSED after every restart even if the endpoint is still down |
| Hystrix-style half-open with percentage traffic | Adds complexity (tracking percentage, splitting the stream); a single probe is simpler and sufficient |

## Consequences

**Positive:**
- A sustained endpoint outage produces one probe attempt per minute instead of continuous retries
- Worker slots are not consumed by a known-broken endpoint
- Endpoint recovery is automatic — no operator action required once the endpoint comes back up

**Negative:**
- A newly deployed endpoint that starts returning 5xx will be circuit-broken after 5 attempts; the tenant must wait `probe_interval_s` (default 60s) before the next delivery attempt, even if they fix the endpoint immediately
- The probe claim (`SET NX`) means only one worker can run the probe; if that worker crashes mid-probe, the probe is effectively delayed until the `SET NX` key expires
