-- Add optional description to environments table.
ALTER TABLE environments ADD COLUMN description VARCHAR(521);

-- Widen key_prefix from 3 to 8 chars to accommodate env-tag + first-3-of-random
-- (e.g. "pro_mbd") for a more useful key hint.
ALTER TABLE api_keys ALTER COLUMN key_prefix TYPE VARCHAR(8);
