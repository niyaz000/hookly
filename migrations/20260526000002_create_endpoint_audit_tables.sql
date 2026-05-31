CREATE TABLE IF NOT EXISTS endpoints_audits (
    id              BIGSERIAL    PRIMARY KEY,
    entity_id       UUID         NOT NULL,
    public_id       VARCHAR(20)  NOT NULL,
    tenant_id       UUID         NOT NULL,
    organization_id UUID         NOT NULL,
    request_id      UUID         NOT NULL,

    operation       VARCHAR(10)  NOT NULL,
    old_data        JSONB,
    new_data        JSONB,

    created_by      UUID         NOT NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_endpoints_audits_entity_id
    ON endpoints_audits (entity_id, created_at);

CREATE INDEX IF NOT EXISTS idx_endpoints_audits_created_at
    ON endpoints_audits (created_at);

-- ---------------------------------------------------------------
-- endpoint_secrets_audits: the 'secret' column is stripped from
-- old_data / new_data by the trigger to prevent persisting secrets
-- in the audit trail.
-- ---------------------------------------------------------------

CREATE TABLE IF NOT EXISTS endpoint_secrets_audits (
    id              BIGSERIAL    PRIMARY KEY,
    entity_id       UUID         NOT NULL,
    public_id       VARCHAR(20)  NOT NULL,
    endpoint_id     UUID         NOT NULL,
    tenant_id       UUID         NOT NULL,
    organization_id UUID         NOT NULL,
    request_id      UUID         NOT NULL,

    operation       VARCHAR(10)  NOT NULL,
    old_data        JSONB,   -- 'secret' field intentionally omitted
    new_data        JSONB,   -- 'secret' field intentionally omitted

    created_by      UUID         NOT NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_endpoint_secrets_audits_entity_id
    ON endpoint_secrets_audits (entity_id, created_at);

CREATE INDEX IF NOT EXISTS idx_endpoint_secrets_audits_endpoint_id
    ON endpoint_secrets_audits (endpoint_id, created_at);
