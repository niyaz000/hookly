-- Relax global email uniqueness to org-scoped.
-- The same email can now own multiple organizations (platform owner, reseller orgs, etc.).
ALTER TABLE identity.users DROP CONSTRAINT users_email_uq;
ALTER TABLE identity.users ADD CONSTRAINT users_email_org_uq UNIQUE (organization_id, email);
