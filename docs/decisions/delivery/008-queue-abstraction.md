# ADR delivery/008: Queue backend abstraction

## Status
Accepted

## Context

Redis Streams is the delivery queue backend today. However, several deployment scenarios require alternative backends:

- A customer running on AWS may want to use SQS to avoid a self-managed Redis dependency
- A customer on GCP may prefer Pub/Sub
- High-throughput deployments may benefit from Kafka or NATS JetStream
- Managed Redis alternatives (Upstash, Redis Cloud) have different connection semantics

The `vision.md` principle of "vendor lock-in at the infrastructure layer is not a constraint we accept" means the delivery pipeline should not be coupled to Redis-specific APIs throughout the codebase.

At the same time, building five queue implementations on day one violates the "minimal external dependencies" and "frugality" principles. The right trade-off is a thin abstraction that Redis implements today, with the interface designed so that SQS, Pub/Sub, Kafka, or NATS can be added as new implementations without changing any business logic.

## Decision

A `Queue` trait is defined in `src/common/queue.rs`. All delivery pipeline code interacts only with `Arc<dyn Queue>`:

```rust
#[async_trait]
pub trait Queue: Send + Sync {
    /// Enqueue a job for delivery.
    async fn enqueue(&self, queue: &str, msg: &QueueMessage) -> Result<MessageId>;

    /// Dequeue up to `count` jobs from a queue for processing.
    /// Blocks up to `timeout` if the queue is empty.
    async fn dequeue(
        &self,
        queue: &str,
        group: &str,
        consumer: &str,
        count: usize,
        timeout: Duration,
    ) -> Result<Vec<ReceivedMessage>>;

    /// Acknowledge successful processing of a message.
    async fn ack(&self, queue: &str, id: &MessageId) -> Result<()>;

    /// Return the number of messages waiting in a queue.
    async fn depth(&self, queue: &str) -> Result<u64>;

    /// Return the number of messages currently in-flight (dequeued, not yet acked).
    async fn pending_count(&self, queue: &str, group: &str) -> Result<u64>;
}
```

`RedisQueue` is the only implementation today. It wraps Redis Streams (`XADD`, `XREADGROUP`, `XACK`, `XLEN`, `XPENDING`).

The delayed requeue mechanism (`hookly:delayed` sorted set + promoter task) is implemented **inside `RedisQueue`** as a Redis-specific detail. Alternative backends that have native delay support (SQS visibility timeout, NATS JetStream delay) will implement delay inside their own `Queue` implementation — the business logic never calls the sorted set directly.

`AppRedis` holds `queue: Arc<dyn Queue>` alongside the other Redis role clients:

```rust
pub struct AppRedis {
    pub queue:     Arc<dyn Queue>,   // RedisQueue today; SqsQueue tomorrow
    pub state:     RedisClient,      // Rate limits, CB, inflight — always Redis
    pub scheduler: RedisClient,      // Sorted sets — always Redis
    pub ephemeral: RedisClient,      // Idempotency, cache — always Redis
}
```

The state, scheduler, and ephemeral roles remain Redis-specific; their operations (sorted sets, TTL keys, Lua scripts) do not have a generic abstraction because they are Redis-native and do not need to be swapped. Only the queue role benefits from the abstraction.

## Principles upheld

- **Minimal external dependencies** — no SQS SDK, Kafka client, or NATS client is added until a second backend is needed; the abstraction costs one trait and one impl file
- **Reliability through simplicity** — the abstraction is thin; business logic does not know or care which queue backend is active; adding a new backend is a new file, not a refactor
- **Two-person operations ceiling** — swapping a queue backend is a config change (`QUEUE_BACKEND=sqs`) plus a new implementation; it does not require touching delivery logic, retry logic, or worker pool code

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Couple directly to Redis Streams throughout | Fast to build now but creates a global refactor when a second backend is needed; violates the no-vendor-lock-in principle |
| Abstract everything including state Redis | Over-abstraction — CB, rate limits, and sorted sets are Redis-native patterns; abstracting them adds indirection for no current benefit |
| Use a message broker abstraction library (e.g., `lapin` for AMQP, `rdkafka`) | Third-party abstractions impose their own API shapes and connection models; owning a thin internal trait is less coupling, not more |
| Build all backends now | Violates frugality; unimplemented backends are dead code and unmaintained; the abstraction design is sufficient for future additions |

## Consequences

**Positive:**
- Adding SQS, GCP Pub/Sub, Kafka, or NATS requires writing one new `impl Queue` file and one config variant — no business logic changes
- Tests can use an `InMemoryQueue` implementation for unit testing without a Redis instance
- Queue backend can differ per deployment cluster (enterprise on SQS, shared on Redis) with identical application code

**Negative:**
- The trait imposes one layer of dynamic dispatch per queue operation (a `Box<dyn Queue>` call); negligible overhead compared to the Redis network round trip
- Delayed requeue semantics must be re-implemented per backend (sorted set for Redis, visibility timeout for SQS); this is the expected cost of the abstraction
