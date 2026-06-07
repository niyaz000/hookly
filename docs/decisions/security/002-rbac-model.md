# ADR-005: RBAC model with scoped assignments

## Status
Accepted

## Context

Hookly serves multiple tenants, each of which may have multiple users with different levels of access. An organization-wide admin should have different capabilities than a read-only auditor or an application-specific developer. Additionally, API keys — not just human users — need to carry permissions so that programmatic integrations can be scoped appropriately.

Design questions:

1. **Flat or hierarchical roles?** A simple list of roles vs. a tree that inherits permissions
2. **Permission granularity?** Coarse (admin/viewer) vs. fine-grained (per-resource per-action)
3. **Scope?** Global per tenant vs. per application vs. per environment
4. **Principal types?** Users only, or also API keys?

## Decision

### Three-layer model: Permissions → Roles → Assignments

**Permissions** are atomic capabilities defined by the platform (seeded via migration). Each permission has a `resource` and `action` (e.g., `resource = "endpoint"`, `action = "delete"`). They are not user-defined.

**Roles** bundle permissions. The platform seeds system roles; tenants can define custom roles. A `role_permissions` join table links roles to their permissions.

**Assignments** bind a principal (user or API key) to a role, with an optional scope (organization, tenant, application). The `principal_type` column distinguishes `user` from `api_key`.

```
Permission: { resource, action }
Role: { name, is_system } → [Permission, ...]
Assignment: { principal_id, principal_type, role_id, scope_type, scope_id }
```

### Flat roles (no inheritance)

Role inheritance (roles that extend other roles) adds meaningful complexity at evaluation time — recursive SQL or in-memory tree traversal. Flat roles are sufficient for the current use cases: system roles cover the common cases, and custom roles provide tenant flexibility without needing inheritance.

### Fine-grained permissions

Coarse `admin/viewer` roles are quick to implement but produce permission gaps (e.g., a user who can manage endpoints but not delete them). Seeding 40+ fine-grained permissions (create/read/update/delete/list per resource) gives tenants precise control. The permissions table is read-only from the API — tenants cannot define new permissions.

### Dual principal types

Both users and API keys can have role assignments. This means programmatic integrations get the minimum necessary permissions rather than inheriting the full set. An API key used for read-only reporting cannot accidentally delete a webhook endpoint.

## Consequences

**Positive:**
- A single assignment check path covers both human and programmatic access
- Fine-grained permissions reduce over-provisioning
- Scoped assignments enable application-level isolation within a tenant

**Negative:**
- 40+ system permissions to seed and maintain; adding a new resource requires a migration to add its permissions
- Assignment evaluation at request time requires a join across `user_assignments`/`api_key_assignments` → `roles` → `role_permissions` → `permissions` — covered by indexes but more complex than a simple `user.role` column
- No role inheritance means common permission bundles must be manually repeated in multiple roles (acceptable for the current scale)
