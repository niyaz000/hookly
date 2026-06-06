CREATE TYPE permission_type AS ENUM ('system', 'custom');

CREATE TABLE permissions (
    id          UUID            NOT NULL,
    public_id   VARCHAR(20)     NOT NULL,
    tenant_id   UUID            NULL,
    name        VARCHAR(128)    NOT NULL,
    description TEXT,
    perm_type   permission_type NOT NULL DEFAULT 'custom',
    resource    VARCHAR(64)     NOT NULL,
    action      VARCHAR(64)     NOT NULL,
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX permissions_public_id_uq  ON permissions (public_id);
CREATE UNIQUE INDEX permissions_system_name_uq ON permissions (name) WHERE tenant_id IS NULL;
CREATE UNIQUE INDEX permissions_tenant_name_uq ON permissions (tenant_id, name) WHERE tenant_id IS NOT NULL;
