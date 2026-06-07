# ADR-002: Redis Streams for the delivery queue

## Status
Accepted

## Context

Webhook delivery is inherently asynchronous: an API handler should acknowledge receipt of an event immediately and not block on the (potentially slow, potentially failing) HTTP call to a tenant's endpoint. A queue decouples event emission from delivery.

Options considered:

| Option | Pros | Cons |
|---|---|---|
| PostgreSQL `LISTEN/NOTIFY` | No new infrastructure; same DB | No persistence across restarts; fan-out is awkward; no consumer groups |
| PostgreSQL outbox table | Durable; transactional emit | Polling overhead; requires careful cleanup; additional migration work |
| Redis Pub/Sub | Simple API | Fire-and-forget; no persistence; messages dropped if consumer is down |
| **Redis Streams** | Persistent; consumer groups; ack/nack; at-least-once; already in the stack | Requires Redis to be available; not transactional with PG |
| Kafka / NATS | High throughput; exactly-once options | Operational overhead; overkill for current scale |

Redis was already a dependency (for caching and rate limiting headroom), so Redis Streams adds no new infrastructure component.

## Decision

Use Redis Streams (`XADD` / `XREADGROUP`) as the delivery queue. The stream key is tiered by delivery priority, allowing urgent events to be processed before low-priority bulk events. Consumer groups provide at-least-once delivery with explicit acknowledgement (`XACK`).

The worker binary (`src/worker/main.rs`) reads from these streams and processes delivery jobs independently from the API server. Both can be scaled horizontally.

Stream names follow the pattern `hookly:delivery:<tier>` (e.g., `hookly:delivery:high`, `hookly:delivery:default`). At startup, the API server ensures each stream's consumer group exists (`XGROUP CREATE ... MKSTREAM`).

## Consequences

**Positive:**
- At-least-once delivery guarantee — a failed worker restart does not lose pending messages
- Independent scaling of API server and delivery worker
- Consumer groups enable multiple worker replicas with no double-processing
- Dead-letter handling via `XPENDING` + `XCLAIM` for messages that exceed retry limits
- Tiered priority without Kafka partitioning complexity

**Negative:**
- Redis is now a critical-path dependency for event delivery (not just a cache)
- No transactional guarantee between a PostgreSQL write and the Redis `XADD` — a process crash between the two can produce a lost event (mitigated by the outbox pattern if needed in future)
- Payload size is limited by Redis memory; large event payloads should store data in PostgreSQL and pass a reference ID on the stream
