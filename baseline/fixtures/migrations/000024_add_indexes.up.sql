-- +migrate NoTransaction
-- Indexes for common access patterns.
-- Uses CONCURRENTLY to reduce locking (recommended for production).

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_skenarios_operasi_id_created_at ON skenarios (operasi_id, created_at);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_markers_skenario_id_created_at ON markers (skenario_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_buildings_skenario_id_created_at ON buildings (skenario_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_particles_skenario_id_created_at ON particles (skenario_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_alurs_skenario_id_created_at ON alurs (skenario_id, created_at);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_anims_skenario_id_urut ON anims (skenario_id, urut);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_anims_skenario_id_username_urut ON anims (skenario_id, username, urut);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_commands_skenario_id_id ON commands (skenario_id, id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_last_positions_skenario_id_created_at ON last_positions (skenario_id, created_at);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_last_positions_marker_id_created_at ON last_positions (marker_id, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_last_maps_skenario_id_created_at ON last_maps (skenario_id, created_at DESC);
