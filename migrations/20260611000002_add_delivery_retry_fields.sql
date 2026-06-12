ALTER TABLE delivery_jobs
    ADD COLUMN retry_after  TIMESTAMPTZ,
    ADD COLUMN max_attempts INTEGER NOT NULL DEFAULT 5;

CREATE INDEX idx_delivery_jobs_retry
    ON delivery_jobs (retry_after)
    WHERE status = 'retrying' AND enqueued_at IS NULL;
