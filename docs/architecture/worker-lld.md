# Worker — Low-Level Design

The delivery worker (`hookly-worker`) is a single process that runs N concurrent consumer tasks plus three
background tasks. It reads delivery jobs from Redis Streams and delivers them to tenant endpoints over HTTP.

---

## Process layout

```
┌──────────────────────────────────────────────────────────────────────┐
│                         hookly-worker                                │
│                                                                      │
│   ┌────────────┐ ┌────────────┐ ┌────────────┐                       │
│   │  Worker 0  │ │  Worker 1  │ │  Worker N  │  ← JoinSet            │
│   │ consumer   │ │ consumer   │ │ consumer   │    WORKER_NUM_WORKERS │
│   └─────┬──────┘ └─────┬──────┘ └─────┬──────┘    (default 4)        │
│         │              │              │                              │
│         └──────────────┴──────────────┘                              │
│                        │                                             │
│              Shared via Arc / Clone                                  │
│         PgPool · redis::Client · TenantCrypto · request::Client      │
│                                                                      │
│   ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐   │
│   │  reclaim::run()  │  │  outbox::run()   │  │   trim::run()    │   │
│   │  every 60s       │  │  every 10s       │  │   every 60s      │   │
│   │  XAUTOCLAIM idle │  │  missed XADD     │  │   XTRIM MINID    │   │
│   │  messages        │  │  recovery        │  │   safe cleanup   │   │
│   └──────────────────┘  └──────────────────┘  └──────────────────┘   │
│                                                                      │
│   Shutdown: tokio::watch channel → 15s graceful drain, then abort    │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Stream scheduling: how workers pick the next stream

Every worker shares a Redis sorted set `hookly:q:streams`. Score = last-claimed-at in
milliseconds. Workers always claim the stream with the **lowest score** — the one that was
consumed least recently. This gives round-robin fairness across all streams without any
inter-worker coordination in application code.

```
hookly:q:streams  (Redis ZSET)
┌──────────────────────────────────────────────────────┐
│  member                      score (ms since epoch)  │
│  ─────────────────────────── ──────────────────────  │
│  hookly:q:tier:default       1718000000000  ← lowest │  ← claimed next
│  hookly:q:tier:silver        1718000001200            │
│  hookly:q:tier:gold          1718000002400            │
│  hookly:q:tier:platinum      1718000003600            │
│  hookly:q:org:3f4a1b2c...    1718000004800  ← highest │
└──────────────────────────────────────────────────────┘

Claim Lua (atomic, single-threaded in Redis):
  members = ZRANGE hookly:q:streams 0 0      → lowest-score member
  ZADD    hookly:q:streams {now_ms} {member} → bump score to now
  return  member

No two workers claim the same stream simultaneously.
If the set is empty, the worker sleeps WORKER_POLL_INTERVAL_MS (default 250ms).
```

**How streams enter the set:**
```
On XADD (enqueue):
  register_stream:  ZADD hookly:q:streams NX 0 {stream}
                    NX → no-op if already present, never resets an active score

On startup (worker main):
  All TIER_STREAMS registered unconditionally.
  Enterprise streams (hookly:q:org:*) discovered via SCAN and registered.
