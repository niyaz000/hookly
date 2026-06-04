ALTER TABLE api_keys DROP COLUMN environment;
ALTER TABLE api_keys ADD COLUMN environment_id VARCHAR(20) NOT NULL DEFAULT '';
ALTER TABLE api_keys ALTER COLUMN environment_id DROP DEFAULT;
ALTER TABLE api_keys ADD COLUMN tags JSONB NOT NULL DEFAULT '{}';
DROP TYPE api_key_environment;
