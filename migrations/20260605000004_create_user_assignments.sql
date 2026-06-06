CREATE TABLE user_roles (
    user_public_id  VARCHAR(20)  NOT NULL,
    role_id         UUID         NOT NULL,
    tenant_id       UUID         NOT NULL,
    expires_at      TIMESTAMPTZ  NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by      UUID         NOT NULL,
    PRIMARY KEY (user_public_id, role_id)
);

CREATE TABLE user_permissions (
    user_public_id  VARCHAR(20)  NOT NULL,
    permission_id   UUID         NOT NULL,
    tenant_id       UUID         NOT NULL,
    expires_at      TIMESTAMPTZ  NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by      UUID         NOT NULL,
    PRIMARY KEY (user_public_id, permission_id)
);
