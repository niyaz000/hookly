CREATE TABLE schedules (
    id              UUID         NOT NULL,
    public_id       VARCHAR(24)  NOT NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    tenant_id       UUID         NOT NULL,
    organization_id UUID         NOT NULL,
    event_type_id   UUID         NOT NULL REFERENCES event_types(id),
    payload         JSONB        NOT NULL DEFAULT '{}',
    cron_expression VARCHAR(100) NOT NULL,
    timezone        VARCHAR(64)  NOT NULL DEFAULT 'UTC',
    status          VARCHAR(20)  NOT NULL DEFAULT 'active',
    next_run_at     TIMESTAMPTZ,
    last_run_at     TIMESTAMPTZ,
    last_run_status VARCHAR(20),
    created_by      UUID         NOT NULL,
    updated_by      UUID         NOT NULL,
    request_id      UUID         NOT NULL,
    version         INTEGER      NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,
    CONSTRAINT schedules_pk            PRIMARY KEY (id),
    CONSTRAINT schedules_public_id_uq  UNIQUE (public_id),
    CONSTRAINT schedules_name_nonempty CHECK (char_length(trim(name)) >= 1),
    CONSTRAINT schedules_version_pos   CHECK (version > 0),
    CONSTRAINT schedules_status_valid  CHECK (status IN ('active', 'paused', 'disabled'))
);

CREATE INDEX idx_schedules_tenant_id       ON schedules (tenant_id);
CREATE INDEX idx_schedules_organization_id ON schedules (organization_id);
CREATE INDEX idx_schedules_event_type_id   ON schedules (event_type_id);
CREATE INDEX idx_schedules_next_run_at     ON schedules (next_run_at) WHERE status = 'active' AND deleted_at IS NULL;
CREATE INDEX idx_schedules_deleted_at      ON schedules (deleted_at) WHERE deleted_at IS NOT NULL;

CREATE TABLE schedule_endpoints (
    schedule_id UUID        NOT NULL REFERENCES schedules(id),
    endpoint_id UUID        NOT NULL REFERENCES endpoints(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT schedule_endpoints_pk UNIQUE (schedule_id, endpoint_id)
);

CREATE INDEX idx_schedule_endpoints_endpoint_id ON schedule_endpoints (endpoint_id);

CREATE TABLE schedule_executions (
    id              UUID        NOT NULL,
    public_id       VARCHAR(24) NOT NULL,
    schedule_id     UUID        NOT NULL REFERENCES schedules(id),
    tenant_id       UUID        NOT NULL,
    organization_id UUID        NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'pending',
    triggered_at    TIMESTAMPTZ NOT NULL,
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    error_message   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT schedule_executions_pk           PRIMARY KEY (id),
    CONSTRAINT schedule_executions_public_id_uq UNIQUE (public_id),
    CONSTRAINT schedule_executions_status_valid CHECK (
        status IN ('pending', 'running', 'success', 'partial_failure', 'failure')
    )
);

CREATE INDEX idx_schedule_executions_schedule_id  ON schedule_executions (schedule_id);
CREATE INDEX idx_schedule_executions_triggered_at ON schedule_executions (triggered_at DESC);

CREATE TABLE schedules_audits (
    id              BIGSERIAL   PRIMARY KEY,
    entity_id       UUID        NOT NULL,
    public_id       VARCHAR(24) NOT NULL,
    tenant_id       UUID        NOT NULL,
    organization_id UUID        NOT NULL,
    request_id      UUID        NOT NULL,
    operation       VARCHAR(10) NOT NULL,
    old_data        JSONB,
    new_data        JSONB,
    created_by      UUID        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_schedules_audits_entity_id ON schedules_audits (entity_id);

CREATE OR REPLACE FUNCTION audit_schedule()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO schedules_audits (
            entity_id, public_id, tenant_id, organization_id,
            request_id, operation, old_data, new_data, created_by
        ) VALUES (
            OLD.id, OLD.public_id, OLD.tenant_id, OLD.organization_id,
            OLD.request_id, 'DELETE', to_jsonb(OLD), NULL, OLD.updated_by
        );
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO schedules_audits (
            entity_id, public_id, tenant_id, organization_id,
            request_id, operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id,
            NEW.request_id, 'UPDATE', to_jsonb(OLD), to_jsonb(NEW), NEW.updated_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'INSERT' THEN
        INSERT INTO schedules_audits (
            entity_id, public_id, tenant_id, organization_id,
            request_id, operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id,
            NEW.request_id, 'INSERT', NULL, to_jsonb(NEW), NEW.created_by
        );
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_audit_schedule
AFTER INSERT OR UPDATE OR DELETE ON schedules
FOR EACH ROW EXECUTE FUNCTION audit_schedule();
