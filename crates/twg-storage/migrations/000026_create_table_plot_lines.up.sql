CREATE TABLE plot_lines (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
    skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    color VARCHAR(20) NOT NULL DEFAULT '#ff0000',
    width DOUBLE PRECISION NOT NULL DEFAULT 4 CHECK (width > 0),
    alpha DOUBLE PRECISION NOT NULL DEFAULT 1 CHECK (alpha >= 0 AND alpha <= 1),
    geom TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_plot_lines_skenario_id_created_at
    ON plot_lines (skenario_id, created_at DESC);