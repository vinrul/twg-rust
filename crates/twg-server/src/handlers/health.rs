use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use twg_contracts::HealthResponse;

use crate::state::AppState;

pub async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse::ok())
}

pub async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let Some(pool) = &state.pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "unavailable".to_owned(),
            }),
        );
    };
    match twg_storage::ping(pool).await {
        Ok(()) => (StatusCode::OK, Json(HealthResponse::ok())),
        Err(error) => {
            tracing::warn!(%error, "readiness database ping failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "unavailable".to_owned(),
                }),
            )
        }
    }
}
