-- Bootstrap deterministik untuk database fixture lokal.
-- Guard: hanya boleh dijalankan pada database bernama twg_baseline.

DO $$
BEGIN
    IF current_database() <> 'twg_baseline' THEN
        RAISE EXCEPTION 'Refusing to reset database %, expected twg_baseline', current_database();
    END IF;
END $$;

DROP EXTENSION IF EXISTS "uuid-ossp" CASCADE;
DROP SCHEMA IF EXISTS public CASCADE;
CREATE SCHEMA public;
GRANT ALL ON SCHEMA public TO public;
CREATE EXTENSION "uuid-ossp";
