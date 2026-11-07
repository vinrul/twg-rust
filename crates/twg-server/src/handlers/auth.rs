use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Json, State};
use axum::http::{HeaderMap, StatusCode};
use twg_auth::AuthContext;
use twg_contracts::auth::{LoginRequest, LoginResponse, LogoutRequest, RefreshTokenRequest};
use twg_contracts::users::ChangePasswordRequest;
use twg_contracts::{MessageResponse, SingleResponse};

use crate::error::ApiError;
use crate::middleware::{ClientIp, header_device_id};
use crate::state::AppState;

pub async fn login(
    State(state): State<AppState>,
    Extension(client): Extension<ClientIp>,
    headers: HeaderMap,
    body: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<SingleResponse<LoginResponse>>), ApiError> {
    state
        .rate_limiter
        .check(&client_key("login", &client.0))
        .map_err(ApiError::too_many_requests)?;
    let Json(body) = body?;
    let device_id = device_id(&headers, &body.device_id);
    let result = state
        .auth
        .login(&body.username, &body.password, &device_id)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(SingleResponse::success(result.response)),
    ))
}

/// Login used by `twg-online-admin`. Admins are single-device on the normal
/// route; this one creates an independent multi-device session and is limited
/// to the `admin` role. Rate limiting is shared with `/auth/login`.
pub async fn login_admin(
    State(state): State<AppState>,
    Extension(client): Extension<ClientIp>,
    headers: HeaderMap,
    body: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<SingleResponse<LoginResponse>>), ApiError> {
    state
        .rate_limiter
        .check(&client_key("login", &client.0))
        .map_err(ApiError::too_many_requests)?;
    let Json(body) = body?;
    let device_id = device_id(&headers, &body.device_id);
    let result = state
        .auth
        .login_admin_tool(&body.username, &body.password, &device_id)
        .await?;
    tracing::info!(
        username = %body.username,
        device_id = %device_id,
        "admin tool login"
    );
    Ok((
        StatusCode::CREATED,
        Json(SingleResponse::success(result.response)),
    ))
}

pub async fn refresh(
    State(state): State<AppState>,
    Extension(client): Extension<ClientIp>,
    headers: HeaderMap,
    body: Result<Json<RefreshTokenRequest>, JsonRejection>,
) -> Result<Json<SingleResponse<LoginResponse>>, ApiError> {
    state
        .rate_limiter
        .check(&client_key("refresh", &client.0))
        .map_err(ApiError::too_many_requests)?;
    let Json(body) = body?;
    let device_id = device_id(&headers, &body.device_id);
    let result = state.auth.refresh(&body.refresh_token, &device_id).await?;
    Ok(Json(SingleResponse::success(result.response)))
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    headers: HeaderMap,
    body: Result<Json<LogoutRequest>, JsonRejection>,
) -> Result<Json<MessageResponse>, ApiError> {
    let body_device = body.map(|Json(body)| body.device_id).unwrap_or_default();
    let device_id = device_id(&headers, &body_device);
    state.auth.logout(&context.subject, &device_id).await?;
    Ok(Json(MessageResponse {
        code: 200,
        message: "success".to_owned(),
    }))
}

pub async fn identify(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    headers: HeaderMap,
) -> Result<Json<SingleResponse<LoginResponse>>, ApiError> {
    let device_id = header_device_id(&headers)
        .ok_or_else(|| ApiError::unauthorized("device_id is required"))?;
    let result = state.auth.identify(&context.subject, device_id).await?;
    Ok(Json(SingleResponse::success(result.response)))
}

pub async fn change_password(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    body: Result<Json<ChangePasswordRequest>, JsonRejection>,
) -> Result<Json<MessageResponse>, ApiError> {
    let Json(body) = body?;
    state
        .auth
        .change_password(&context.subject, &body.old_password, &body.new_password)
        .await?;
    Ok(Json(MessageResponse {
        code: 200,
        message: "success".to_owned(),
    }))
}

fn device_id(headers: &HeaderMap, body: &str) -> String {
    header_device_id(headers).unwrap_or(body.trim()).to_owned()
}

fn client_key(scope: &str, ip: &str) -> String {
    format!("{scope}:{ip}")
}
