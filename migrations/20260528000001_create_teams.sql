CREATE TABLE teams (
    id              UUID         NOT NULL,
    public_id       VARCHAR(24)  NOT NULL,
    name            VARCHAR(255) NOT NULL,
    tenant_id       UUID         NOT NULL,
    organization_id UUID         NOT NULL,
    description     TEXT,
    tags            JSONB        NOT NULL DEFAULT '{}',
    metadata        JSONB        NOT NULL DEFAULT '{}',
    settings        JSONB        NOT NULL DEFAULT '{}',
    created_by      UUID         NOT NULL,
    updated_by      UUID         NOT NULL,
    request_id      UUID         NOT NULL,
    version         INTEGER      NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT teams_pk            PRIMARY KEY (id),
    CONSTRAINT teams_public_id_uq  UNIQUE (public_id),
    CONSTRAINT teams_name_nonempty CHECK (char_length(trim(name)) >= 1),
    CONSTRAINT teams_version_pos   CHECK (version > 0)
);

CREATE INDEX idx_teams_organization_id ON teams (organization_id);
CREATE INDEX idx_teams_tenant_id       ON teams (tenant_id);
CREATE INDEX idx_teams_created_at      ON teams (created_at);
CREATE INDEX idx_teams_deleted_at      ON teams (deleted_at) WHERE deleted_at IS NOT NULL;

-- ============================================================

CREATE TABLE team_members (
    id              UUID         NOT NULL,
    public_id       VARCHAR(24)  NOT NULL,
    tenant_id       UUID         NOT NULL,
    organization_id UUID         NOT NULL,
    team_id         UUID         NOT NULL REFERENCES teams (id),
    user_id         UUID         NOT NULL,
    created_by      UUID         NOT NULL,
    updated_by      UUID         NOT NULL,
    request_id      UUID         NOT NULL,
    version         INTEGER      NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT team_members_pk           PRIMARY KEY (id),
    CONSTRAINT team_members_public_id_uq UNIQUE (public_id),
    CONSTRAINT team_members_team_user_uq UNIQUE (team_id, user_id),
    CONSTRAINT team_members_version_pos  CHECK (version > 0)
);

CREATE INDEX idx_team_members_team_id    ON team_members (team_id);
CREATE INDEX idx_team_members_user_id    ON team_members (user_id);
CREATE INDEX idx_team_members_deleted_at ON team_members (deleted_at) WHERE deleted_at IS NOT NULL;

-- ============================================================

CREATE TABLE teams_audits (
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

CREATE INDEX idx_teams_audits_entity_id  ON teams_audits (entity_id, created_at);
CREATE INDEX idx_teams_audits_created_at ON teams_audits (created_at);

-- ============================================================

CREATE TABLE team_members_audits (
    id              BIGSERIAL   PRIMARY KEY,
    entity_id       UUID        NOT NULL,
    public_id       VARCHAR(24) NOT NULL,
    tenant_id       UUID        NOT NULL,
    organization_id UUID        NOT NULL,
    team_id         UUID        NOT NULL,
    request_id      UUID        NOT NULL,
    operation       VARCHAR(10) NOT NULL,
    old_data        JSONB,
    new_data        JSONB,
    created_by      UUID        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_team_members_audits_entity_id  ON team_members_audits (entity_id, created_at);
CREATE INDEX idx_team_members_audits_created_at ON team_members_audits (created_at);

-- ============================================================
-- audit trigger: teams
-- ============================================================
CREATE OR REPLACE FUNCTION audit_teams()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO teams_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'INSERT', NULL, to_jsonb(NEW), NEW.created_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO teams_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'UPDATE', to_jsonb(OLD), to_jsonb(NEW), NEW.updated_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO teams_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            OLD.id, OLD.public_id, OLD.tenant_id, OLD.organization_id, OLD.request_id,
            'DELETE', to_jsonb(OLD), NULL, OLD.updated_by
        );
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_teams ON teams;
CREATE TRIGGER trg_audit_teams
    AFTER INSERT OR UPDATE OR DELETE ON teams
    FOR EACH ROW EXECUTE FUNCTION audit_teams();

-- ============================================================
-- audit trigger: team_members
-- ============================================================
CREATE OR REPLACE FUNCTION audit_team_members()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO team_members_audits (
            entity_id, public_id, tenant_id, organization_id, team_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.team_id, NEW.request_id,
            'INSERT', NULL, to_jsonb(NEW), NEW.created_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO team_members_audits (
            entity_id, public_id, tenant_id, organization_id, team_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.team_id, NEW.request_id,
            'UPDATE', to_jsonb(OLD), to_jsonb(NEW), NEW.updated_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO team_members_audits (
            entity_id, public_id, tenant_id, organization_id, team_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            OLD.id, OLD.public_id, OLD.tenant_id, OLD.organization_id, OLD.team_id, OLD.request_id,
            'DELETE', to_jsonb(OLD), NULL, OLD.updated_by
        );
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_team_members ON team_members;
CREATE TRIGGER trg_audit_team_members
    AFTER INSERT OR UPDATE OR DELETE ON team_members
    FOR EACH ROW EXECUTE FUNCTION audit_team_members();
