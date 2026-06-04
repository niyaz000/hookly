CREATE TYPE environment_status AS ENUM ('active', 'disabled');

CREATE TABLE environments (
    id          UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    public_id   VARCHAR(20) NOT NULL,
    tenant_id   UUID        NOT NULL,
    name        VARCHAR(64) NOT NULL,
    status      environment_status NOT NULL DEFAULT 'active',
    tags        JSONB       NOT NULL DEFAULT '{}',
    request_id  UUID        NOT NULL,
    version     INTEGER     NOT NULL DEFAULT 0,
    created_by  UUID        NOT NULL,
    updated_by  UUID        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT environments_public_id_uq   UNIQUE (public_id),
    CONSTRAINT environments_tenant_name_uq UNIQUE (tenant_id, name),
    CONSTRAINT environments_name_format    CHECK (name ~ '^[a-z][a-z0-9_-]{2,63}$')
);

CREATE INDEX idx_environments_tenant_id ON environments (tenant_id);
CREATE INDEX idx_environments_status    ON environments (tenant_id, status);
