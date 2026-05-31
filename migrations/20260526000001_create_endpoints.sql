CREATE TABLE IF NOT EXISTS endpoints (
    id                    UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    public_id             VARCHAR(20)  NOT NULL,
    application_id        UUID         NOT NULL REFERENCES applications(id),
    tenant_id             UUID         NOT NULL,
    organization_id       UUID         NOT NULL,

    description           VARCHAR(512),
    endpoint_type         VARCHAR(50)  NOT NULL DEFAULT 'http',
    config                JSONB        NOT NULL,
    event_types           TEXT[]       NOT NULL DEFAULT '{}',
    status                VARCHAR(20)  NOT NULL DEFAULT 'active',
    rate_limit_per_minute INTEGER,
    tags                  JSONB        NOT NULL DEFAULT '{}',

    version               INTEGER      NOT NULL DEFAULT 0,
    request_id            UUID         NOT NULL,
    created_by            UUID         NOT NULL,
    updated_by            UUID         NOT NULL,
    created_at            TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at            TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    deleted_at            TIMESTAMPTZ,

    CONSTRAINT endpoints_status_check      CHECK (status IN ('active', 'paused')),
    CONSTRAINT endpoints_type_check        CHECK (endpoint_type IN ('http')),
    CONSTRAINT endpoints_rate_limit_check  CHECK (
        rate_limit_per_minute IS NULL OR rate_limit_per_minute BETWEEN 1 AND 100000
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_endpoints_public_id
    ON endpoints (public_id);

CREATE INDEX IF NOT EXISTS idx_endpoints_application_id
    ON endpoints (application_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_endpoints_tenant_id
    ON endpoints (tenant_id)
    WHERE deleted_at IS NULL;

-- GIN index for future delivery routing: find all endpoints subscribed to a given event_type
CREATE INDEX IF NOT EXISTS idx_endpoints_event_types
    ON endpoints USING GIN (event_types);

-- ---------------------------------------------------------------

CREATE TABLE IF NOT EXISTS endpoint_secrets (
    id              UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    public_id       VARCHAR(20)  NOT NULL,
    endpoint_id     UUID         NOT NULL REFERENCES endpoints(id),
    tenant_id       UUID         NOT NULL,
    organization_id UUID         NOT NULL,

    -- AES-256-GCM encrypted envelope: "v1$<nonce_b64url>$<ciphertext_b64url>"
    secret          TEXT         NOT NULL,
    is_active       BOOLEAN      NOT NULL DEFAULT TRUE,
    -- non-NULL only during a grace-period rotation; active while is_active=TRUE and expires_at > NOW()
    expires_at      TIMESTAMPTZ,

    request_id      UUID         NOT NULL,
    created_by      UUID         NOT NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_endpoint_secrets_public_id
    ON endpoint_secrets (public_id);

CREATE INDEX IF NOT EXISTS idx_endpoint_secrets_lookup
    ON endpoint_secrets (endpoint_id, is_active);
