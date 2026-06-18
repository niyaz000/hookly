-- Migrate idempotency to entity-level columns on events and schedules only.
-- Tie schedules to an application (previously missing association).
--
-- Events: drop the old application-scoped unique index (forever deduplication),
-- add body_hash, shrink key to 64 chars (matches header max), new non-unique
-- partial index scoped to application_id with 1-hour TTL enforced at query time.
--
-- Schedules: add application_id FK, then idempotency columns with matching index.

-- ── events ────────────────────────────────────────────────────────────────────

-- Drop old partial unique index (wrong scope: no TTL, unique constraint)
DROP INDEX IF EXISTS idx_events_idempotency;

-- Resize idempotency_key VARCHAR(256) → VARCHAR(64) to match header max-length
ALTER TABLE events
    ALTER COLUMN idempotency_key TYPE VARCHAR(64);

-- SHA-256 of canonical request body (32 raw bytes) for drift detection
ALTER TABLE events
    ADD COLUMN IF NOT EXISTS body_hash BYTEA;

-- Non-unique lookup index — TTL enforced via created_at filter at query time
CREATE INDEX IF NOT EXISTS idx_events_idempotency
    ON events (application_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

-- ── schedules ─────────────────────────────────────────────────────────────────

-- Schedules previously had no application association — add it now
ALTER TABLE schedules
    ADD COLUMN IF NOT EXISTS application_id UUID REFERENCES applications(id);

ALTER TABLE schedules
    ADD COLUMN IF NOT EXISTS idempotency_key VARCHAR(64),
    ADD COLUMN IF NOT EXISTS body_hash       BYTEA;

CREATE INDEX IF NOT EXISTS idx_schedules_idempotency
    ON schedules (application_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL AND application_id IS NOT NULL;
