CREATE TABLE IF NOT EXISTS organizations_audits (
    id              BIGSERIAL    PRIMARY KEY,
    entity_id       UUID         NOT NULL,
    public_id       VARCHAR(24)  NOT NULL,
    request_id      UUID         NOT NULL,

    operation       VARCHAR(10)  NOT NULL,
    old_data        JSONB,
    new_data        JSONB,

    created_by      UUID         NOT NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_organizations_audits_entity_id
    ON organizations_audits (entity_id, created_at);

CREATE INDEX IF NOT EXISTS idx_organizations_audits_created_at
    ON organizations_audits (created_at);
