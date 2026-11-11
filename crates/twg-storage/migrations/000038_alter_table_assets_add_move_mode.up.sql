ALTER TABLE assets
    ADD COLUMN IF NOT EXISTS move_mode TEXT NOT NULL DEFAULT 'none';

UPDATE assets
SET move_mode = 'free'
WHERE model_kind = 'animated' AND move_mode <> 'free';
