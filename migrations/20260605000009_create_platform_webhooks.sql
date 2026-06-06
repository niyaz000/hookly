CREATE TYPE platform_webhook_status AS ENUM ('active', 'suspended', 'disabled');

CREATE TABLE platform_webhooks (
    id                 UUID                    NOT NULL,
    public_id          VARCHAR(20)             NOT NULL,
    tenant_id          UUID                    NOT NULL,
    name               VARCHAR(128)            NOT NULL,
    description        TEXT,
    url                TEXT                    NOT NULL,
    signing_secret_enc TEXT                    NOT NULL,
    status             platform_webhook_status NOT NULL DEFAULT 'active',
    metadata           JSONB                   NOT NULL DEFAULT '{}',
    created_by         UUID,
    updated_by         UUID,
    created_at         TIMESTAMPTZ             NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ             NOT NULL DEFAULT NOW(),
    deleted_at         TIMESTAMPTZ,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX pwh_public_id_uq    ON platform_webhooks (public_id);
CREATE UNIQUE INDEX pwh_tenant_name_uq  ON platform_webhooks (tenant_id, name) WHERE deleted_at IS NULL;
CREATE INDEX        pwh_tenant_idx      ON platform_webhooks (tenant_id)        WHERE deleted_at IS NULL;
CREATE INDEX        pwh_tenant_stat_idx ON platform_webhooks (tenant_id, status) WHERE deleted_at IS NULL;
