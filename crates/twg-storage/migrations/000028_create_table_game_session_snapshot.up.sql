CREATE TABLE game_session_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
    skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
    snapshot_enabled BOOLEAN DEFAULT false,
    map_state JSONB,
    markers_state JSONB,
    anims_state JSONB,
    updated_by UUID REFERENCES users(id),
    updated_at TIMESTAMP DEFAULT NOW(),
    version INTEGER DEFAULT 1
);

CREATE UNIQUE INDEX idx_game_session_snapshots_skenario_id ON game_session_snapshots(skenario_id);
