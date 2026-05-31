-- invites -----------------------------------------------------------------------

CREATE TABLE invites (
    id              UUID            NOT NULL,
    public_id       VARCHAR(24)     NOT NULL,
    tenant_id       UUID            NOT NULL REFERENCES tenants(id),
    organization_id UUID            NOT NULL REFERENCES organizations(id),
    user_email      VARCHAR(255)    NOT NULL,
    role            VARCHAR(50)     NOT NULL,
    status          VARCHAR(20)     NOT NULL DEFAULT 'sent',
    token_hash      TEXT            NOT NULL,
    tags            JSONB           NOT NULL DEFAULT '{}',
    metadata        JSONB           NOT NULL DEFAULT '{}',
    created_by      UUID            NOT NULL,
    request_id      UUID            NOT NULL,
    version         INTEGER         NOT NULL DEFAULT 1,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,
    revoked_at      TIMESTAMPTZ,
    accepted_at     TIMESTAMPTZ,
    expires_at      TIMESTAMPTZ     NOT NULL,

    CONSTRAINT invites_pk             PRIMARY KEY (id),
    CONSTRAINT invites_public_id_uq   UNIQUE (public_id),
    CONSTRAINT invites_token_hash_uq  UNIQUE (token_hash),
    CONSTRAINT invites_email_nonempty CHECK (char_length(trim(user_email)) >= 1),
    CONSTRAINT invites_role_nonempty  CHECK (char_length(trim(role)) >= 1),
    CONSTRAINT invites_version_pos    CHECK (version > 0),
    CONSTRAINT invites_status_valid   CHECK (
        status IN ('failed', 'sent', 'opened', 'accepted', 'expired', 'revoked')
    )
);

CREATE INDEX idx_invites_tenant_id       ON invites (tenant_id);
CREATE INDEX idx_invites_organization_id ON invites (organization_id);
CREATE INDEX idx_invites_user_email      ON invites (user_email);
CREATE INDEX idx_invites_status          ON invites (status) WHERE deleted_at IS NULL;
CREATE INDEX idx_invites_token_hash      ON invites (token_hash);
CREATE INDEX idx_invites_expires_at      ON invites (expires_at) WHERE status IN ('sent', 'opened', 'failed');

-- tenant_members ---------------------------------------------------------------

CREATE TABLE tenant_members (
    id              UUID            NOT NULL,
    public_id       VARCHAR(24)     NOT NULL,
    tenant_id       UUID            NOT NULL REFERENCES tenants(id),
    organization_id UUID            NOT NULL REFERENCES organizations(id),
    invite_id       UUID            NOT NULL REFERENCES invites(id),
    user_email      VARCHAR(255)    NOT NULL,
    user_id         UUID            REFERENCES identity.users(id),
    role            VARCHAR(50)     NOT NULL,
    status          VARCHAR(20)     NOT NULL DEFAULT 'active',
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_at      TIMESTAMPTZ,

    CONSTRAINT tenant_members_pk        PRIMARY KEY (id),
    CONSTRAINT tenant_members_pubid_uq  UNIQUE (public_id),
    CONSTRAINT tenant_members_invite_uq UNIQUE (invite_id),
    CONSTRAINT tenant_members_status_valid CHECK (status IN ('active', 'disabled'))
);

CREATE UNIQUE INDEX tenant_members_active_uq
    ON tenant_members (tenant_id, user_email)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_tenant_members_tenant_id ON tenant_members (tenant_id);
CREATE INDEX idx_tenant_members_user_email ON tenant_members (user_email);

-- invites_audits ---------------------------------------------------------------

CREATE TABLE invites_audits (
    id          UUID        NOT NULL DEFAULT gen_random_uuid(),
    table_name  VARCHAR(50) NOT NULL DEFAULT 'invites',
    record_id   UUID        NOT NULL,
    operation   VARCHAR(10) NOT NULL,
    old_data    JSONB,
    new_data    JSONB,
    changed_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT invites_audits_pk PRIMARY KEY (id)
);

CREATE INDEX idx_invites_audits_record_id ON invites_audits (record_id);

CREATE OR REPLACE FUNCTION audit_invite() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        INSERT INTO invites_audits (record_id, operation, old_data)
        VALUES (OLD.id, 'DELETE', to_jsonb(OLD));
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO invites_audits (record_id, operation, old_data, new_data)
        VALUES (OLD.id, 'UPDATE', to_jsonb(OLD), to_jsonb(NEW));
        RETURN NEW;
    ELSE
        INSERT INTO invites_audits (record_id, operation, new_data)
        VALUES (NEW.id, 'INSERT', to_jsonb(NEW));
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_audit_invite
    AFTER INSERT OR UPDATE OR DELETE ON invites
    FOR EACH ROW EXECUTE FUNCTION audit_invite();
