CREATE TABLE particles (
        id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
        operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
        skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
        name VARCHAR(255),
        pos_x DOUBLE PRECISION,
        pos_y DOUBLE PRECISION,
        data TEXT,
        is_spawn int DEFAULT 0,
        created_at TIMESTAMP DEFAULT NOW(),
        updated_at TIMESTAMP DEFAULT NOW()
);