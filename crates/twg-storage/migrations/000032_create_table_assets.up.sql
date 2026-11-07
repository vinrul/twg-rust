-- Asset library: uploaded models (glTF/GLB), textures, and icons plus their
-- converted variants (Draco, LOD2, resized images).
--
-- The original upload is always kept; variants live in a child table so the
-- desktop client can download only what it needs and re-sync per asset version.

CREATE TABLE assets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    category VARCHAR(16) NOT NULL,
    original_filename VARCHAR(255) NOT NULL,
    mime VARCHAR(128) NOT NULL,
    size_bytes BIGINT NOT NULL,
    sha256 VARCHAR(64) NOT NULL,
    has_draco BOOLEAN NOT NULL DEFAULT FALSE,
    lod SMALLINT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT assets_category_check CHECK (category IN ('model', 'texture', 'icon'))
);

CREATE INDEX idx_assets_category ON assets (category);
CREATE INDEX idx_assets_updated_at ON assets (updated_at DESC);

CREATE TABLE asset_variants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    asset_id UUID NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    kind VARCHAR(32) NOT NULL,
    filename VARCHAR(255) NOT NULL,
    mime VARCHAR(128) NOT NULL,
    size_bytes BIGINT NOT NULL,
    sha256 VARCHAR(64) NOT NULL,
    width INTEGER NULL,
    height INTEGER NULL,
    reused BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT asset_variants_kind_check CHECK (
        kind IN ('draco', 'lod2', 'draco_lod2', 'resize_512', 'resize_256', 'resize_128')
    )
);

CREATE UNIQUE INDEX asset_variants_asset_kind_uniq ON asset_variants (asset_id, kind);
