# ADR database/004: Idempotency storage

## Status
Accepted

## Context

Idempotency key semantics and the client-facing API contract are established in
[api-design/007-idempotency-key.md](../api-design/007-idempotency-key.md). This ADR
covers only the **storage backend** — how the server persists idempotency records so
it can detect and replay duplicates.

Two cross-cutting decisions apply to all candidates:

- **TTL is 1 hour, fixed.** Hookly clients are automated systems — retry windows are
  seconds to minutes, not overnight. 1 hour is more than sufficient and keeps the
  index footprint small. Making it configurable adds an env var and operational surface
  for no real benefit.
- **Scope is events and schedules only.** These are the two entities where a duplicate
  create has an irreversible side effect (webhook fires twice; cron entry registered twice).
  All other POST endpoints ignore the `Idempotency-Key` header.
- **Scope within entity is application_id.** A key is unique per application, not per
  tenant. This mirrors how SQS scopes deduplication to the queue (the application is the
  parent container). A key `"abc"` in app A is independent of `"abc"` in app B.

---

## Core guarantee vs best-effort

**Best-effort:** Store records in Redis. If the Redis node is lost, duplicate requests
in the retry window execute twice.

**Core guarantee:** Store records in PostgreSQL. A Redis crash doesn't affect
completed records — only in-flight concurrent requests lose their lock, and those are
recoverable by the SELECT-before-INSERT check.

Hookly's clients are automated systems. A webhook publisher retrying with the same
idempotency key expects exactly-once semantics regardless of infrastructure events on
our side. We treat idempotency as a **core guarantee**.

---

## What to store

| Approach | Per-record size | Replay cost | Risk |
|---|---|---|---|
| Full JSON response | 750 B – 10 KB | Zero — return cached bytes | Unbounded: scales with payload size |
| `body_hash` + resource reference | ~200 B fixed | One DB read per replay | Current DB state on replay, not original snapshot |

The body-hash + reference approach is fixed-size regardless of payload. Replay reads the
current entity row, which is acceptable — for immutable resources (events) this is
equivalent; for mutable ones the guarantee is "same operation, not same bytes".

---

## Storage cost at 200 000 RPM

Live records = 200 000 × TTL in minutes.

Rates: ElastiCache (r7g) ≈ **$12/GB-month RAM** · RDS gp3 ≈ **$0.12/GB-month disk**.

| TTL | Live records | Redis binary<br>(300 B, RAM) | Redis full resp<br>(2.2 KB, RAM) | PG hash+ref<br>(408 B, disk) | PG full resp<br>(2.5 KB, disk) |
|---|---:|---|---|---|---|
| 1 min  |    200 K | 60 MB · **$0.72**    | 440 MB · **$5.28**     | 82 MB · **$0.01**    | 500 MB · **$0.06**   |
| 5 min  |      1 M | 300 MB · **$3.60**   | 2.2 GB · **$26.40**    | 408 MB · **$0.05**   | 2.5 GB · **$0.30**   |
| 10 min |      2 M | 600 MB · **$7.20**   | 4.4 GB · **$52.80**    | 816 MB · **$0.10**   | 5.0 GB · **$0.60**   |
| 15 min |      3 M | 900 MB · **$10.80**  | 6.6 GB · **$79.20**    | 1.2 GB · **$0.15**   | 7.5 GB · **$0.90**   |
| 30 min |      6 M | 1.8 GB · **$21.60**  | 13.2 GB · **$158.40**  | 2.4 GB · **$0.29**   | 15 GB · **$1.80**    |
| 1 hour |     12 M | 3.6 GB · **$43.20**  | 26.4 GB · **$316.80**  | 4.9 GB · **$0.59**   | 30 GB · **$3.60**    |

Key observations:

1. **RAM vs disk**: Redis costs ~100× more per GB than RDS disk. At 1-hour TTL the
   binary-Redis approach costs $43/month in dedicated RAM; the PG equivalent is $0.59
   on disk.

2. **Full response is a money trap**: at only 5 minutes TTL and 200K RPM, full-response
   Redis is already 2.2 GB RAM / $26/month — and this is for the idempotency store
   alone, not the rest of Redis.

3. **PG full response is still very cheap** on disk: 30 GB / $3.60/month at 1-hour TTL.
   The cost of storing the response in PostgreSQL is not the money problem; it's the
   operational cost of re-serializing and versioning arbitrary JSON blobs.

4. **Candidate 3 (entity columns)** does not appear in this table. The entity row is
   written regardless of idempotency; the only overhead is two extra columns (64 bytes)
   and one sparse index (~80 bytes) per row that carries an idempotency key.

---

## Three candidates

### Candidate 1 — Redis

