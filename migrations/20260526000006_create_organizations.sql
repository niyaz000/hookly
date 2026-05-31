DROP TABLE IF EXISTS organizations;
DROP TYPE IF EXISTS organization_status;

CREATE TYPE organization_status AS ENUM ('active', 'suspended', 'inactive');

CREATE TABLE organizations (
    id                  UUID                NOT NULL,
    public_id           VARCHAR(24)         NOT NULL,
    name                VARCHAR(255)        NOT NULL,
    slug                VARCHAR(64)         NOT NULL,
    status              organization_status NOT NULL DEFAULT 'active',
    billing_email       VARCHAR(64),
    plan                VARCHAR(32)         NOT NULL DEFAULT 'free',
    stripe_customer_id  VARCHAR(32),
    external_id         VARCHAR(64),
    tags                JSONB               NOT NULL DEFAULT '{}',
    metadata            JSONB               NOT NULL DEFAULT '{}',
    settings            JSONB               NOT NULL DEFAULT '{}',
    created_by          UUID                NOT NULL,
    updated_by          UUID                NOT NULL,
    request_id          UUID                NOT NULL,
    version             INTEGER             NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    deleted_at          TIMESTAMPTZ,

    CONSTRAINT organizations_pk            PRIMARY KEY (id),
    CONSTRAINT organizations_public_id_uq  UNIQUE (public_id),
    CONSTRAINT organizations_slug_uq       UNIQUE (slug),
    CONSTRAINT organizations_name_nonempty CHECK (char_length(trim(name)) >= 1),
    CONSTRAINT organizations_slug_format
        CHECK (
            slug ~ '^[a-z0-9][a-z0-9-]*[a-z0-9]$'
            OR (char_length(slug) = 1 AND slug ~ '^[a-z0-9]$')
        )
);

CREATE INDEX idx_organizations_status     ON organizations (status);
CREATE INDEX idx_organizations_created_at ON organizations (created_at);
CREATE INDEX idx_organizations_deleted_at ON organizations (deleted_at) WHERE deleted_at IS NOT NULL;
