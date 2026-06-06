CREATE TYPE jwt_key_use    AS ENUM ('authentication', 'webhook_signature');
CREATE TYPE jwt_algorithm  AS ENUM ('RS256', 'RS384', 'RS512', 'ES256', 'ES384', 'ES512', 'HS256', 'HS512');
CREATE TYPE jwt_key_status AS ENUM ('active', 'disabled', 'expired');

CREATE TABLE jwt_keys (
    id                   UUID           NOT NULL,
    public_id            VARCHAR(20)    NOT NULL,
    tenant_id            UUID           NOT NULL,
    application_id       VARCHAR(20)    NULL,
    name                 VARCHAR(128)   NOT NULL,
    key_use              jwt_key_use    NOT NULL,
    algorithm            jwt_algorithm  NOT NULL,
    key_id               VARCHAR(64)    NOT NULL,
    status               jwt_key_status NOT NULL DEFAULT 'active',
    public_key           TEXT           NULL,
    private_key_enc      TEXT           NULL,
    secret_enc           TEXT           NULL,
    expires_at           TIMESTAMPTZ    NULL,
    grace_period_ends_at TIMESTAMPTZ    NULL,
    rotated_from_id      VARCHAR(20)    NULL,
    last_rotated_at      TIMESTAMPTZ    NULL,
    version              INT            NOT NULL DEFAULT 0,
    created_by           UUID           NOT NULL,
    updated_by           UUID           NOT NULL,
    created_at           TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
    deleted_at           TIMESTAMPTZ    NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX jwt_keys_public_id_uq ON jwt_keys (public_id);
CREATE UNIQUE INDEX jwt_keys_key_id_uq    ON jwt_keys (key_id);
CREATE INDEX jwt_keys_tenant_idx          ON jwt_keys (tenant_id);
-- Partial index to efficiently find keys whose grace period has ended and need disabling
CREATE INDEX jwt_keys_grace_expiry_idx    ON jwt_keys (grace_period_ends_at)
    WHERE grace_period_ends_at IS NOT NULL AND status = 'active';
