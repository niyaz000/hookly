CREATE UNIQUE INDEX api_keys_user_name_uq ON api_keys (user_id, name) WHERE deleted_at IS NULL;
