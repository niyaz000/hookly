ALTER TABLE applications
    ADD COLUMN IF NOT EXISTS environment_id UUID REFERENCES environments(id);

CREATE INDEX IF NOT EXISTS idx_applications_environment_id
    ON applications (environment_id);
