# ADR scheduler/002: Missed fire policy for cron schedules

## Status
Accepted

## Context

The scheduler binary may be unavailable for a period — planned maintenance, a crash, or a deploy. During this window, cron schedules accumulate `next_run_at` values in the past. When the scheduler recovers, it faces a choice: what should it do with the missed fires?

Three outcomes are meaningful for tenants:

**Skip**: Advance `next_run_at` to the next future occurrence without firing. Treats the schedule as "fire on schedule or not at all." Correct for jobs where staleness invalidates the value of running (e.g., a report that's now too old to be useful).

**Fire once**: Fire exactly one catch-up event per schedule, regardless of how many occurrences were missed. Balances reliability ("the thing happened") with safety ("don't flood the endpoint after a restart").

**Fire all**: Fire every missed occurrence in sequence. Correct for jobs where each occurrence must be processed (e.g., invoicing that must run for every billing period). Risks overwhelming tenant endpoints with a burst if the scheduler was down for hours.

There is no universally correct answer — the right behaviour depends on what the schedule is used for. This must be a per-schedule configuration.

## Decision

A `missed_fire_policy` column on the `schedules` table with three values:

| Value | Behaviour on recovery |
|---|---|
| `skip` | Advance `next_run_at` to the next future tick; no catch-up event |
| `fire_once` | Fire exactly one catch-up event; advance to the next future tick |
| `fire_all` | Fire one event per missed occurrence in order; then advance to the next future tick |

**Default**: `fire_once`.

The rationale for `fire_once` as the default: most webhook-triggered workflows are event-driven (something happened), not time-series-critical (every minute matters). A single catch-up notification that the system is running again is more useful than silence, and less dangerous than a burst.

### Detection

On scheduler startup and after any gap exceeding 90 seconds (detected by comparing `now` against the scheduler's own `last_tick_at` key in Redis), the reconciliation task scans for:
```sql
SELECT * FROM schedules
WHERE status = 'active'
  AND next_run_at < NOW() - INTERVAL '90 seconds'
```

For each result, the `missed_fire_policy` determines how many outbox entries to create.

### fire_all implementation

For `fire_all`, the scheduler iterates missed occurrences using the cron expression evaluator, creating one outbox entry per occurrence, up to a safety cap of `max_catch_up_fires` (default: 50 per schedule, configurable). This cap prevents a schedule with a 1-minute expression that was missed for 24 hours from creating 1440 events on recovery.

The `max_catch_up_fires` cap is logged as a warning and surfaced as a metric when hit: `hookly_scheduler_catch_up_capped_total`.

## Principles upheld

- **Observability for everyone** — the policy is visible on the schedule resource via the API; tenants can choose the behaviour that matches their use case without reading platform documentation
- **Developer experience** — the default (`fire_once`) is the safe choice; tenants who need `fire_all` opt in explicitly; `skip` is available for time-sensitive-only workloads
- **Reliability through simplicity** — the policy is evaluated once per schedule per recovery event; no ongoing state machine; the `max_catch_up_fires` cap prevents unbounded catch-up work

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Platform-wide fixed policy (always skip) | Breaks tenants who depend on every occurrence being processed (e.g., billing) |
| Platform-wide fixed policy (always fire_once) | Breaks tenants who need every occurrence (e.g., time-series ETL) and those who want clean no-fire semantics (e.g., TTL-gated jobs) |
| Platform-wide fixed policy (always fire_all) | Risks tenant endpoint floods after any scheduler downtime; poorest default for most use cases |
| Per-schedule configurable (this decision) | Correct but adds a column and a code branch; accepted because the correct answer genuinely varies by use case |

## Consequences

**Positive:**
- Tenants choose the recovery behaviour that matches their workload
- The `fire_once` default is safe for the majority of webhook-triggered workflows
- The `max_catch_up_fires` cap prevents worst-case catch-up bursts from `fire_all` schedules

**Negative:**
- A third column on the `schedules` table that most tenants will never change
- `fire_all` with high missed-occurrence counts creates a burst of outbox entries; the delivery pipeline absorbs this, but tenant endpoints may see a rapid sequence of events immediately after scheduler recovery
- The 90-second gap detection requires the scheduler to maintain a `last_tick_at` key in Redis; this key must be bootstrapped on first run
