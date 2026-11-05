CREATE TABLE last_maps (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
    skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
    center_x DOUBLE PRECISION,
    center_y DOUBLE PRECISION,
    zoom DOUBLE PRECISION,
    bearing DOUBLE PRECISION,
    pitch DOUBLE PRECISION,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);