Completed records are stored in Redis as binary values with a key-level TTL.

```
Key:   idmp:{tenant_id}:{idempotency_key}
Value: CBOR/binary { body_hash: [u8;32], resource_id: str, resource_type: str }
TTL:   IDEMPOTENCY_TTL_SECONDS
```

The distributed lock (`idmp_lock:{key}`, 60s, `SET NX`) guards concurrent in-flight
requests.

**Throughput and latency:**
- Lookup: ~0.1–0.3 ms (single GET)
- Write: ~0.1–0.3 ms (single SET)
- Pruning: automatic — key TTL handles expiry

**Tradeoffs:**

| | |
|---|---|
| + Simplest possible implementation | – Volatile: node crash or eviction loses all records → duplicates execute |
| + Fastest lookup (~0.3 ms) | – RAM is 100× more expensive than disk |
| + No pruning cron | – At 1-hour TTL / 200K RPM: 3.6 GB dedicated RAM / **$43/month** just for idempotency |
| + TTL is a single env var | – Redis must be sized for idempotency load on top of all other uses |

**When Redis evicts:** Under memory pressure Redis evicts keys according to the
configured eviction policy (`allkeys-lru`, `volatile-lru`, etc.). An evicted
idempotency key is indistinguishable from a key that was never written — the next
request with that key executes again. This is a silent correctness failure, not an
observable error.

---

### Candidate 2 — PostgreSQL (separate `idempotency_keys` table)

Completed records are stored in a dedicated `idempotency_keys` table with hourly range
partitions. Redis retains only the distributed lock.

```sql
CREATE TABLE idempotency_keys (
    idempotency_key  VARCHAR(32)  NOT NULL,
    tenant_id        VARCHAR(24)  NOT NULL,
    body_hash        BYTEA        NOT NULL,
    resource_id      VARCHAR(24)  NOT NULL,
    resource_type    TEXT         NOT NULL,
    created_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    expires_at       TIMESTAMPTZ  NOT NULL,
    PRIMARY KEY (tenant_id, idempotency_key, created_at)
) PARTITION BY RANGE (created_at);

CREATE INDEX ON idempotency_keys (tenant_id, idempotency_key, body_hash);
CREATE INDEX ON idempotency_keys (expires_at);
```

`created_at` is in the primary key because PostgreSQL requires the partition key to
appear in any `PRIMARY KEY` or `UNIQUE` constraint on a partitioned table. Logical
uniqueness of `(tenant_id, idempotency_key)` within the active window is enforced at
the application layer by the Redis lock + SELECT-before-INSERT protocol.

**Partitioning and pruning:**

Partitions are created per calendar hour. A record written at 14:30 expires at 15:30.
The last record in the 14:xx partition (14:59:59) expires at 15:59:59.

**Rule: the partition for hour H can be dropped at hour H + 2.**

```sql
DROP TABLE idempotency_keys_2026_06_17_14;  -- instant DDL; no row scan, no vacuum
```

**Partition cron (runs at :55 — pre-creates next-hour partition):**

```sql
DO $$
DECLARE
  next_hour TIMESTAMPTZ := date_trunc('hour', NOW()) + INTERVAL '1 hour';
  tbl       TEXT        := 'idempotency_keys_' || to_char(next_hour, 'YYYY_MM_DD_HH24');
BEGIN
  EXECUTE format(
    'CREATE TABLE IF NOT EXISTS %I PARTITION OF idempotency_keys
         FOR VALUES FROM (%L) TO (%L)',
    tbl, next_hour, next_hour + INTERVAL '1 hour');
END $$;
```

**Prune cron (runs at :05 — drops the H-2 partition):**

```sql
DO $$
DECLARE
  expired TIMESTAMPTZ := date_trunc('hour', NOW()) - INTERVAL '2 hours';
  tbl     TEXT        := 'idempotency_keys_' || to_char(expired, 'YYYY_MM_DD_HH24');
BEGIN
  EXECUTE format('DROP TABLE IF EXISTS %I', tbl);
END $$;
```

**Startup bootstrap (pre-creates current + next 3 hours):**

```sql
DO $$
DECLARE h TIMESTAMPTZ;
BEGIN
  FOR i IN 0..3 LOOP
    h := date_trunc('hour', NOW()) + (i || ' hours')::INTERVAL;
    EXECUTE format(
      'CREATE TABLE IF NOT EXISTS %I PARTITION OF idempotency_keys
           FOR VALUES FROM (%L) TO (%L)',
      'idempotency_keys_' || to_char(h, 'YYYY_MM_DD_HH24'),
      h, h + INTERVAL '1 hour');
  END LOOP;
END $$;
```

