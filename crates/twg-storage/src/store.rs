use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use twg_auth::{AuthStore, AuthStoreError, AuthUser, RotateOutcome, SessionRecord};
use twg_domain::Role;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgAuthStore {
    pool: PgPool,
}

impl PgAuthStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[derive(Debug, FromRow)]
struct UserRow {
    id: Uuid,
    name: Option<String>,
    username: Option<String>,
    password: Option<String>,
    role: Option<String>,
    satker_id: Option<Uuid>,
    profile_img: Option<String>,
    units: Option<String>,
    created_at: Option<NaiveDateTime>,
    updated_at: Option<NaiveDateTime>,
}

impl UserRow {
    fn into_auth_user(self) -> Result<AuthUser, AuthStoreError> {
        let role = self
            .role
            .as_deref()
            .ok_or_else(|| AuthStoreError::Internal("user role is missing".to_owned()))?
            .parse::<Role>()
            .map_err(|_| AuthStoreError::Internal("invalid user role".to_owned()))?;
        let now = Utc::now();
        Ok(AuthUser {
            id: self.id,
            name: self.name.unwrap_or_default(),
            username: self.username.unwrap_or_default(),
            password_hash: self.password.unwrap_or_default(),
            role,
            satker_id: self.satker_id,
            profile_img: self.profile_img.unwrap_or_default(),
            units: self.units.unwrap_or_else(|| "[]".to_owned()),
            created_at: self
                .created_at
                .map(|value| Utc.from_utc_datetime(&value))
                .unwrap_or(now),
            updated_at: self
                .updated_at
                .map(|value| Utc.from_utc_datetime(&value))
                .unwrap_or(now),
        })
    }
}

#[derive(Debug, FromRow)]
struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    device_id: String,
    refresh_token_hash: String,
    is_single_device: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

impl From<SessionRow> for SessionRecord {
    fn from(row: SessionRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            device_id: row.device_id,
            refresh_token_hash: row.refresh_token_hash,
            is_single_device: row.is_single_device,
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_used_at: row.last_used_at,
            revoked_at: row.revoked_at,
        }
    }
}

