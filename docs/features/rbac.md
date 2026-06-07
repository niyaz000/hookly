# RBAC — Roles, permissions, and assignments

Hookly uses a three-layer role-based access control model that applies to both human users and API keys.

---

## Model

```
Permission  ─┐
Permission  ─┤──→ Role ──→ Assignment ──→ Principal (user or API key)
Permission  ─┘              └── scope (organization / tenant / application)
```

### Permissions

Atomic platform-defined capabilities. Each permission has a `resource` and `action`. They are seeded by migration and cannot be created or deleted via the API.

Examples:
```
resource=endpoint    action=create
resource=endpoint    action=delete
resource=api_key     action=rotate
resource=role        action=assign
```

### Roles

Named bundles of permissions. Two kinds:

| Kind | Created by | Deletable |
|---|---|---|
| System roles | Seeded by migration | No |
| Custom roles | Tenant via API | Yes (if no active assignments) |

A `role_permissions` join table links roles to their permissions.

### Assignments

An assignment binds a principal to a role with an optional scope:

| Field | Description |
|---|---|
| `principal_id` | UUID of the user or API key |
| `principal_type` | `user` or `api_key` |
| `role_id` | The role being assigned |
| `scope_type` | `organization`, `tenant`, or `application` (nullable for global) |
| `scope_id` | UUID of the scoped resource (nullable for global) |

A user scoped to `tenant` + `<tenant_uuid>` can only act within that tenant. A user scoped to `application` + `<app_uuid>` can only act within that application.

---

## API

### Permissions

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/v1/permissions` | List all system permissions |
| `GET` | `/api/v1/permissions/:id` | Get a single permission |

### Roles

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/v1/roles` | Create a custom role |
| `GET` | `/api/v1/roles` | List roles (filterable by `is_system`) |
| `GET` | `/api/v1/roles/:id` | Get a role with its permissions |
| `PATCH` | `/api/v1/roles/:id` | Update name / permissions |
| `DELETE` | `/api/v1/roles/:id` | Delete (fails if active assignments exist) |

### Assignments

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/v1/assignments` | Assign a role to a principal |
| `GET` | `/api/v1/assignments` | List assignments (filterable by principal, role, scope) |
| `DELETE` | `/api/v1/assignments/:id` | Revoke an assignment |

---

## Example: creating and assigning a custom role

**1. Create a role:**
```http
POST /api/v1/roles
{
  "name": "endpoint-manager",
  "permission_ids": ["perm_ep_create", "perm_ep_update", "perm_ep_list"]
}
```

**2. Assign it to a user, scoped to a specific tenant:**
```http
POST /api/v1/assignments
{
  "principal_id": "<user_uuid>",
  "principal_type": "user",
  "role_id": "rol_...",
  "scope_type": "tenant",
  "scope_id": "<tenant_uuid>"
}
```

**3. Assign the same role to an API key:**
```http
POST /api/v1/assignments
{
  "principal_id": "<api_key_uuid>",
  "principal_type": "api_key",
  "role_id": "rol_...",
  "scope_type": "tenant",
  "scope_id": "<tenant_uuid>"
}
```

---

## Design notes

- **No role inheritance.** Roles cannot extend other roles. Common permission bundles are repeated across roles explicitly. This keeps permission evaluation to a single JOIN chain with no recursion.
- **Flat permission evaluation.** Checking "does principal X have permission Y in scope Z?" is a single query: join assignments → roles → role_permissions → permissions.
- **API keys as first-class principals.** An API key used for a read-only integration should carry only read permissions. Over-provisioning a key with admin permissions is a conscious choice, not a default.
- **System roles cannot be deleted.** They are seeded with a stable `is_system = true` flag and the delete handler rejects attempts to remove them.
