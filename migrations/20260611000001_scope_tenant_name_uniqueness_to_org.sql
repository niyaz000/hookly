-- Tenant names must be unique within an organization (not globally).
-- Soft-deleted tenants are excluded so the same name can be reused after deletion.
ALTER TABLE tenants DROP CONSTRAINT IF EXISTS tenants_name_uq;

CREATE UNIQUE INDEX idx_tenants_name_uq
    ON tenants (organization_id, name)
    WHERE deleted_at IS NULL;
