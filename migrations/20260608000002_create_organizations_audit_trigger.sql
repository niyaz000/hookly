CREATE OR REPLACE FUNCTION audit_organizations()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO organizations_audits (
            entity_id, public_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.request_id,
            'INSERT', NULL, to_jsonb(NEW), NEW.created_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO organizations_audits (
            entity_id, public_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.request_id,
            'UPDATE', to_jsonb(OLD), to_jsonb(NEW), NEW.updated_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO organizations_audits (
            entity_id, public_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            OLD.id, OLD.public_id, OLD.request_id,
            'DELETE', to_jsonb(OLD), NULL, OLD.updated_by
        );
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_organizations ON organizations;
CREATE TRIGGER trg_audit_organizations
    AFTER INSERT OR UPDATE OR DELETE ON organizations
    FOR EACH ROW EXECUTE FUNCTION audit_organizations();
