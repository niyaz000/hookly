DO $$ BEGIN
    CREATE TYPE application_state AS ENUM ('ACTIVE', 'SUSPENDED', 'INACTIVE');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

ALTER TABLE applications
    ADD COLUMN IF NOT EXISTS state application_state NOT NULL DEFAULT 'ACTIVE';
