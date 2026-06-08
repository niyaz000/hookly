# ADR delivery/003: At-least-once delivery semantics

## Status
Accepted

## Context

Webhook delivery happens over unreliable networks to external HTTP endpoints. Any of the following can occur after a worker starts processing a job:

- The worker process crashes after making the HTTP call but before acknowledging the message
- The network drops during the HTTP call, leaving the outcome unknown
- The worker is SIGKILL'd during deployment before it can XACK

The question is not whether these events will occur — they will — but how the system should behave when they do.

Two delivery semantics exist:

- **At-most-once**: acknowledge the message before attempting delivery. A crash after ACK means the job is gone — the endpoint may never be called.
- **At-least-once**: acknowledge only after delivery succeeds or the job is explicitly dead-lettered. A crash means the job is re-delivered — the endpoint may be called more than once.

Exactly-once delivery over an external HTTP call is not achievable without support from the receiving endpoint (idempotency). The choice is between at-most-once (accepts loss) and at-least-once (accepts duplicates).

## Decision

Hookly guarantees **at-least-once delivery**. A message is `XACK`'d only after the delivery outcome is determined and recorded. A worker crash mid-delivery leaves the message in the Redis Streams Pending Entries List (PEL). After a configurable idle timeout (default: 90 seconds), `XAUTOCLAIM` re-assigns the message to another worker instance for re-delivery.

The `X-Hookly-Delivery` header on every outbound webhook request carries the delivery attempt ID (a UUIDv7). Tenants use this ID to deduplicate re-deliveries on their side. This is documented as the tenant's responsibility and the expected contract.

The outbox pattern (see [ADR architecture/003](../architecture/003-outbox-pattern.md)) extends this guarantee to the enqueue step: a job is not considered enqueued until it is in the outbox table, which is drained to Redis atomically.

Worker crash recovery: a recovery task scans for `delivery_jobs WHERE status = 'delivering' AND updated_at < NOW() - INTERVAL '5 minutes'` and re-enqueues them via the outbox. This covers the case where both the worker and Redis crashed simultaneously, destroying the PEL.

## Principles upheld

- **Reliability through simplicity** — at-least-once is implemented entirely by Redis Streams' built-in PEL mechanism and `XAUTOCLAIM`; no novel consensus protocol required
- **Observability for everyone** — the `X-Hookly-Delivery` ID is surfaced to tenants explicitly, giving them the deduplication key; the system is honest about its delivery semantics
- **Developer experience** — the delivery ID convention follows the pattern of Stripe and Svix; developers familiar with those platforms understand it immediately

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| At-most-once (XACK before delivery) | Delivery loss is silent and unrecoverable; unacceptable for a reliability-first webhook platform |
| Exactly-once via distributed transaction | Requires the receiving HTTP endpoint to participate in a two-phase protocol; external endpoints cannot be controlled; impractical |
| Exactly-once via idempotency keys on the worker side | Storing per-attempt idempotency state in the worker and coordinating with the endpoint is equivalent complexity to asking tenants to deduplicate — and removes the choice from them |

## Consequences

**Positive:**
- A worker crash never permanently loses a delivery job
- The PEL provides a built-in view of in-flight messages
- Tenants have a clear, stable deduplication key (`X-Hookly-Delivery`) they can rely on

**Negative:**
- Tenants must implement deduplication logic if their endpoint is not idempotent; this is documented but is an integration burden
- A very slow endpoint (near the HTTP timeout) followed by a worker restart can result in the endpoint being called twice — once by the crashing worker, once by the recovery worker
