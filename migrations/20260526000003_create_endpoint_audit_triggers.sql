-- ============================================================
-- endpoints
-- ============================================================
CREATE OR REPLACE FUNCTION audit_endpoints()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO endpoints_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'INSERT', NULL, to_jsonb(NEW), NEW.created_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO endpoints_audits (
            entity_id, public_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'UPDATE', to_jsonb(OLD), to_jsonb(NEW), NEW.updated_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO endpoints_audits (
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

DROP TRIGGER IF EXISTS trg_audit_endpoints ON endpoints;
CREATE TRIGGER trg_audit_endpoints
    AFTER INSERT OR UPDATE OR DELETE ON endpoints
    FOR EACH ROW EXECUTE FUNCTION audit_endpoints();

-- ============================================================
-- endpoint_secrets
-- The 'secret' column is stripped from both old_data and new_data
-- to prevent encrypted secrets from persisting in the audit log.
-- ============================================================
CREATE OR REPLACE FUNCTION audit_endpoint_secrets()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO endpoint_secrets_audits (
            entity_id, public_id, endpoint_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.endpoint_id,
            NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'INSERT', NULL, to_jsonb(NEW) - 'secret', NEW.created_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        INSERT INTO endpoint_secrets_audits (
            entity_id, public_id, endpoint_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            NEW.id, NEW.public_id, NEW.endpoint_id,
            NEW.tenant_id, NEW.organization_id, NEW.request_id,
            'UPDATE', to_jsonb(OLD) - 'secret', to_jsonb(NEW) - 'secret', NEW.created_by
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        INSERT INTO endpoint_secrets_audits (
            entity_id, public_id, endpoint_id, tenant_id, organization_id, request_id,
            operation, old_data, new_data, created_by
        ) VALUES (
            OLD.id, OLD.public_id, OLD.endpoint_id,
            OLD.tenant_id, OLD.organization_id, OLD.request_id,
            'DELETE', to_jsonb(OLD) - 'secret', NULL, OLD.created_by
        );
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_endpoint_secrets ON endpoint_secrets;
CREATE TRIGGER trg_audit_endpoint_secrets
    AFTER INSERT OR UPDATE OR DELETE ON endpoint_secrets
    FOR EACH ROW EXECUTE FUNCTION audit_endpoint_secrets();
