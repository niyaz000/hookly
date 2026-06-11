-- Shard id each schedule belongs to; assigned once at create time, never recomputed.
-- Default 0 so existing rows land on shard 0 until the reconciliation task re-assigns them.
ALTER TABLE schedules
    ADD COLUMN assigned_shard SMALLINT NOT NULL DEFAULT 0;

CREATE INDEX idx_schedules_assigned_shard ON schedules (assigned_shard) WHERE deleted_at IS NULL;

-- Registry of all shards: their state and which Redis node holds their sorted set.
CREATE TABLE scheduler_shards (
    id          SMALLINT     NOT NULL,
    state       VARCHAR(20)  NOT NULL DEFAULT 'active',
    redis_url   VARCHAR(255) NOT NULL DEFAULT '',
    note        TEXT,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    CONSTRAINT scheduler_shards_pk          PRIMARY KEY (id),
    CONSTRAINT scheduler_shards_state_valid CHECK (state IN ('active', 'draining', 'paused', 'drained'))
);

-- Seed a default shard so the API server has at least one active shard to route to.
INSERT INTO scheduler_shards (id, state, note) VALUES (0, 'active', 'default shard');

-- Tenants that are pinned to a dedicated shard (enterprise SLA isolation).
CREATE TABLE tenant_shard_affinity (
    tenant_id   UUID        NOT NULL,
    shard_id    SMALLINT    NOT NULL REFERENCES scheduler_shards(id),
    note        TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT tenant_shard_affinity_pk PRIMARY KEY (tenant_id)
);

CREATE INDEX idx_tenant_shard_affinity_shard_id ON tenant_shard_affinity (shard_id);
