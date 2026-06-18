# Idempotency — implementation

This document covers the internal mechanics of idempotency key handling. For the API design rationale and client-facing semantics see [decisions/api-design/007-idempotency-key.md](../decisions/api-design/007-idempotency-key.md).

## Scope

Idempotency is supported on two entities only:

- `POST /api/v1/events` — a duplicate create fires a webhook twice
- `POST /api/v1/schedules` — a duplicate create registers conflicting cron entries

All other `POST` handlers ignore the `Idempotency-Key` header silently.

## Body identity

Request body identity is determined by a SHA-256 hash of the canonical JSON representation
(re-serialized from the parsed struct — normalizes whitespace and key ordering). The 32-byte
hash is stored as `BYTEA` on the entity row. On replay, the stored hash is compared to the
incoming request's hash; a mismatch returns 409.

## Storage — entity-level columns

Completed idempotency records are stored **on the entity table itself** — no separate store.
The entity row is the idempotency record. Redis retains only the distributed lock for
concurrent in-flight request protection.

For the full rationale and alternatives considered see
[decisions/database/004-idempotency-storage.md](../decisions/database/004-idempotency-storage.md).

### Redis (lock only)

| Key pattern | Purpose | TTL |
|---|---|---|
| `idmp_lock:{namespace}:{key}` | Distributed lock for concurrent in-flight requests | 60 seconds |

`namespace` is the entity type (`events`, `schedules`) — scopes locks to prevent
cross-entity collisions.

### PostgreSQL (entity columns)

```sql
-- events table
ALTER TABLE events
    ADD COLUMN idempotency_key  VARCHAR(64),
    ADD COLUMN body_hash        BYTEA;

CREATE INDEX idx_events_idempotency
    ON events (tenant_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- schedules table
ALTER TABLE schedules
    ADD COLUMN idempotency_key  VARCHAR(64),
    ADD COLUMN body_hash        BYTEA;

CREATE INDEX idx_schedules_idempotency
    ON schedules (tenant_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
```

`idempotency_key` is nullable. Rows created without an `Idempotency-Key` header carry `NULL`
and are excluded from the partial index entirely.

**TTL window:** 1 hour, enforced at query time via `created_at` filter. No rows are
ever deleted for idempotency purposes — the entity rows persist on their own lifecycle.

**Key scope:** per application. A key `"abc"` used in application A is independent of `"abc"`
used in application B. Cross-entity collision is impossible by design (events and schedules
use separate tables).

## Lookup

```sql
-- events
SELECT * FROM events
WHERE  application_id  = $1
  AND  idempotency_key = $2
  AND  created_at      > NOW() - INTERVAL '1 hour'
LIMIT  1;

-- schedules (identical pattern)
SELECT * FROM schedules
WHERE  application_id  = $1
  AND  idempotency_key = $2
  AND  created_at      > NOW() - INTERVAL '1 hour'
LIMIT  1;
```

## Request flow

```
Incoming POST with Idempotency-Key header:

1. Acquire Redis lock  SET NX PX 60000
   key:   idmp_lock:{namespace}:{idempotency_key}
   value: random UUID token
   ↓ Not acquired → 409 (concurrent request already in-flight)

2. SELECT * FROM <entity_table>
   WHERE  application_id  = $1
     AND  idempotency_key = $2
     AND  created_at      > NOW() - INTERVAL '1 hour'

   Row found, body_hash matches  → release lock, return entity row (200)
   Row found, body_hash mismatch → release lock, 409
   No row                        → proceed

3. Execute handler logic (validate, resolve FKs, etc.)

4. INSERT entity row with idempotency_key and body_hash populated.
   On error: transaction rolls back, no record is stored, key is available for retry.

5. Release Redis lock
   Lua: DEL lock key only if stored token still matches (prevents ABA race)

6. Return 201
```

## Failure behavior

If the handler returns an error, the entity INSERT is rolled back. No idempotency record
is stored. The client may retry with the same key and body — the request will execute
again. This is deliberate: a failed request carries no side effects, so the key must remain
available for retry.

## Lock protocol

The distributed lock uses Redis `SET NX PX` (set-if-not-exists with a millisecond TTL).
Release uses a Lua script that checks the stored token before deleting — this prevents a
slow handler from releasing a lock that has already expired and been acquired by another
request (ABA race):

```lua
if redis.call("get", KEYS[1]) == ARGV[1] then
    return redis.call("del", KEYS[1])
else
    return 0
end
```
