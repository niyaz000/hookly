-- delivery_jobs: mutable state for each pending/in-progress/done delivery
CREATE TABLE IF NOT EXISTS delivery_jobs (
    id              UUID          PRIMARY KEY DEFAULT gen_random_uuid(),
    public_id       VARCHAR(20)   NOT NULL,
    event_id        UUID          NOT NULL REFERENCES events(id),
    endpoint_id     UUID          NOT NULL REFERENCES endpoints(id),
    organization_id UUID          NOT NULL,
    status          TEXT          NOT NULL DEFAULT 'pending',
    attempt         INT           NOT NULL DEFAULT 0,
    enqueued_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_delivery_jobs_public_id
    ON delivery_jobs (public_id);

CREATE INDEX IF NOT EXISTS idx_delivery_jobs_event_id
    ON delivery_jobs (event_id);

-- Outbox poller scans this index to find jobs missed by XADD
CREATE INDEX IF NOT EXISTS idx_delivery_jobs_outbox
    ON delivery_jobs (created_at)
    WHERE enqueued_at IS NULL AND status = 'pending';

-- delivery_attempts: append-only log; one row per HTTP attempt
CREATE TABLE IF NOT EXISTS delivery_attempts (
    id              UUID          PRIMARY KEY DEFAULT gen_random_uuid(),
    delivery_job_id UUID          NOT NULL REFERENCES delivery_jobs(id),
    event_id        UUID          NOT NULL REFERENCES events(id),
    endpoint_id     UUID          NOT NULL REFERENCES endpoints(id),
    attempt_number  INT           NOT NULL,
    status          TEXT          NOT NULL,   -- 'success' | 'failed' | 'timeout'
    http_status     INT,
    response_body   TEXT,
    latency_ms      INT,
    attempted_at    TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_delivery_attempts_job
    ON delivery_attempts (delivery_job_id);

CREATE INDEX IF NOT EXISTS idx_delivery_attempts_event
    ON delivery_attempts (event_id);