```

**Stream types:**

| Stream key | Tenant tier | Worker assignment |
|---|---|---|
| `hookly:q:tier:default` | default | Shared pool |
| `hookly:q:tier:silver` | silver | Shared pool |
| `hookly:q:tier:gold` | gold | Shared pool |
| `hookly:q:tier:platinum` | platinum | Shared pool |
| `hookly:q:org:{org_id}` | enterprise | Shared pool (isolated queue) |

Enterprise orgs get a dedicated stream to prevent a high-volume tenant from starving others.

---

## Main delivery flow

```
Worker loop (each of N workers runs this independently)
────────────────────────────────────────────────────────────────────────

  ┌─────────────────────────────────────────────────────────────────┐
  │ 1. CLAIM  claim_next_stream(now_ms)                             │
  │           Lua: ZRANGE + ZADD(now_ms)                           │
  │           → None:    sleep(250ms), loop                        │
  │           → stream:  proceed                                    │
  └────────────────────────────┬────────────────────────────────────┘
                               │
  ┌────────────────────────────▼────────────────────────────────────┐
  │ 2. READ   XREADGROUP GROUP workers {consumer}                   │
  │                      COUNT 10                                   │
  │                      STREAMS {stream} >                         │
  │           (non-blocking — returns immediately)                  │
  │                                                                 │
  │           → empty:   remove_stream_if_empty                     │
  │                      Lua: XLEN {stream} == 0 →                 │
  │                           ZREM hookly:q:streams {stream}        │
  │                      (publisher re-adds on next XADD)           │
  │                                                                 │
  │           → messages: for each (msg_id, job_pub_id)            │
  └────────────────────────────┬────────────────────────────────────┘
                               │
  ┌────────────────────────────▼────────────────────────────────────┐
  │ 3. FETCH  PG: SELECT delivery_jobs dj                           │
  │                JOIN  endpoints     e  ON e.id = dj.endpoint_id  │
  │                JOIN  events        ev ON ev.id = dj.event_id    │
  │                JOIN  endpoint_secrets es                        │
  │                WHERE dj.public_id = {job_pub_id}               │
  │                  AND dj.status IN ('pending','retrying')        │
  │                                                                 │
  │           → None (terminal/inactive): XACK, continue           │
  │           → DB error: skip XACK (XAUTOCLAIM retries in 90s)   │
  └────────────────────────────┬────────────────────────────────────┘
                               │
  ┌────────────────────────────▼────────────────────────────────────┐
  │ 4. SIGN   Decrypt:  AES-256-GCM(master_key, tenant_id)         │
  │                     → signing secret plaintext                  │
  │           Sign:     msg = "{event_id}.{unix_ts}.{payload_json}"│
  │                     sig = HMAC-SHA256(secret, msg)             │
  │                     header: "webhook-signature: v1,{base64}"   │
  └────────────────────────────┬────────────────────────────────────┘
                               │
  ┌────────────────────────────▼────────────────────────────────────┐
  │ 5. DELIVER HTTP {method} {endpoint_url}                         │
  │           Content-Type:       application/json                  │
  │           webhook-id:         {event_public_id}                 │
  │           webhook-timestamp:  {unix_ts}                         │
  │           webhook-signature:  v1,{sig}                          │
  │           traceparent:        {W3C TraceContext}                │
  │           Timeout:            WORKER_DELIVERY_TIMEOUT_SECS (30s)│
  │                                                                 │
  │           Outcome:                                              │
  │             200–299  → DeliveryStatus::Success                  │
  │             4xx/5xx  → DeliveryStatus::Failed                   │
  │             timeout  → DeliveryStatus::Timeout                  │
  └────────────────────────────┬────────────────────────────────────┘
                               │
  ┌────────────────────────────▼────────────────────────────────────┐
  │ 6. RECORD PG: INSERT delivery_attempts                          │
  │                (job_id, event_id, endpoint_id, attempt_number,  │
  │                 status, http_status, response_body[:4096],      │
  │                 latency_ms)                                     │
  └────────────────────────────┬────────────────────────────────────┘
                               │
              ┌────────────────┴────────────────┐
              │ Success (2xx)                   │ Failure / Timeout
              ▼                                 ▼
  ┌────────────────────────┐     ┌──────────────────────────────────┐
  │ complete_job           │     │ attempt < max_attempts?          │
  │ UPDATE delivery_jobs   │     │                                  │
  │   SET status=delivered │     │  YES: schedule_retry             │
  └────────────┬───────────┘     │    delay = min(30*2^n, 3600) s  │
               │                 │    UPDATE delivery_jobs          │
               │                 │      SET status=retrying         │
               │                 │          next_retry_at=now+delay │
               │                 │    outbox poller re-enqueues     │
               │                 │    when next_retry_at <= now()   │
               │                 │                                  │
               │                 │  NO:  fail_job                   │
               │                 │    UPDATE delivery_jobs          │
               │                 │      SET status=dead_lettered    │
               │                 └──────────────┬───────────────────┘
               │                                │
               └───────────────┬────────────────┘
                               │
  ┌────────────────────────────▼────────────────────────────────────┐
  │ 7. ACK    XACK {stream} workers {msg_id}                        │
  │           Always ACK — failure state is in delivery_attempts,   │
  │           not in the stream PEL.                                │
  └─────────────────────────────────────────────────────────────────┘
