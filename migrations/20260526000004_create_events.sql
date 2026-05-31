CREATE TABLE IF NOT EXISTS events (
    id                  UUID          PRIMARY KEY DEFAULT gen_random_uuid(),
    public_id           VARCHAR(20)   NOT NULL,           -- "evn_<16 NanoId>"
    application_id      UUID          NOT NULL REFERENCES applications(id),
    event_type_id       UUID          NOT NULL REFERENCES event_types(id),
    endpoint_id         UUID          REFERENCES endpoints(id),  -- NULL = fan-out
    tenant_id           UUID          NOT NULL,
    organization_id     UUID          NOT NULL,

    payload             JSONB         NOT NULL,
    idempotency_key     VARCHAR(256),
    tags                JSONB         NOT NULL DEFAULT '{}',

    -- traceability; no updated_at or deleted_at — events are immutable
    request_id          UUID          NOT NULL,
    created_by          UUID          NOT NULL,
    created_at          TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_events_public_id
    ON events (public_id);

CREATE INDEX IF NOT EXISTS idx_events_application_id
    ON events (application_id);

CREATE INDEX IF NOT EXISTS idx_events_tenant_id
    ON events (tenant_id);

CREATE INDEX IF NOT EXISTS idx_events_type_created
    ON events (application_id, event_type_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_events_endpoint
    ON events (endpoint_id) WHERE endpoint_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_events_created_at
    ON events (application_id, created_at DESC);

-- Partial unique so NULL idempotency_key rows are never considered duplicates.
CREATE UNIQUE INDEX IF NOT EXISTS idx_events_idempotency
    ON events (application_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
