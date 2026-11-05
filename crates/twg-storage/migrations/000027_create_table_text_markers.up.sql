CREATE TABLE text_markers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
    skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
    text TEXT NOT NULL,
    color VARCHAR(32) NOT NULL DEFAULT '#ffffff',
    font_size DOUBLE PRECISION NOT NULL DEFAULT 16,
    align VARCHAR(16) NOT NULL DEFAULT 'center',
    wrap_text BOOLEAN NOT NULL DEFAULT false,
    pos_x DOUBLE PRECISION NOT NULL,
    pos_y DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_text_markers_skenario_id_created_at
    ON text_markers (skenario_id, created_at DESC);
