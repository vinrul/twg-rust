CREATE TABLE anims (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
    skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
    name VARCHAR(255),
    username VARCHAR(255),
    keterangan TEXT,
    tipe VARCHAR(255),
    data TEXT,
    geom TEXT,
    speed int4,
    detik int4,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);