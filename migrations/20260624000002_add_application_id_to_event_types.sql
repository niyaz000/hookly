ALTER TABLE event_types
    ADD COLUMN IF NOT EXISTS application_id UUID REFERENCES applications(id);

CREATE INDEX IF NOT EXISTS idx_event_types_application_id
    ON event_types (application_id);
