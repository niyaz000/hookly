CREATE TABLE api_key_audits (
    id                UUID        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    api_key_id        UUID        NOT NULL,
    api_key_public_id VARCHAR(20) NOT NULL,
    organization_id   UUID        NOT NULL,
    tenant_id         UUID        NOT NULL,
    user_id           UUID        NOT NULL,
    action            VARCHAR(30) NOT NULL,
    actor_id          UUID,
    request_id        UUID        NOT NULL,
    changes           JSONB,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_key_audits_key_id       ON api_key_audits (api_key_id);
CREATE INDEX idx_api_key_audits_tenant_time  ON api_key_audits (tenant_id, created_at DESC);
