use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Extension, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use twg_auth::{AuthContext, AuthError};
use twg_domain::Role;
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Clone)]
pub struct RoleGuard(pub Vec<Role>);

pub async fn request_id(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let span = tracing::info_span!(
        "http",
        request_id = %request_id,
        method = %request.method(),
        path = %request.uri().path()
    );
    request
        .extensions_mut()
        .insert(RequestId(request_id.clone()));
    let mut response = next.run(request).instrument(span).await;
    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-request-id"), value);
    }
    response
}

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

#[derive(Debug, Clone)]
pub struct ClientIp(pub String);

pub async fn client_ip(mut request: Request, next: Next) -> Response {
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(address)| address.ip().to_string())
        .unwrap_or_else(|| "unknown".to_owned());
    request.extensions_mut().insert(ClientIp(ip));
    next.run(request).await
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = bearer_token(request.headers())?.to_owned();
    let subject = state.auth.authenticate_access(&token)?;
    request.extensions_mut().insert(AuthContext::new(subject));
    Ok(next.run(request).await)
}

pub async fn require_session(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = bearer_token(request.headers())?.to_owned();
    let device_id = header_device_id(request.headers())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ApiError::unauthorized("device_id is required"))?;
    let subject = state.auth.authenticate_access(&token)?;
    state.auth.validate_session(&subject, &device_id).await?;
    request.extensions_mut().insert(AuthContext::new(subject));
    Ok(next.run(request).await)
}

pub async fn require_roles(
    State(guard): State<RoleGuard>,
    Extension(context): Extension<AuthContext>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    context.require_roles(&guard.0)?;
    Ok(next.run(request).await)
}

pub fn bearer_token(headers: &HeaderMap) -> Result<&str, AuthError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or(AuthError::MissingToken)?;
    let header = header
        .to_str()
        .map_err(|_| AuthError::InvalidAuthorizationHeader)?;
    let mut fields = header.split_whitespace();
    let scheme = fields.next().ok_or(AuthError::InvalidAuthorizationHeader)?;
    let token = fields.next().ok_or(AuthError::InvalidAuthorizationHeader)?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return Err(AuthError::UnsupportedAuthorizationType(
            scheme.to_ascii_lowercase(),
        ));
    }
    if fields.next().is_some() {
        return Err(AuthError::InvalidAuthorizationHeader);
    }
    Ok(token)
}

pub fn header_device_id(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-device-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};
    use twg_auth::AuthError;

    use super::{bearer_token, header_device_id};

    #[test]
    fn parses_bearer_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer v2.local.token"),
        );
        assert_eq!(bearer_token(&headers), Ok("v2.local.token"));
    }

    #[test]
    fn rejects_missing_or_malformed_bearer_headers() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), Err(AuthError::MissingToken));
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Basic abc"),
        );
        assert_eq!(
            bearer_token(&headers),
            Err(AuthError::UnsupportedAuthorizationType("basic".to_owned()))
        );
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer"),
        );
        assert_eq!(
            bearer_token(&headers),
            Err(AuthError::InvalidAuthorizationHeader)
        );
    }

    #[test]
    fn header_device_id_trims_empty_values() {
        let mut headers = HeaderMap::new();
        assert_eq!(header_device_id(&headers), None);
        headers.insert("x-device-id", HeaderValue::from_static("  "));
        assert_eq!(header_device_id(&headers), None);
        headers.insert("x-device-id", HeaderValue::from_static(" device-1 "));
        assert_eq!(header_device_id(&headers), Some("device-1"));
    }
}
