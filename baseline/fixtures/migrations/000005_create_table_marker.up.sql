CREATE TABLE markers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
    skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
    unit_id UUID,
    name VARCHAR(255),
    kategori VARCHAR(255),
    jumlah int,
    rot_x DOUBLE PRECISION,
    rot_y DOUBLE PRECISION,
    rot_z DOUBLE PRECISION,
    pos_x DOUBLE PRECISION,
    pos_y DOUBLE PRECISION,
    keterangan TEXT,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);