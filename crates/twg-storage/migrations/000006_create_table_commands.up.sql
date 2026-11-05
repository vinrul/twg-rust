CREATE TABLE commands (
        id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
        operasi_id UUID REFERENCES operasis(id) ON DELETE CASCADE,
        skenario_id UUID REFERENCES skenarios(id) ON DELETE CASCADE,
        marker_id UUID,
        command VARCHAR(255),
        data TEXT,
        created_at TIMESTAMP DEFAULT NOW(),
        updated_at TIMESTAMP DEFAULT NOW()
);