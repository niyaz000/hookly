CREATE TYPE api_key_environment AS ENUM ('live', 'test', 'dev', 'sandbox');
CREATE TYPE api_key_status AS ENUM ('active', 'expired');

CREATE TABLE api_keys (
    id              UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    public_id       VARCHAR(20) NOT NULL,
    organization_id UUID        NOT NULL,
    tenant_id       UUID        NOT NULL,
    user_id         UUID        NOT NULL,
    name            VARCHAR(64) NOT NULL,
    description     VARCHAR(521),
    key_hash        TEXT        NOT NULL,
    key_encrypted   TEXT,
    key_prefix      VARCHAR(3)  NOT NULL,
    environment     api_key_environment NOT NULL,
    status          api_key_status NOT NULL DEFAULT 'active',
    expires_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,
    request_id      UUID        NOT NULL,
    version         INTEGER     NOT NULL DEFAULT 0,
    created_by      UUID        NOT NULL,
    updated_by      UUID        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT api_keys_public_id_uq UNIQUE (public_id),
    CONSTRAINT api_keys_key_hash_uq  UNIQUE (key_hash)
);

CREATE INDEX idx_api_keys_tenant_user ON api_keys (tenant_id, user_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_api_keys_tenant_id   ON api_keys (tenant_id)          WHERE deleted_at IS NULL;