```

---

## Retry policy — exponential backoff

Backoff formula: `delay = min(30 × 2^attempt, 3600)` seconds

| Attempt | Delay before retry |
|---|---|
| 1st failure | 30 s |
| 2nd failure | 60 s |
| 3rd failure | 120 s |
| 4th failure | 240 s |
| 5th failure | 480 s |
| 6th+ failure | 960 s → 1920 s → … → capped at 3600 s |
| max_attempts reached | `dead_lettered` — no more retries |

After `schedule_retry`, the job sits in `status=retrying` with `next_retry_at` set. The
**outbox poller** re-enqueues it once `next_retry_at <= now()`, at which point the worker
picks it up and makes another attempt.

---

## XAUTOCLAIM recovery (reclaim task)

```
If a worker crashes after claiming a message but before XACK,
the message stays in the PEL (pending entry list) forever.

reclaim::run()  runs every 60s:

  1. ZRANGE hookly:q:streams 0 -1       → all registered streams
  2. For each stream:
       XAUTOCLAIM GROUP   workers
                  CONSUMER {reclaim_consumer}
                  MIN-IDLE 90000 ms      ← idle threshold
                  START    0-0
                  COUNT    100
     → returns messages idle >90s (owner was likely dead)
  3. Each claimed message → process_one() immediately
  4. XACK on completion

At-least-once guarantee: worst case, a message is re-delivered
once after worker crash. Recipients should deduplicate on
webhook-id (= event_public_id).
```

---

## Outbox poller (missed XADD recovery)

```
If Redis is unavailable when the API server calls XADD, the delivery_job
is written to PostgreSQL but never enters a stream.

outbox::run()  runs every WORKER_OUTBOX_INTERVAL_SECS (default 10s):

  SELECT public_id, stream_name FROM delivery_jobs
  WHERE  enqueued_at IS NULL
    AND  status IN ('pending', 'retrying')
    AND  (next_retry_at IS NULL OR next_retry_at <= NOW())
  LIMIT  500

  For each job:
    XADD {stream_name} * j {job_public_id}       → enqueue
    ZADD hookly:q:streams NX 0 {stream_name}     → register_stream
    UPDATE delivery_jobs SET enqueued_at = NOW()  → mark done

PostgreSQL is the durable source of truth.
Redis is an acceleration layer; the outbox is the safety net.
```

---

## Safe stream trimming (trim task)

```
trim::run()  runs every WORKER_TRIM_INTERVAL_SECS (default 60s):

  ZRANGE hookly:q:streams 0 -1       → all streams

  For each stream:

    XPENDING {stream} workers         → [count, min-id, max-id, ...]

    PEL non-empty:
      cutoff = oldest PEL entry (min-id)
      Everything before min-id is guaranteed ACK'd.

    PEL empty:
      XINFO GROUPS {stream}
      cutoff = last-delivered-id for "workers" group
      All messages consumed and ACK'd up to this point.

    XTRIM {stream} MINID ~ {cutoff}
      ~ allows Redis to round to the nearest block boundary
        (avoids splitting a radix-tree node for a marginal trim)

Never trims unacknowledged messages. Safe across multiple pods.
```

---

## OTel instrumentation

```
Metrics (opentelemetry global meter "hookly.worker"):
  delivery_attempts_total     counter
    labels: status={success|failed|timeout}, endpoint_id={uuid}

  delivery_latency_ms         histogram
    labels: same as above

Traces (tracing-opentelemetry bridge):
  Span set on each deliver() call:
    http.url        = {endpoint URL}
    http.method     = POST
    http.status_code= {response code}
    otel.kind       = client

  W3C TraceContext injected into outbound request:
    traceparent: {version}-{trace_id}-{parent_id}-{flags}
    tracestate:  (if present)

  Receiving endpoint can attach its own spans to the same trace.

Startup: telemetry::init(&cfg)
  OTEL_EXPORTER_OTLP_ENDPOINT set → OTLP gRPC export
  unset                           → structured stdout only (no-op guard)
```
