ALTER TABLE users
    ADD COLUMN satker_id UUID REFERENCES satkers(id) ON DELETE SET NULL;

ALTER TABLE operasis
    ADD COLUMN satker_id UUID REFERENCES satkers(id) ON DELETE SET NULL;

ALTER TABLE skenarios
    ADD COLUMN satker_id UUID REFERENCES satkers(id) ON DELETE SET NULL;

CREATE INDEX idx_users_satker_id ON users(satker_id);
CREATE INDEX idx_users_role_satker_id ON users(role, satker_id);
CREATE INDEX idx_operasis_satker_id ON operasis(satker_id);
CREATE INDEX idx_skenarios_satker_id ON skenarios(satker_id);
CREATE INDEX idx_skenarios_satker_operasi ON skenarios(satker_id, operasi_id);
