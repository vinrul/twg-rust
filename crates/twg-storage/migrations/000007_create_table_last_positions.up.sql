CREATE TABLE last_positions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
    skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
    marker_id UUID REFERENCES markers(id) ON DELETE CASCADE,
    rot_x DOUBLE PRECISION,
    rot_y DOUBLE PRECISION,
    rot_z DOUBLE PRECISION,
    pos_x DOUBLE PRECISION,
    pos_y DOUBLE PRECISION,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);