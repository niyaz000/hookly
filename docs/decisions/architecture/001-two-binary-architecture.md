# ADR architecture/001: Two-binary architecture — API server and delivery worker

## Status
Accepted

## Context

Webhook delivery has two fundamentally different runtime profiles under the same workload:

- **API server**: short-lived request-response, latency-sensitive, CPU-light, scales with HTTP concurrency
- **Delivery worker**: long-lived async tasks, IO-bound (outbound HTTP), tolerates latency, scales with throughput

Running both in a single binary means accepting the least-favourable configuration for both: the API server would carry the overhead of delivery goroutines competing for CPU and connection pool slots, and the worker could not be scaled independently of API capacity.

A single-binary design also makes rolling deploys risky — deploying a change to the delivery worker requires restarting the API server, which serves live HTTP traffic.

## Decision

Hookly runs as two separate binaries sharing a PostgreSQL database and Redis:

| Binary | Rust crate | Role |
|---|---|---|
| `hookly` | `src/main.rs` | REST API — handles all HTTP traffic; writes events and outbox entries |
| `hookly-worker` | `src/worker/main.rs` | Delivery — drains outbox, dequeues jobs, delivers webhooks, manages retries |

The two processes have **no direct network connection**. Redis is the only coupling point for job handoff. PostgreSQL is the source of truth for all durable state.

Both binaries share the same Rust workspace and the same `AppState` type, keeping business logic in one place while allowing independent deployment.

## Principles upheld

- **Frugality** — each binary is sized and scaled independently; paying for API capacity does not mean paying for worker capacity and vice versa
- **Reliability through simplicity** — a deploy of the worker does not restart the API server; a worker crash does not affect HTTP traffic
- **Two-person operations ceiling** — a single operator can deploy, restart, and scale either binary without coordinating with another team; both binaries share the same codebase and config conventions

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Single binary (API + worker in same process) | Rolling deploys restart both; resource contention between HTTP and async delivery tasks; cannot scale independently |
| Three binaries (API + scheduler + worker merged as one) | Scheduler has a distinct resource profile again; see ADR architecture/002 |
| Sidecar model (delivery worker as sidecar per API pod) | Wastes worker capacity proportional to API replicas; delivery should scale on queue depth, not HTTP request rate |

## Consequences

**Positive:**
- API server restarts do not interrupt in-flight deliveries
- Worker can be scaled to zero during off-peak without affecting API availability
- Each binary has a clear, single responsibility — easier to reason about, test, and operate
- Independent deployment allows the worker to be upgraded without touching the API serving path

**Negative:**
- Two binaries to build, deploy, monitor, and keep in sync
- Shared database schema changes must be backward-compatible with both old API and new worker (or vice versa) during the deployment window
