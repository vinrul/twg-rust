CREATE TABLE IF NOT EXISTS asset_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(120) NOT NULL UNIQUE,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

ALTER TABLE assets
    ADD COLUMN IF NOT EXISTS group_id UUID REFERENCES asset_groups(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_assets_group_id ON assets (group_id);

INSERT INTO asset_groups (name) VALUES
    ('Personel'),
    ('Bangunan'),
    ('Kendaraan Bermotor'),
    ('Senjata'),
    ('Perlengkapan'),
    ('Lainnya')
ON CONFLICT (name) DO NOTHING;
