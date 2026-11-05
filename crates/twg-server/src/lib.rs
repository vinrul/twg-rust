use axum::{Json, Router, routing::get};
use tower_http::trace::TraceLayer;
use twg_contracts::HealthResponse;

pub mod tiles;

pub fn router() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .merge(tiles::router())
        .layer(TraceLayer::new_for_http())
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse::ok())
}

async fn readyz() -> Json<HealthResponse> {
    Json(HealthResponse::ok())
}

#[cfg(test)]
mod tests {
    use super::router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_returns_ok() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["status"], "ok");
    }

    #[tokio::test]
    async fn unknown_route_returns_not_found() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
