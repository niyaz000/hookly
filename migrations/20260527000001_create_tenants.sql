CREATE TYPE tenant_status AS ENUM ('active', 'suspended', 'inactive');

CREATE TABLE tenants (
    id              UUID            NOT NULL,
    public_id       VARCHAR(24)     NOT NULL,
    organization_id UUID            NOT NULL,
    name            VARCHAR(255)    NOT NULL,
    description     TEXT,
    status          tenant_status   NOT NULL DEFAULT 'active',
    tags            JSONB           NOT NULL DEFAULT '{}',
    metadata        JSONB           NOT NULL DEFAULT '{}',
    settings        JSONB           NOT NULL DEFAULT '{}',
    created_by      UUID            NOT NULL,
    updated_by      UUID            NOT NULL,
    request_id      UUID            NOT NULL,
    version         INTEGER         NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,
    CONSTRAINT tenants_pk           PRIMARY KEY (id),
    CONSTRAINT tenants_public_id_uq UNIQUE (public_id),
    CONSTRAINT tenants_name_uq      UNIQUE (name),
    CONSTRAINT tenants_name_nonempty CHECK (char_length(trim(name)) >= 1)
);

CREATE INDEX idx_tenants_organization_id ON tenants (organization_id);
CREATE INDEX idx_tenants_status          ON tenants (status);
CREATE INDEX idx_tenants_created_at      ON tenants (created_at);
CREATE INDEX idx_tenants_deleted_at      ON tenants (deleted_at) WHERE deleted_at IS NOT NULL;
