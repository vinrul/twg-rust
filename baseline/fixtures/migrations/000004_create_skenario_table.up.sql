CREATE TABLE skenarios (
        id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
        operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
        name VARCHAR(255),
        center_x DOUBLE PRECISION,
        center_y DOUBLE PRECISION,
        zoom DOUBLE PRECISION,
        max_zoom DOUBLE PRECISION,
        pitch DOUBLE PRECISION,
        created_at TIMESTAMP DEFAULT NOW(),
        updated_at TIMESTAMP DEFAULT NOW()
);