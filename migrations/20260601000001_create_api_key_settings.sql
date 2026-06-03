CREATE TABLE api_key_settings (
    id                  UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    public_id           VARCHAR(20) NOT NULL,
    organization_id     UUID        NOT NULL,
    tenant_id           UUID        NOT NULL,
    max_keys_per_user   INTEGER,
    key_length          SMALLINT    NOT NULL DEFAULT 32,
    default_ttl_seconds INTEGER,
    allow_view_later    BOOLEAN     NOT NULL DEFAULT FALSE,
    request_id          UUID        NOT NULL,
    version             INTEGER     NOT NULL DEFAULT 0,
    created_by          UUID        NOT NULL,
    updated_by          UUID        NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT api_key_settings_public_id_uq UNIQUE (public_id),
    CONSTRAINT api_key_settings_org_tenant_uq UNIQUE (organization_id, tenant_id)
);

CREATE INDEX idx_api_key_settings_tenant_id ON api_key_settings (tenant_id);
CREATE INDEX idx_api_key_settings_org_id ON api_key_settings (organization_id);
