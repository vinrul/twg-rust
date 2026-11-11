-- Assets uploaded from the scenario editor ("import") belong to a single
-- operation and have no admin asset group. `created_by` lets the uploader (or
-- an admin) delete their own import; admin assets keep a NULL owner because
-- they are managed from the asset library.

ALTER TABLE assets
    ADD COLUMN IF NOT EXISTS origin TEXT NOT NULL DEFAULT 'admin';

ALTER TABLE assets
    ADD COLUMN IF NOT EXISTS operasi_id UUID REFERENCES operasis (id) ON DELETE CASCADE;

ALTER TABLE assets
    ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users (id) ON DELETE SET NULL;

ALTER TABLE assets
    ADD CONSTRAINT assets_origin_check CHECK (origin IN ('admin', 'import'));

CREATE INDEX IF NOT EXISTS idx_assets_operasi_id ON assets (operasi_id);
CREATE INDEX IF NOT EXISTS idx_assets_origin ON assets (origin);
