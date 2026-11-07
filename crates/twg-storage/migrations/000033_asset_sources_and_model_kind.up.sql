-- Model assets now declare `static` or `animated`, and every asset keeps its
-- source file(s) in `asset_sources` so an animated model can carry both an
-- `idle` and an `animated` GLB. Conversions are keyed by (asset, slot, kind).
--
-- Files move to `<ASSETS_DIR>/<asset-id>/<slot>/...`, so an animated model has
-- `idle/original.glb` and `animated/original.glb`.

ALTER TABLE assets ADD COLUMN model_kind VARCHAR(16) NULL;

UPDATE assets SET model_kind = 'static' WHERE category = 'model';

ALTER TABLE assets ADD CONSTRAINT assets_model_kind_check CHECK (
    (category = 'model' AND model_kind IN ('static', 'animated'))
    OR (category IN ('texture', 'icon') AND model_kind IS NULL)
);

CREATE TABLE asset_sources (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    asset_id UUID NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    slot VARCHAR(16) NOT NULL,
    filename VARCHAR(255) NOT NULL,
    mime VARCHAR(128) NOT NULL,
    size_bytes BIGINT NOT NULL,
    sha256 VARCHAR(64) NOT NULL,
    has_draco BOOLEAN NOT NULL DEFAULT FALSE,
    lod SMALLINT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT asset_sources_slot_check CHECK (slot IN ('static', 'idle', 'animated')),
    CONSTRAINT asset_sources_asset_slot_uniq UNIQUE (asset_id, slot)
);

INSERT INTO asset_sources (
    asset_id, slot, filename, mime, size_bytes, sha256, has_draco, lod
)
SELECT id, 'static', original_filename, mime, size_bytes, sha256, has_draco, lod
FROM assets;

ALTER TABLE asset_variants ADD COLUMN slot VARCHAR(16) NOT NULL DEFAULT 'static';

ALTER TABLE asset_variants
    ADD CONSTRAINT asset_variants_slot_check CHECK (slot IN ('static', 'idle', 'animated'));

DROP INDEX IF EXISTS asset_variants_asset_kind_uniq;
CREATE UNIQUE INDEX asset_variants_asset_slot_kind_uniq
ON asset_variants (asset_id, slot, kind);

ALTER TABLE assets
    DROP COLUMN original_filename,
    DROP COLUMN mime,
    DROP COLUMN size_bytes,
    DROP COLUMN sha256,
    DROP COLUMN has_draco,
    DROP COLUMN lod;
