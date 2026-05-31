DROP TABLE IF EXISTS public.users;
DROP TYPE IF EXISTS user_status;

CREATE SCHEMA IF NOT EXISTS identity;

CREATE TYPE user_status AS ENUM ('active', 'suspended', 'inactive', 'locked');

CREATE TABLE identity.users (
    id                UUID            NOT NULL,
    public_id         VARCHAR(24)     NOT NULL,
    organization_id   UUID            NOT NULL,
    tenant_id         UUID            NOT NULL,
    email             VARCHAR(64)     NOT NULL,
    phone             VARCHAR(13),
    status            user_status     NOT NULL DEFAULT 'active',
    email_verified_at TIMESTAMPTZ,
    phone_verified_at TIMESTAMPTZ,
    last_active_at    TIMESTAMPTZ,
    metadata          JSONB           NOT NULL DEFAULT '{}',
    tags              JSONB           NOT NULL DEFAULT '{}',
    settings          JSONB           NOT NULL DEFAULT '{}',
    password_hash     VARCHAR(255),
    version           INTEGER         NOT NULL DEFAULT 1,
    created_by        UUID            NOT NULL,
    updated_by        UUID            NOT NULL,
    request_id        UUID            NOT NULL,
    locked_until      TIMESTAMPTZ,
    login_count       INTEGER         NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    deleted_at        TIMESTAMPTZ,
    CONSTRAINT users_pk                 PRIMARY KEY (id),
    CONSTRAINT users_public_id_uq       UNIQUE (public_id),
    CONSTRAINT users_email_uq           UNIQUE (email),
    CONSTRAINT users_version_positive   CHECK (version > 0),
    CONSTRAINT users_email_format       CHECK (
        email ~* '^[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}$'
    ),
    CONSTRAINT users_login_count_nonneg CHECK (login_count >= 0)
);

CREATE INDEX idx_users_organization_id ON identity.users (organization_id);
CREATE INDEX idx_users_tenant_id       ON identity.users (tenant_id);
CREATE INDEX idx_users_status          ON identity.users (status);
CREATE INDEX idx_users_email           ON identity.users (email);
CREATE INDEX idx_users_created_at      ON identity.users (created_at);
CREATE INDEX idx_users_deleted_at      ON identity.users (deleted_at)
    WHERE deleted_at IS NOT NULL;