**Throughput and latency:**
- Lookup: ~0.5–2 ms (composite-indexed SELECT)
- Write: ~1–3 ms (INSERT into partitioned table)
- At 200K RPM: ~3 333 reads/sec — routine for a well-indexed Postgres table
- Pruning: instant `DROP TABLE` DDL, no vacuum, no bloat

**Replay flow:**
```
1. Acquire Redis lock (concurrent guard)
2. SELECT resource_id, resource_type FROM idempotency_keys
   WHERE tenant_id = $1 AND idempotency_key = $2 AND body_hash = $3
     AND expires_at > NOW()
   → match: re-fetch entity by resource_id + resource_type → return
   → hash mismatch: 409
   → no row: execute handler
3. INSERT idempotency record on success
4. Release Redis lock
```

**Tradeoffs:**

| | |
|---|---|
| + Durable — survives Redis and app crashes | – Partition cron required (create at :55, drop at :05) |
| + Disk is cheap ($0.59/month at 1hr/200K RPM) | – Startup bootstrap script needed |
| + Partition DROP is instant DDL | – `resource_type` routing on replay adds handler coupling |
| + TTL is a single env var | – Redis still required for concurrent lock |
| + Scales independently of entity tables | – Missing partition = INSERT failure until next cron tick |

**TTL configurability and partition granularity:**

Hourly partitions work correctly for any TTL from 1 minute to 1 hour. A 5-minute TTL
means the 14:xx partition holds records that expire between 14:01 and 15:04; it can
be dropped at 16:05. The `expires_at > NOW()` filter excludes expired rows without
a partition scan. Partition granularity does not need to change when TTL changes.

---

### Candidate 3 — Entity-level `idempotency_key` column (chosen)

Add `idempotency_key` and `body_hash` columns directly to the events and schedules tables.
There is no separate idempotency store — the entity row itself is the idempotency record.

```sql
-- Applied to events and schedules tables only
ALTER TABLE events
    ADD COLUMN idempotency_key  VARCHAR(64),
    ADD COLUMN body_hash        BYTEA;

CREATE INDEX idx_events_idempotency
    ON events (tenant_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- Identical pattern for schedules
ALTER TABLE schedules
    ADD COLUMN idempotency_key  VARCHAR(64),
    ADD COLUMN body_hash        BYTEA;

CREATE INDEX idx_schedules_idempotency
    ON schedules (tenant_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
```

**Lookup:**

```sql
SELECT *
FROM   events
WHERE  tenant_id       = $1
  AND  idempotency_key = $2
  AND  created_at      > NOW() - INTERVAL '1 hour'
LIMIT  1;
```

**Insert (idempotency columns populated alongside entity columns):**

```sql
INSERT INTO events (public_id, tenant_id, ..., idempotency_key, body_hash)
VALUES ($1, $2, ..., $key, $hash);
```

**Replay:** The SELECT above returns the entity row directly — there is no re-fetch,
no `resource_type` routing, no second query. The idempotency record is the entity.

**Pruning:** None. Entity rows persist indefinitely (soft-delete via `deleted_at`).
The `created_at > NOW() - TTL` filter in the SELECT excludes expired records at
query time. Old rows are simply ignored — not deleted.

**Storage overhead:** Entity rows are written regardless. Idempotency adds 64 bytes
per row (`idempotency_key` + `body_hash`) and a sparse partial index (~80 bytes per
row that carries a key). This is negligible because entities would occupy that storage
either way.

**Throughput and latency:**
- Lookup: ~0.5–2 ms (partial index on the entity table)
- Write: no extra INSERT — idempotency columns are part of the entity INSERT
- At 200K RPM: same ~3 333 reads/sec as Candidate 2, but against the entity table
  rather than a separate one (entity table is already hot in the buffer pool)

**Tradeoffs:**

| | |
|---|---|
| + No separate table, no partition cron, no startup bootstrap | – Every entity table gets two nullable columns |
| + Zero additional storage cost | – Sparse partial index per entity table |
| + Replay hits no extra query — entity row is the result | – Key scope is per-entity-type: `"abc"` in events and `"abc"` in endpoints are independent |
| + TTL is one env var; no partition granularity decisions | – Redis still required for concurrent lock |
| + Durable (PostgreSQL) | – Nullable columns require `WHERE idempotency_key IS NOT NULL` on index |
| + Key reuse after TTL window works naturally | |

**Key scope per entity type** is actually desirable: an idempotency key submitted to
`POST /events` cannot collide with one submitted to `POST /schedules`, even if the
string is identical. Each entity table enforces its own uniqueness.

---

## Candidate comparison

