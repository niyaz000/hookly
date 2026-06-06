CREATE TABLE api_key_roles (
    api_key_public_id  VARCHAR(20)  NOT NULL,
    role_id            UUID         NOT NULL,
    created_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by         UUID         NOT NULL,
    PRIMARY KEY (api_key_public_id, role_id)
);

CREATE TABLE api_key_permissions (
    api_key_public_id  VARCHAR(20)  NOT NULL,
    permission_id      UUID         NOT NULL,
    expires_at         TIMESTAMPTZ  NULL,
    created_at         TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_by         UUID         NOT NULL,
    PRIMARY KEY (api_key_public_id, permission_id)
);
