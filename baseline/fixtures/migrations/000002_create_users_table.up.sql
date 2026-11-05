CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255),
    username VARCHAR(255) UNIQUE,
    password VARCHAR(255),
    role VARCHAR(20),
    unit_id UUID,
    profile_img VARCHAR(255) NULL,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);