| Dimension | Redis | PG separate table | Entity column |
|---|---|---|---|
| Durability | Volatile (eviction = silent duplicate) | Durable | Durable |
| Additional storage cost (1hr/200K RPM) | 3.6 GB RAM / **$43/mo** | 4.9 GB disk / **$0.59/mo** | ~0 (entities already stored) |
| Lookup latency | ~0.3 ms | ~1–2 ms | ~1–2 ms |
| Replay mechanism | Return cached value or re-fetch | Re-fetch by resource_id + resource_type | Return entity row directly |
| Pruning | Automatic (key TTL) | Hourly partition DROP cron | None |
| Concurrent guard | Redis lock | Redis lock | Redis lock |
| TTL configurability | Single env var | Single env var (partition size stays hourly) | Single env var |
| Operational complexity | Low | High (partition cron, startup bootstrap) | Low |
| Schema impact | None | Separate table | Two columns per entity table |
| Key reuse after TTL | Yes | Yes | Yes (created_at filter) |

---

## Decision

**Candidate 3 — entity-level `idempotency_key` column.**

The reasons:

1. **Zero incremental storage cost.** Entities are written to PostgreSQL regardless.
   Adding 64 bytes and a sparse index per row is noise.

2. **No operational machinery.** No partition pre-create cron. No prune cron. No
   startup bootstrap. No missing-partition failure mode.

3. **Replay is free.** The SELECT that checks for an existing record returns the full
   entity row. No second query, no `resource_type` dispatch.

4. **Durable by inheritance.** PostgreSQL durability is inherited from the entity
   table — no separate durability story to maintain.

5. **TTL is a single number.** Changing `IDEMPOTENCY_TTL_SECONDS` requires no
   partition schedule recalculation.

---

## Implementation

### Schema (per entity table)

```sql
-- Run for events, endpoints, schedules, invites, etc.
ALTER TABLE events
    ADD COLUMN idempotency_key  VARCHAR(32),
    ADD COLUMN body_hash        BYTEA;

CREATE INDEX idx_events_idempotency
    ON events (tenant_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
```

`idempotency_key` is nullable — rows created without a key carry NULL and are
excluded from the partial index entirely.

`body_hash` is SHA-256 of the canonical (re-serialised) request body, stored as 32
raw bytes. This detects key reuse with a different payload (→ 409) without storing
the payload itself.

### Request flow

```
Incoming request with Idempotency-Key header:

1. Acquire Redis lock  SET NX PX 60000
   ↓ Not acquired → 409 (concurrent request in-flight with same key)

2. SELECT * FROM <entity_table>
   WHERE  tenant_id       = $1
     AND  idempotency_key = $2
     AND  created_at      > NOW() - $ttl::INTERVAL

   Row found, body_hash matches  → release lock, return entity row
   Row found, body_hash mismatch → release lock, 409
   No row                        → proceed

3. Execute handler, INSERT entity with idempotency_key and body_hash populated

4. Release Redis lock (Lua: DEL only if token matches — prevents ABA race)

5. Return response
```

If the handler returns an error, the entity INSERT is rolled back. The key remains
available for retry with the same body — no side effects were committed.

### Abstraction

The idempotency check is performed in the service layer (not middleware) because the
lookup is entity-specific. A shared helper carries the TTL from config and is called
before the entity INSERT in every write service that accepts an `Idempotency-Key`
header.

```rust
pub async fn acquire_lock(redis: &Client, namespace: &str, key: &str) -> Result<String, AppError>;
pub async fn release_lock(redis: &Client, namespace: &str, key: &str, token: &str);
pub fn body_hash_bytes<T: Serialize>(body: &T) -> Vec<u8>;
```

The service layer calls `acquire_lock`, performs the SELECT-before-INSERT, then calls
`release_lock`. No shared closure or middleware — the idempotency check is inlined in
each service's `create` function because the lookup is entity-specific.

### TTL

24 hours, fixed. The `created_at > NOW() - INTERVAL '24 hours'` filter is applied at
query time. No env var, no partition schedule recalculation, no operational surface.

Stripe, Adyen, and Mastercard all use 24-hour windows. Hookly's clients are automated
systems; any retry that would arrive more than 24 hours after the original attempt is
effectively a new operation, not a retry.

---

## Consequences

**Positive:**
- No separate idempotency infrastructure to operate
- Idempotency durability is automatic — same as entity table durability
- Replay returns the entity row directly with no extra query
- TTL is a single, trivially configurable value
- Zero additional storage cost at any throughput level

**Negative:**
- Every entity table that accepts idempotency keys grows by two nullable columns and
  one partial index
- Key scope is per-entity-type — cannot share a key across entity types by design
  (this is also the correct behaviour)
- Redis remains a hard dependency for the concurrent-request lock; a Redis outage
  allows duplicate concurrent requests through, though the SELECT-before-INSERT still
  catches most duplicates once the first request completes
