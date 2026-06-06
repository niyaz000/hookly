CREATE TABLE platform_event_types (
    id          UUID         NOT NULL,
    public_id   VARCHAR(20)  NOT NULL,
    name        VARCHAR(128) NOT NULL,
    description TEXT,
    resource    VARCHAR(64)  NOT NULL,
    action      VARCHAR(64)  NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id)
);
CREATE UNIQUE INDEX pet_public_id_uq ON platform_event_types (public_id);
CREATE UNIQUE INDEX pet_name_uq ON platform_event_types (name);
CREATE INDEX pet_resource_idx ON platform_event_types (resource);

INSERT INTO platform_event_types (id, public_id, name, description, resource, action) VALUES
  (gen_random_uuid(), 'pet_apk_created',  'api_key.created',      'An API key was created',                  'api_key',     'created'),
  (gen_random_uuid(), 'pet_apk_updated',  'api_key.updated',      'An API key was updated',                  'api_key',     'updated'),
  (gen_random_uuid(), 'pet_apk_deleted',  'api_key.deleted',      'An API key was deleted',                  'api_key',     'deleted'),
  (gen_random_uuid(), 'pet_apk_rotated',  'api_key.rotated',      'An API key was rotated',                  'api_key',     'rotated'),
  (gen_random_uuid(), 'pet_env_created',  'environment.created',  'An environment was created',              'environment', 'created'),
  (gen_random_uuid(), 'pet_env_updated',  'environment.updated',  'An environment was updated',              'environment', 'updated'),
  (gen_random_uuid(), 'pet_env_deleted',  'environment.deleted',  'An environment was deleted',              'environment', 'deleted'),
  (gen_random_uuid(), 'pet_env_disabled', 'environment.disabled', 'An environment was disabled',             'environment', 'disabled'),
  (gen_random_uuid(), 'pet_jwk_created',  'jwt_key.created',      'A JWT key was created',                   'jwt_key',     'created'),
  (gen_random_uuid(), 'pet_jwk_rotated',  'jwt_key.rotated',      'A JWT key was rotated',                   'jwt_key',     'rotated'),
  (gen_random_uuid(), 'pet_jwk_disabled', 'jwt_key.disabled',     'A JWT key was disabled',                  'jwt_key',     'disabled'),
  (gen_random_uuid(), 'pet_jwk_deleted',  'jwt_key.deleted',      'A JWT key was deleted',                   'jwt_key',     'deleted'),
  (gen_random_uuid(), 'pet_rol_created',  'role.created',         'A role was created',                      'role',        'created'),
  (gen_random_uuid(), 'pet_rol_updated',  'role.updated',         'A role was updated',                      'role',        'updated'),
  (gen_random_uuid(), 'pet_rol_deleted',  'role.deleted',         'A role was deleted',                      'role',        'deleted'),
  (gen_random_uuid(), 'pet_usr_invited',  'user.invited',         'A user was invited',                      'user',        'invited'),
  (gen_random_uuid(), 'pet_usr_joined',   'user.joined',          'A user joined',                           'user',        'joined'),
  (gen_random_uuid(), 'pet_usr_deleted',  'user.deleted',         'A user was deleted',                      'user',        'deleted'),
  (gen_random_uuid(), 'pet_usr_rol_asgn', 'user.role_assigned',   'A role was assigned to a user',           'user',        'role_assigned'),
  (gen_random_uuid(), 'pet_usr_rol_rmvd', 'user.role_removed',    'A role was removed from a user',          'user',        'role_removed'),
  (gen_random_uuid(), 'pet_ep_created',   'endpoint.created',     'A webhook endpoint was created',          'endpoint',    'created'),
  (gen_random_uuid(), 'pet_ep_updated',   'endpoint.updated',     'A webhook endpoint was updated',          'endpoint',    'updated'),
  (gen_random_uuid(), 'pet_ep_deleted',   'endpoint.deleted',     'A webhook endpoint was deleted',          'endpoint',    'deleted'),
  (gen_random_uuid(), 'pet_ep_disabled',  'endpoint.disabled',    'A webhook endpoint was disabled',         'endpoint',    'disabled'),
  (gen_random_uuid(), 'pet_app_created',  'application.created',  'An application was created',              'application', 'created'),
  (gen_random_uuid(), 'pet_app_updated',  'application.updated',  'An application was updated',              'application', 'updated'),
  (gen_random_uuid(), 'pet_app_deleted',  'application.deleted',  'An application was deleted',              'application', 'deleted')
ON CONFLICT DO NOTHING;
