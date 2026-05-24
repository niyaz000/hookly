CREATE TABLE IF NOT EXISTS applications (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL,
    tenant_id       UUID NOT NULL,
    public_id       VARCHAR(20) NOT NULL,
    name            VARCHAR(64) NOT NULL,
    description     VARCHAR(255) NOT NULL DEFAULT '',
    tags            JSONB NOT NULL DEFAULT '{}',
    metadata        JSONB NOT NULL DEFAULT '{}',
    created_by      UUID NOT NULL,
    updated_by      UUID NOT NULL,
    request_id      UUID NOT NULL,
    version         INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_applications_public_id
    ON applications (public_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_applications_tenant_name
    ON applications (tenant_id, name);
