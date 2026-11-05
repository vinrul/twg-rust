CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    refresh_token_hash TEXT NOT NULL,
    is_single_device BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NULL,
    revoked_at TIMESTAMPTZ NULL
);

CREATE UNIQUE INDEX user_sessions_user_device_active_uniq
ON user_sessions (user_id, device_id)
WHERE revoked_at IS NULL;

CREATE UNIQUE INDEX user_sessions_user_single_device_active_uniq
ON user_sessions (user_id)
WHERE revoked_at IS NULL AND is_single_device IS TRUE;

CREATE INDEX user_sessions_user_active_idx
ON user_sessions (user_id)
WHERE revoked_at IS NULL;
