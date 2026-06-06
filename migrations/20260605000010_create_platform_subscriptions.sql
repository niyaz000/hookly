CREATE TABLE platform_webhook_subscriptions (
    tenant_id            UUID        NOT NULL,
    event_type_public_id VARCHAR(20) NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, event_type_public_id)
);

CREATE INDEX pws_event_type_idx ON platform_webhook_subscriptions (event_type_public_id);