#[async_trait]
impl AuthStore for PgAuthStore {
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<AuthUser>, AuthStoreError> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, name, username, password, role, satker_id, profile_img, units,
                    created_at, updated_at
             FROM users
             WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(UserRow::into_auth_user).transpose()
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<AuthUser>, AuthStoreError> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, name, username, password, role, satker_id, profile_img, units,
                    created_at, updated_at
             FROM users
             WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(UserRow::into_auth_user).transpose()
    }

    async fn ensure_single_device_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        refresh_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<SessionRecord, AuthStoreError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;

        let locked = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        if locked.is_none() {
            return Err(AuthStoreError::NotFound);
        }

        let single = sqlx::query_as::<_, SessionRow>(
            "SELECT id, user_id, device_id, refresh_token_hash, is_single_device,
                    created_at, updated_at, last_used_at, revoked_at
             FROM user_sessions
             WHERE user_id = $1 AND revoked_at IS NULL AND is_single_device IS TRUE
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        if let Some(existing) = single {
            if existing.device_id != device_id {
                return Err(AuthStoreError::AlreadyLoggedIn);
            }
            let updated = sqlx::query_as::<_, SessionRow>(
                "UPDATE user_sessions
                 SET refresh_token_hash = $2, updated_at = $3, last_used_at = $3
                 WHERE id = $1
                 RETURNING id, user_id, device_id, refresh_token_hash, is_single_device,
                           created_at, updated_at, last_used_at, revoked_at",
            )
            .bind(existing.id)
            .bind(refresh_token_hash)
            .bind(now)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(updated.into());
        }

        let active_same_device = sqlx::query_as::<_, SessionRow>(
            "SELECT id, user_id, device_id, refresh_token_hash, is_single_device,
                    created_at, updated_at, last_used_at, revoked_at
             FROM user_sessions
             WHERE user_id = $1 AND device_id = $2 AND revoked_at IS NULL
             LIMIT 1",
        )
        .bind(user_id)
        .bind(device_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let session = if let Some(existing) = active_same_device {
            sqlx::query_as::<_, SessionRow>(
                "UPDATE user_sessions
                 SET refresh_token_hash = $2, is_single_device = TRUE,
                     updated_at = $3, last_used_at = $3
                 WHERE id = $1
                 RETURNING id, user_id, device_id, refresh_token_hash, is_single_device,
                           created_at, updated_at, last_used_at, revoked_at",
            )
            .bind(existing.id)
            .bind(refresh_token_hash)
            .bind(now)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?
        } else {
            sqlx::query_as::<_, SessionRow>(
                "INSERT INTO user_sessions
                    (user_id, device_id, refresh_token_hash, is_single_device, last_used_at)
                 VALUES ($1, $2, $3, TRUE, $4)
                 RETURNING id, user_id, device_id, refresh_token_hash, is_single_device,
                           created_at, updated_at, last_used_at, revoked_at",
            )
            .bind(user_id)
            .bind(device_id)
            .bind(refresh_token_hash)
            .bind(now)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?
        };
        tx.commit().await.map_err(map_sqlx)?;
        Ok(session.into())
    }

    async fn upsert_multi_device_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        refresh_token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<SessionRecord, AuthStoreError> {
        let user_exists = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx)?;
        if user_exists.is_none() {
            return Err(AuthStoreError::NotFound);
        }
        let row = sqlx::query_as::<_, SessionRow>(
            "INSERT INTO user_sessions
                (user_id, device_id, refresh_token_hash, is_single_device, last_used_at)
             VALUES ($1, $2, $3, FALSE, $4)
             ON CONFLICT (user_id, device_id) WHERE revoked_at IS NULL
             DO UPDATE SET refresh_token_hash = EXCLUDED.refresh_token_hash,
                           is_single_device = FALSE,
                           updated_at = $4,
                           last_used_at = $4
             RETURNING id, user_id, device_id, refresh_token_hash, is_single_device,
                       created_at, updated_at, last_used_at, revoked_at",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(refresh_token_hash)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.into())
    }

    async fn find_session_by_device(
        &self,
        user_id: Uuid,
        device_id: &str,
    ) -> Result<Option<SessionRecord>, AuthStoreError> {
        let row = sqlx::query_as::<_, SessionRow>(
            "SELECT id, user_id, device_id, refresh_token_hash, is_single_device,
                    created_at, updated_at, last_used_at, revoked_at
             FROM user_sessions
             WHERE user_id = $1 AND device_id = $2 AND revoked_at IS NULL
             LIMIT 1",
        )
        .bind(user_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(Into::into))
    }

    async fn rotate_refresh_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        expected_hash: &str,
        new_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<RotateOutcome, AuthStoreError> {
        let row = sqlx::query_as::<_, SessionRow>(
            "UPDATE user_sessions
             SET refresh_token_hash = $4, updated_at = $5, last_used_at = $5
             WHERE user_id = $1 AND device_id = $2 AND revoked_at IS NULL
               AND refresh_token_hash = $3
             RETURNING id, user_id, device_id, refresh_token_hash, is_single_device,
                       created_at, updated_at, last_used_at, revoked_at",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(expected_hash)
        .bind(new_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        if let Some(row) = row {
            return Ok(RotateOutcome::Rotated(row.into()));
        }
        let active = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM user_sessions
             WHERE user_id = $1 AND device_id = $2 AND revoked_at IS NULL
             LIMIT 1",
        )
        .bind(user_id)
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(if active.is_some() {
            RotateOutcome::HashMismatch
        } else {
            RotateOutcome::Revoked
        })
    }

    async fn touch_session(
        &self,
        session_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError> {
        sqlx::query(
            "UPDATE user_sessions SET last_used_at = $2, updated_at = $2
             WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(session_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn revoke_session(
        &self,
        user_id: Uuid,
        device_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError> {
        sqlx::query(
            "UPDATE user_sessions SET revoked_at = $3, updated_at = $3
             WHERE user_id = $1 AND device_id = $2 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn update_password(
        &self,
        user_id: Uuid,
        password_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AuthStoreError> {
        let result = sqlx::query("UPDATE users SET password = $2, updated_at = $3 WHERE id = $1")
            .bind(user_id)
            .bind(password_hash)
            .bind(now.naive_utc())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(AuthStoreError::NotFound);
        }
        Ok(())
    }
}

fn map_sqlx(error: sqlx::Error) -> AuthStoreError {
    if let sqlx::Error::Database(database) = &error
        && database.code().as_deref() == Some("23505")
    {
        return AuthStoreError::AlreadyLoggedIn;
    }
    AuthStoreError::Internal(error.to_string())
}
