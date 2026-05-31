CREATE TABLE IF NOT EXISTS event_types (
    id              UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID         NOT NULL,
    tenant_id       UUID         NOT NULL,
    public_id       VARCHAR(20)  NOT NULL,

    name            VARCHAR(255) NOT NULL,
    schema_version  VARCHAR(50)  NOT NULL DEFAULT '1.0',
    description     VARCHAR(512),
    event_schema    JSONB        NOT NULL,

    archived        BOOLEAN      NOT NULL DEFAULT FALSE,

    created_by      UUID         NOT NULL,
    updated_by      UUID         NOT NULL,
    request_id      UUID         NOT NULL,
    version         INTEGER      NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_event_types_public_id
    ON event_types (public_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_event_types_tenant_name_version
    ON event_types (tenant_id, name, schema_version)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_event_types_tenant_id
    ON event_types (tenant_id)
    WHERE deleted_at IS NULL;
