-- users_audits: relax NOT NULL on fields the users table doesn't have
ALTER TABLE users_audits ALTER COLUMN request_id DROP NOT NULL;
ALTER TABLE users_audits ALTER COLUMN created_by DROP NOT NULL;

-- ============================================================
-- users
-- ============================================================
CREATE OR REPLACE FUNCTION audit_users()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO users_audits (entity_id, operation, old_data, new_data)
        VALUES (NEW.id, 'INSERT', NULL, to_jsonb(NEW));
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO users_audits (entity_id, operation, old_data, new_data)
        VALUES (NEW.id, 'UPDATE', to_jsonb(OLD), to_jsonb(NEW));
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO users_audits (entity_id, operation, old_data, new_data)
        VALUES (OLD.id, 'DELETE', to_jsonb(OLD), NULL);
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_users ON identity.users;
CREATE TRIGGER trg_audit_users
    AFTER INSERT OR UPDATE OR DELETE ON identity.users
    FOR EACH ROW EXECUTE FUNCTION audit_users();

-- ============================================================
-- applications
-- INSERT  → created_by from created_by
-- UPDATE  → created_by from updated_by (whoever triggered this change)
-- DELETE  → created_by from updated_by (last actor on the row)
-- ============================================================
CREATE OR REPLACE FUNCTION audit_applications()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO applications_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'INSERT', NULL, to_jsonb(NEW), NEW.created_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO applications_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'UPDATE', to_jsonb(OLD), to_jsonb(NEW), NEW.updated_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO applications_audits (
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

DROP TRIGGER IF EXISTS trg_audit_applications ON applications;
CREATE TRIGGER trg_audit_applications
    AFTER INSERT OR UPDATE OR DELETE ON applications
    FOR EACH ROW EXECUTE FUNCTION audit_applications();

-- ============================================================
-- event_types
-- ============================================================
CREATE OR REPLACE FUNCTION audit_event_types()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO event_types_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'INSERT', NULL, to_jsonb(NEW), NEW.created_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO event_types_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'UPDATE', to_jsonb(OLD), to_jsonb(NEW), NEW.updated_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO event_types_audits (
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

DROP TRIGGER IF EXISTS trg_audit_event_types ON event_types;
CREATE TRIGGER trg_audit_event_types
    AFTER INSERT OR UPDATE OR DELETE ON event_types
    FOR EACH ROW EXECUTE FUNCTION audit_event_types();
