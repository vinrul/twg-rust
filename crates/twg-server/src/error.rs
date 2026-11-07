use std::time::Duration;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use twg_auth::{AuthError, AuthErrorKind};
use twg_contracts::ErrorResponse;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    retry_after: Option<u64>,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, message)
    }

    pub fn too_many_requests(retry_after: Duration) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: "too many requests".to_owned(),
            retry_after: Some(retry_after.as_secs().max(1)),
        }
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "internal server error");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
    }

    /// Conversion failures are surfaced to the admin UI because they usually
    /// mean a missing tool or an unsupported source file.
    pub fn conversion_failed(message: impl Into<String>) -> Self {
        let message = message.into();
        tracing::error!(error = %message, "asset conversion failed");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        let status = match error.kind() {
            AuthErrorKind::Invalid => StatusCode::BAD_REQUEST,
            AuthErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            AuthErrorKind::Forbidden => StatusCode::FORBIDDEN,
            AuthErrorKind::NotFound => StatusCode::NOT_FOUND,
            AuthErrorKind::Conflict => StatusCode::CONFLICT,
            AuthErrorKind::Internal => {
                tracing::error!(error = ?error, "auth internal error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            "internal server error".to_owned()
        } else {
            error.to_string()
        };
        Self::new(status, message)
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        match rejection.status() {
            StatusCode::PAYLOAD_TOO_LARGE => {
                Self::new(StatusCode::PAYLOAD_TOO_LARGE, "request body too large")
            }
            StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                Self::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported media type")
            }
            _ => Self::bad_request("invalid request body"),
        }
    }
}

impl From<twg_storage::WriteError> for ApiError {
    fn from(error: twg_storage::WriteError) -> Self {
        match error {
            twg_storage::WriteError::NotFound(message) => Self::not_found(message),
            twg_storage::WriteError::Conflict(message) => Self::conflict(message),
            twg_storage::WriteError::Invalid(message) => Self::bad_request(message),
            twg_storage::WriteError::Database(error) => Self::internal(error),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            code: self.status.as_u16(),
            message: self.message,
        };
        let mut response = (self.status, Json(body)).into_response();
        if let Some(seconds) = self.retry_after {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_str(&seconds.to_string())
                    .unwrap_or_else(|_| HeaderValue::from_static("1")),
            );
        }
        response.headers_mut().insert(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        );
        response
    }
}
