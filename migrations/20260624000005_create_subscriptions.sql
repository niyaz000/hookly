CREATE TABLE IF NOT EXISTS subscriptions (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    public_id       VARCHAR(20) NOT NULL,
    endpoint_id     UUID        NOT NULL REFERENCES endpoints(id),
    event_type_id   UUID        NOT NULL REFERENCES event_types(id),
    application_id  UUID        NOT NULL REFERENCES applications(id),
    tenant_id       UUID        NOT NULL,
    organization_id UUID        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'active',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_public_id
    ON subscriptions (public_id);

-- At most one active subscription per (endpoint, event_type) pair.
-- Partial so soft-deleted rows don't block re-subscription.
CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_unique_active
    ON subscriptions (endpoint_id, event_type_id)
    WHERE deleted_at IS NULL;

-- Hot path: fan-out query on event publish.
CREATE INDEX IF NOT EXISTS idx_subscriptions_fanout
    ON subscriptions (application_id, event_type_id)
    WHERE deleted_at IS NULL AND status = 'active';

CREATE INDEX IF NOT EXISTS idx_subscriptions_endpoint
    ON subscriptions (endpoint_id)
    WHERE deleted_at IS NULL;
