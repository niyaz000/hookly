INSERT INTO permissions (id, public_id, tenant_id, name, description, perm_type, resource, action) VALUES
-- applications
(gen_random_uuid(), 'per_app_read',    NULL, 'applications:read',   'Read applications',              'system', 'applications', 'read'),
(gen_random_uuid(), 'per_app_write',   NULL, 'applications:write',  'Create and update applications', 'system', 'applications', 'write'),
(gen_random_uuid(), 'per_app_delete',  NULL, 'applications:delete', 'Delete applications',            'system', 'applications', 'delete'),
-- endpoints
(gen_random_uuid(), 'per_ep_read',     NULL, 'endpoints:read',      'Read endpoints',                 'system', 'endpoints',    'read'),
(gen_random_uuid(), 'per_ep_write',    NULL, 'endpoints:write',     'Create and update endpoints',    'system', 'endpoints',    'write'),
(gen_random_uuid(), 'per_ep_delete',   NULL, 'endpoints:delete',    'Delete endpoints',               'system', 'endpoints',    'delete'),
-- event_types
(gen_random_uuid(), 'per_et_read',     NULL, 'event_types:read',    'Read event types',               'system', 'event_types',  'read'),
(gen_random_uuid(), 'per_et_write',    NULL, 'event_types:write',   'Create and update event types',  'system', 'event_types',  'write'),
(gen_random_uuid(), 'per_et_delete',   NULL, 'event_types:delete',  'Delete event types',             'system', 'event_types',  'delete'),
-- events
(gen_random_uuid(), 'per_ev_read',     NULL, 'events:read',         'Read events',                    'system', 'events',       'read'),
(gen_random_uuid(), 'per_ev_send',     NULL, 'events:send',         'Send and trigger events',        'system', 'events',       'send'),
(gen_random_uuid(), 'per_ev_delete',   NULL, 'events:delete',       'Delete events',                  'system', 'events',       'delete'),
-- schedules
(gen_random_uuid(), 'per_sc_read',     NULL, 'schedules:read',      'Read schedules',                 'system', 'schedules',    'read'),
(gen_random_uuid(), 'per_sc_write',    NULL, 'schedules:write',     'Create and update schedules',    'system', 'schedules',    'write'),
(gen_random_uuid(), 'per_sc_delete',   NULL, 'schedules:delete',    'Delete schedules',               'system', 'schedules',    'delete'),
-- environments
(gen_random_uuid(), 'per_env_read',    NULL, 'environments:read',   'Read environments',              'system', 'environments', 'read'),
(gen_random_uuid(), 'per_env_write',   NULL, 'environments:write',  'Create and update environments', 'system', 'environments', 'write'),
-- api_keys
(gen_random_uuid(), 'per_ak_read',     NULL, 'api_keys:read',       'Read API keys',                  'system', 'api_keys',     'read'),
(gen_random_uuid(), 'per_ak_write',    NULL, 'api_keys:write',      'Create and update API keys',     'system', 'api_keys',     'write'),
(gen_random_uuid(), 'per_ak_delete',   NULL, 'api_keys:delete',     'Delete API keys',                'system', 'api_keys',     'delete'),
-- jwt_keys
(gen_random_uuid(), 'per_jk_read',     NULL, 'jwt_keys:read',       'Read JWT keys',                  'system', 'jwt_keys',     'read'),
(gen_random_uuid(), 'per_jk_write',    NULL, 'jwt_keys:write',      'Create and update JWT keys',     'system', 'jwt_keys',     'write'),
(gen_random_uuid(), 'per_jk_delete',   NULL, 'jwt_keys:delete',     'Delete JWT keys',                'system', 'jwt_keys',     'delete'),
(gen_random_uuid(), 'per_jk_rotate',   NULL, 'jwt_keys:rotate',     'Rotate JWT keys',                'system', 'jwt_keys',     'rotate'),
-- users
(gen_random_uuid(), 'per_usr_read',    NULL, 'users:read',          'Read users',                     'system', 'users',        'read'),
(gen_random_uuid(), 'per_usr_write',   NULL, 'users:write',         'Create and update users',        'system', 'users',        'write'),
(gen_random_uuid(), 'per_usr_delete',  NULL, 'users:delete',        'Delete users',                   'system', 'users',        'delete'),
-- teams
(gen_random_uuid(), 'per_tm_read',     NULL, 'teams:read',          'Read teams',                     'system', 'teams',        'read'),
(gen_random_uuid(), 'per_tm_write',    NULL, 'teams:write',         'Create and update teams',        'system', 'teams',        'write'),
(gen_random_uuid(), 'per_tm_delete',   NULL, 'teams:delete',        'Delete teams',                   'system', 'teams',        'delete'),
-- roles
(gen_random_uuid(), 'per_rol_read',    NULL, 'roles:read',          'Read roles',                     'system', 'roles',        'read'),
(gen_random_uuid(), 'per_rol_write',   NULL, 'roles:write',         'Create and update roles',        'system', 'roles',        'write'),
(gen_random_uuid(), 'per_rol_delete',  NULL, 'roles:delete',        'Delete roles',                   'system', 'roles',        'delete'),
-- permissions
(gen_random_uuid(), 'per_perm_read',   NULL, 'permissions:read',    'Read permissions',               'system', 'permissions',  'read'),
(gen_random_uuid(), 'per_perm_write',  NULL, 'permissions:write',   'Create and update permissions',  'system', 'permissions',  'write'),
(gen_random_uuid(), 'per_perm_delete', NULL, 'permissions:delete',  'Delete permissions',             'system', 'permissions',  'delete'),
-- invites
(gen_random_uuid(), 'per_inv_read',    NULL, 'invites:read',        'Read invites',                   'system', 'invites',      'read'),
(gen_random_uuid(), 'per_inv_write',   NULL, 'invites:write',       'Create invites',                 'system', 'invites',      'write'),
-- tenant
(gen_random_uuid(), 'per_ten_read',    NULL, 'tenant:read',         'Read tenant settings',           'system', 'tenant',       'read'),
(gen_random_uuid(), 'per_ten_write',   NULL, 'tenant:write',        'Update tenant settings',         'system', 'tenant',       'write'),
-- wildcard
(gen_random_uuid(), 'per_all',         NULL, '*:*',                 'All permissions (super admin)',   'system', '*',            '*')
ON CONFLICT DO NOTHING;
