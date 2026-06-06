-- Soft-delete duplicate (user_id, name) rows, keeping the most recently created one
UPDATE api_keys
SET deleted_at = NOW()
WHERE deleted_at IS NULL
  AND id NOT IN (
    SELECT DISTINCT ON (user_id, name) id
    FROM api_keys
    WHERE deleted_at IS NULL
    ORDER BY user_id, name, created_at DESC
  );

CREATE UNIQUE INDEX IF NOT EXISTS api_keys_user_name_uq ON api_keys (user_id, name) WHERE deleted_at IS NULL;
