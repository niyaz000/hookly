-- users_audits
-- public_id / tenant_id / organization_id are nullable: users table predates the multi-tenant schema
CREATE TABLE IF NOT EXISTS users_audits (
    id              BIGSERIAL    PRIMARY KEY,
    entity_id       UUID         NOT NULL,
    public_id       VARCHAR(20),
    tenant_id       UUID,
    organization_id UUID,
    request_id      UUID         NOT NULL,

    operation       VARCHAR(10)  NOT NULL,
    old_data        JSONB,
    new_data        JSONB,

    created_by      UUID         NOT NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_users_audits_entity_id
    ON users_audits (entity_id, created_at);

CREATE INDEX IF NOT EXISTS idx_users_audits_created_at
    ON users_audits (created_at);

-- applications_audits
CREATE TABLE IF NOT EXISTS applications_audits (
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

CREATE INDEX IF NOT EXISTS idx_applications_audits_entity_id
    ON applications_audits (entity_id, created_at);

CREATE INDEX IF NOT EXISTS idx_applications_audits_created_at
    ON applications_audits (created_at);

-- event_types_audits
CREATE TABLE IF NOT EXISTS event_types_audits (
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

CREATE INDEX IF NOT EXISTS idx_event_types_audits_entity_id
    ON event_types_audits (entity_id, created_at);

CREATE INDEX IF NOT EXISTS idx_event_types_audits_created_at
    ON event_types_audits (created_at);
