CREATE TABLE roles (
    id          UUID         NOT NULL,
    public_id   VARCHAR(20)  NOT NULL,
    tenant_id   UUID         NOT NULL,
    name        VARCHAR(128) NOT NULL,
    description TEXT,
    is_system   BOOLEAN      NOT NULL DEFAULT false,
    version     INT          NOT NULL DEFAULT 0,
    created_by  UUID         NOT NULL,
    updated_by  UUID         NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    deleted_at  TIMESTAMPTZ  NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX roles_public_id_uq    ON roles (public_id);
CREATE UNIQUE INDEX roles_tenant_name_uq  ON roles (tenant_id, name) WHERE deleted_at IS NULL;
