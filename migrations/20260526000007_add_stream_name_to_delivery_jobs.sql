-- Records the stream that the delivery job was (or should be) enqueued into.
-- Used by the outbox poller to re-enqueue jobs that missed their initial XADD.
ALTER TABLE delivery_jobs
    ADD COLUMN IF NOT EXISTS stream_name TEXT NOT NULL DEFAULT 'hookly:q:tier:default';
