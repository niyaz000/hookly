# Idempotency — implementation

This document covers the internal mechanics of idempotency key handling. For the API design rationale and client-facing semantics see [decisions/api-design/007-idempotency-key.md](../decisions/api-design/007-idempotency-key.md).

## Body identity

Request body identity is determined by a SHA-256 hash of the canonical JSON representation (re-serialized from the parsed struct — normalizes whitespace and key ordering). The hash is stored alongside the response; on replay, the stored hash is compared to the incoming request's hash before returning the cached response.

## Storage

Idempotency records are stored in Redis:

| Key pattern | Purpose | TTL |
|---|---|---|
| `idmp:{namespace}:{key}` | Completed request record (hash + response) | 24 hours |
| `idmp_lock:{namespace}:{key}` | Distributed lock for in-flight requests | 60 seconds |

`namespace` is the handler name (e.g., `applications`) — keys are scoped to prevent cross-endpoint collisions.

## Failure behavior

If the handler returns an error, no idempotency record is stored. The client may retry with the same key and body — the request will execute again. This is deliberate: a failed request carries no side effects, so the key must remain available for retry.

## Lock protocol

The distributed lock uses Redis `SET NX PX` (set-if-not-exists with a millisecond TTL). Release uses a Lua script that checks the stored token before deleting — this prevents a slow handler from releasing a lock that has already expired and been acquired by another request (ABA race):

```lua
if redis.call("get", KEYS[1]) == ARGV[1] then
    return redis.call("del", KEYS[1])
else
    return 0
end
```
