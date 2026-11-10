pub mod assets;
pub mod config;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod rate_limit;
pub mod realtime;
pub mod state;
pub mod tiles;
pub mod uploads;

use std::time::Duration;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, Method, header};
use axum::routing::{delete, get, patch, post, put};
use tower_http::cors::{AllowMethods, AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let cors = cors_layer(&state.cors_domains);
    let max_body_bytes = state.max_body_bytes;
    let max_body_header = HeaderValue::from_str(&max_body_bytes.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("104857600"));

    let public = Router::new()
        .route("/healthz", get(handlers::health::healthz))
        .route("/readyz", get(handlers::health::readyz))
        .route("/ws", get(handlers::realtime::ws_upgrade))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/login/admin", post(handlers::auth::login_admin))
        .route("/auth/refresh", post(handlers::auth::refresh))
        .route(
            "/public/skenarios/{operasi_id}",
            get(handlers::public::scenarios),
        )
        .route(
            "/public/skenarios/get/{id}",
            get(handlers::public::scenario_detail),
        )
        .route(
            "/public/markers/list/{skenario_id}",
            get(handlers::public::markers),
        )
        .route(
            "/public/text-markers/list/{skenario_id}",
            get(handlers::public::text_markers),
        )
        .route(
            "/public/particles/list/{skenario_id}",
            get(handlers::public::particles),
        )
        .route(
            "/public/buildings/list/{skenario_id}",
            get(handlers::public::buildings),
        )
        .route(
            "/public/lines/list/{skenario_id}",
            get(handlers::public::lines),
        )
        .route(
            "/public/alurs/list/{skenario_id}",
            get(handlers::public::alurs),
        )
        .route(
            "/public/anims/list/{skenario_id}",
            get(handlers::public::anims),
        )
        .route(
            "/public/last_positions/{skenario_id}",
            get(handlers::public::last_positions),
        )
        .route(
            "/public/list/operasis",
            get(handlers::public::home_operations),
        )
        .route(
            "/public/get/home/operasis/{id}",
            get(handlers::public::home_operation_detail),
        )
        .route("/public/users/online", get(handlers::public::online_users))
        .route("/public/geocode", get(handlers::public::geocode))
        .route(
            "/public/game-sessions/snapshot/{skenario_id}",
            get(handlers::public::snapshot),
        );

    let logout = Router::new()
        .route("/auth/logout", post(handlers::auth::logout))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    let private = Router::new()
        .route("/private/identify", get(handlers::auth::identify))
        .route("/private/ws-ticket", post(handlers::realtime::ws_ticket))
        .route(
            "/private/profile/password",
            put(handlers::auth::change_password),
        )
        .route("/private/profile", put(handlers::manage::update_profile))
        .route("/private/operasis", get(handlers::private::operations))
        .route(
            "/private/operasis/{id}",
            get(handlers::private::operation_detail),
        )
        .route(
            "/private/home/operasis",
            get(handlers::private::home_operations),
        )
        .route("/private/users", get(handlers::private::users))
        .route("/private/users/online", get(handlers::public::online_users))
        .route("/private/satkers", get(handlers::private::satkers))
        .route("/private/assets", get(handlers::assets::list))
        .route("/private/assets/manifest", get(handlers::assets::manifest))
        .route("/private/assets/{id}", get(handlers::assets::detail))
        .route(
            "/private/categories/all",
            get(handlers::private::categories),
        )
        .route(
            "/private/game-sessions/snapshot/{skenario_id}",
            get(handlers::public::snapshot),
        )
        .route(
            "/private/upload-image",
            post(handlers::uploads::upload_image),
        )
        .route(
            "/private/upload-video",
            post(handlers::uploads::upload_video),
        )
        .route(
            "/private/socket",
            post(handlers::sessions::publish_commands),
        )
        .route("/private/markers", post(handlers::markers::create))
        .route(
            "/private/markers/geom/{id}",
            put(handlers::markers::update_geom),
        )
        .route(
            "/private/markers/name/{id}",
            put(handlers::markers::update_name),
        )
        .route(
            "/private/markers/unit/{id}",
            put(handlers::markers::update_unit),
        )
        .route(
            "/private/markers/scale/{id}",
            put(handlers::markers::update_scale),
        )
        .route(
            "/private/markers/altitude/{id}",
            put(handlers::markers::update_altitude),
        )
        .route(
            "/private/markers/rotasi/{id}",
            put(handlers::markers::update_rotation),
        )
        .route(
            "/private/markers/duplicate/{id}",
            put(handlers::markers::duplicate),
        )
        .route("/private/markers/{id}", delete(handlers::markers::delete))
        .route(
            "/private/text-markers",
            post(handlers::geometry::create_text_marker),
        )
        .route(
            "/private/text-markers/{id}",
            put(handlers::geometry::update_text_marker),
        )
        .route(
            "/private/text-markers/geom/{id}",
            put(handlers::geometry::update_text_marker_geom),
        )
        .route(
            "/private/text-markers/{id}",
            delete(handlers::geometry::delete_text_marker),
        )
        .route(
            "/private/buildings",
            post(handlers::geometry::create_building),
        )
        .route(
            "/private/buildings/{id}",
            put(handlers::geometry::update_building),
        )
        .route(
            "/private/buildings/geom/{id}",
            put(handlers::geometry::update_building_geom),
        )
        .route(
            "/private/buildings/{id}",
            delete(handlers::geometry::delete_building),
        )
        .route("/private/lines", post(handlers::geometry::create_line))
        .route("/private/lines/{id}", put(handlers::geometry::update_line))
        .route(
            "/private/lines/geom/{id}",
            put(handlers::geometry::update_line_geom),
        )
        .route(
            "/private/lines/{id}",
            delete(handlers::geometry::delete_line),
        )
        .route("/private/anims/route", post(handlers::anims::create_route))
        .route(
            "/private/anims/route/v2",
            post(handlers::anims::create_route_v2),
        )
        .route("/private/anims/spawn", post(handlers::anims::create_spawn))
        .route(
            "/private/anims/spawn_pickup",
            post(handlers::anims::create_spawn_pickup),
        )
        .route(
            "/private/anims/remove",
            post(handlers::anims::create_remove),
        )
        .route(
            "/private/anims/combine",
            post(handlers::anims::create_combine),
        )
        .route(
            "/private/anims/combine/v2",
            post(handlers::anims::create_combine_v2),
        )
        .route("/private/anims/shoot", post(handlers::anims::create_shoot))
        .route(
            "/private/anims/shoot/v2",
            post(handlers::anims::create_shoot_v2),
        )
        .route(
            "/private/anims/spawn_particle",
            post(handlers::anims::create_spawn_particle),
        )
        .route(
            "/private/anims/remove_particle",
            post(handlers::anims::create_remove_particle),
        )
        .route(
            "/private/anims/route/data/{id}",
            put(handlers::anims::update_route_data),
        )
        .route(
            "/private/anims/route/data/v2/{id}",
            put(handlers::anims::update_route_data_v2),
        )
        .route(
            "/private/anims/spawn/data/{id}",
            put(handlers::anims::update_spawn_data),
        )
        .route(
            "/private/anims/particle/data/{id}",
            put(handlers::anims::update_particle_data),
        )
        .route(
            "/private/anims/remove_particle/data/{id}",
            put(handlers::anims::update_remove_particle_data),
        )
        .route(
            "/private/anims/remove/data/{id}",
            put(handlers::anims::update_remove_data),
        )
        .route(
            "/private/anims/combine/data/{id}",
            put(handlers::anims::update_combine_data),
        )
        .route(
            "/private/anims/shoot/data/{id}",
            put(handlers::anims::update_shoot_data),
        )
        .route(
            "/private/anims/combine/delete/{id}",
            put(handlers::anims::update_combine_type),
        )
        .route(
            "/private/anims/route/geom/{id}",
            put(handlers::anims::update_route_geom),
        )
        .route(
            "/private/anims/route/geom/v2/{id}",
            put(handlers::anims::update_route_geom_v2),
        )
        .route("/private/anims/{id}", delete(handlers::anims::delete_route))
        .route(
            "/private/anims/spawn/{id}",
            delete(handlers::anims::delete_spawn),
        )
        .route(
            "/private/anims/combine/{id}",
            delete(handlers::anims::delete_combine),
        )
        .route(
            "/private/anims/shoot/{id}",
            delete(handlers::anims::delete_shoot),
        )
        .route(
            "/private/anims/particle/{id}",
            delete(handlers::anims::delete_particle),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_session,
        ));

    let private_snapshot_writes = Router::new()
        .route(
            "/private/game-sessions/snapshot/{skenario_id}",
            put(handlers::sessions::upsert_snapshot).delete(handlers::sessions::delete_snapshot),
        )
        .route(
            "/private/game-sessions/snapshot/{skenario_id}/enabled",
            patch(handlers::sessions::toggle_snapshot),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            middleware::RoleGuard(vec![twg_domain::Role::Admin, twg_domain::Role::AdminSatker]),
            middleware::require_roles,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_session,
        ));

    let admin = Router::new()
        .route("/admin/categories", get(handlers::admin::categories))
        .route(
            "/admin/categories/{id}",
            get(handlers::admin::category_detail),
        )
        .route("/admin/categories", post(handlers::manage::create_category))
        .route(
            "/admin/categories/{id}",
            put(handlers::manage::update_category),
        )
        .route(
            "/admin/categories/{id}",
            delete(handlers::manage::delete_category),
        )
        .route("/admin/operasis", post(handlers::manage::create_operation))
        .route(
            "/admin/operasis/{id}",
            put(handlers::manage::update_operation),
        )
        .route(
            "/admin/operasis/{id}",
            delete(handlers::manage::delete_operation),
        )
        .route("/admin/skenarios", post(handlers::manage::create_scenario))
        .route(
            "/admin/skenarios/copy",
            post(handlers::manage::copy_scenario),
        )
        .route(
            "/admin/skenarios/name/{id}",
            put(handlers::manage::update_scenario_name),
        )
        .route(
            "/admin/skenarios/{id}",
            put(handlers::manage::update_scenario),
        )
        .route(
            "/admin/skenarios/{id}",
            delete(handlers::manage::delete_scenario),
        )
        .route("/admin/anims/urut", post(handlers::anims::update_order))
        .route(
            "/admin/anims/urut/v2",
            post(handlers::anims::update_order_v2),
        )
        .route("/admin/alurs", post(handlers::geometry::create_alur))
        .route("/admin/alurs/{id}", put(handlers::geometry::update_alur))
        .route("/admin/alurs/{id}", delete(handlers::geometry::delete_alur))
        .route("/admin/users", post(handlers::manage::create_user))
        .route("/admin/users/{id}", put(handlers::manage::update_user))
        .route("/admin/users/{id}", delete(handlers::manage::delete_user))
        .route("/admin/satkers", post(handlers::manage::create_satker))
        .route("/admin/satkers/{id}", put(handlers::manage::update_satker))
        .route(
            "/admin/satkers/{id}",
            delete(handlers::manage::delete_satker),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            middleware::RoleGuard(vec![
                twg_domain::Role::Admin,
                twg_domain::Role::AdminMonitor,
                twg_domain::Role::AdminSatker,
            ]),
            middleware::require_roles,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_session,
        ));

    // Writes restricted to the `admin` role (assets and session revocation).
    let admin_only = Router::new()
        .route("/admin/assets", post(handlers::assets::upload))
        .route(
            "/admin/assets/{id}",
            put(handlers::assets::update).delete(handlers::assets::delete),
        )
        .route(
            "/admin/assets/{id}/source",
            post(handlers::assets::replace_source),
        )
        .route(
            "/admin/assets/{id}/convert",
            post(handlers::assets::convert),
        )
        .route(
            "/admin/users/{id}/kick-session",
            post(handlers::manage::kick_user_sessions),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            middleware::RoleGuard(vec![twg_domain::Role::Admin]),
            middleware::require_roles,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_session,
        ));

    let uploads_service =
        ServeDir::new(state.uploads_dir.clone()).append_index_html_on_directories(false);
    let uploads_router = Router::<AppState>::new()
        .nest_service("/uploads", uploads_service)
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'none'; sandbox"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("cross-origin-resource-policy"),
            HeaderValue::from_static("same-site"),
        ));

    let asset_files_service =
        ServeDir::new(state.assets_dir.clone()).append_index_html_on_directories(false);
    let asset_files_router = Router::<AppState>::new()
        .nest_service("/assets", asset_files_service)
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'none'; sandbox"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("cross-origin-resource-policy"),
            HeaderValue::from_static("same-site"),
        ));

    Router::<AppState>::new()
        .merge(public)
        .merge(logout)
        .merge(private)
        .merge(private_snapshot_writes)
        .merge(admin)
        .merge(admin_only)
        .merge(uploads_router)
        .merge(asset_files_router)
        .merge(tiles::router::<AppState>())
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-max-body-bytes"),
            max_body_header,
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(axum::middleware::from_fn(middleware::client_ip))
        .layer(axum::middleware::from_fn(middleware::request_id))
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                tracing::info_span!(
                    "http",
                    method = %request.method(),
                    path = %request.uri().path()
                )
            },
        ))
        .layer(cors)
        .with_state(state)
}

fn cors_layer(domains: &[String]) -> CorsLayer {
    let allow_any = domains.iter().any(|domain| domain == "*");
    let methods = AllowMethods::list([
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
    ]);
    let headers = [
        header::AUTHORIZATION,
        header::CONTENT_TYPE,
        header::ACCEPT,
        HeaderName::from_static("x-device-id"),
        HeaderName::from_static("x-request-id"),
    ];
    let exposed = [
        HeaderName::from_static("x-request-id"),
        HeaderName::from_static("x-max-body-bytes"),
    ];

    let mut layer = CorsLayer::new()
        .allow_methods(methods)
        .allow_headers(headers)
        .expose_headers(exposed)
        .max_age(Duration::from_secs(86_400));
    if allow_any {
        layer = layer.allow_origin(AllowOrigin::any());
    } else {
        let origins = domains
            .iter()
            .filter_map(|domain| HeaderValue::from_str(domain).ok())
            .collect::<Vec<_>>();
        layer = layer
            .allow_origin(AllowOrigin::list(origins))
            .allow_credentials(true);
    }
    layer
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{HeaderMap, Method, Request, StatusCode, header};
    use chrono::{Duration as ChronoDuration, Utc};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use twg_auth::{
        ACCESS_TOKEN_TYPE, AuthConfig, AuthService, AuthUser, InMemoryAuthStore, PasetoMaker,
    };
    use twg_domain::Role;
    use uuid::Uuid;

    use crate::config::RateLimitConfig;
    use crate::router;
    use crate::state::AppState;

    const LEGACY_HASH: &str = "$2a$10$7BuDXppm38Mpqv/04I3qQOjuBC4HqDGjjgBtUziquCOMW0iS2aV9m";

    fn test_state(attempts: u32) -> AppState {
        let store = InMemoryAuthStore::new();
        let now = Utc::now();
        let roles = [
            (1, "baselineadmin", Role::Admin, None),
            (2, "baselinemonitor", Role::AdminMonitor, None),
            (
                3,
                "baselineadminsatker",
                Role::AdminSatker,
                Some(Uuid::from_u128(11)),
            ),
            (
                4,
                "baselineusersatker",
                Role::UserSatker,
                Some(Uuid::from_u128(11)),
            ),
            (5, "baselineuser", Role::User, None),
        ];
        for (id, username, role, satker_id) in roles {
            store.insert_user(AuthUser {
                id: Uuid::from_u128(id),
                name: username.to_owned(),
                username: username.to_owned(),
                password_hash: LEGACY_HASH.to_owned(),
                role,
                satker_id,
                profile_img: String::new(),
                units: "[]".to_owned(),
                created_at: now,
                updated_at: now,
            });
        }
        let auth = AuthService::new(Arc::new(store), AuthConfig::default());
        AppState::for_test(
            auth,
            RateLimitConfig {
                attempts,
                window: Duration::from_secs(60),
            },
        )
    }

    async fn send_raw(
        app: Router,
        method: Method,
        uri: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut builder = Request::builder().method(method).uri(uri);
        let mut content_type = false;
        for (name, value) in headers {
            builder = builder.header(*name, *value);
            if name.eq_ignore_ascii_case("content-type") {
                content_type = true;
            }
        }
        if body.is_some() && !content_type {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        let body = body
            .map(|value| Body::from(value.to_string()))
            .unwrap_or_else(Body::empty);
        let response = app.oneshot(builder.body(body).unwrap()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, bytes.to_vec())
    }

    async fn send(
        app: Router,
        method: Method,
        uri: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> (StatusCode, HeaderMap, Value) {
        let (status, headers, bytes) = send_raw(app, method, uri, body, headers).await;
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, value)
    }

    fn login_body(username: &str, device_id: &str) -> Value {
        json!({
            "username": username,
            "password": "Baseline#2026",
            "device_id": device_id
        })
    }

    async fn login(app: Router, username: &str, device: &str) -> Value {
        let (status, _, body) = send(
            app,
            Method::POST,
            "/auth/login",
            Some(login_body(username, device)),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "login failed: {body}");
        body
    }

    fn bearer(value: &Value) -> String {
        format!("Bearer {}", value["data"]["access_token"].as_str().unwrap())
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let (status, _, body) =
            send(router(test_state(100)), Method::GET, "/healthz", None, &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test]
    async fn readyz_is_unavailable_without_a_pool() {
        let (status, _, body) =
            send(router(test_state(100)), Method::GET, "/readyz", None, &[]).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "unavailable");
    }

    #[tokio::test]
    async fn login_all_roles_uses_the_legacy_envelope() {
        let app = router(test_state(100));
        for username in [
            "baselineadmin",
            "baselinemonitor",
            "baselineadminsatker",
            "baselineusersatker",
            "baselineuser",
        ] {
            let body = login(app.clone(), username, "device-1").await;
            assert_eq!(body["code"], 200);
            assert_eq!(body["message"], "success");
            assert_eq!(body["data"]["user"]["username"], username);
            assert!(
                body["data"]["access_token"]
                    .as_str()
                    .unwrap()
                    .starts_with("v2.local.")
            );
            assert!(
                body["data"]["refresh_token"]
                    .as_str()
                    .unwrap()
                    .starts_with("v2.local.")
            );
        }
    }

    #[tokio::test]
    async fn wrong_password_returns_401() {
        let mut body = login_body("baselineadmin", "device-1");
        body["password"] = json!("wrong-password");
        let (status, _, response) = send(
            router(test_state(100)),
            Method::POST,
            "/auth/login",
            Some(body),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(response["code"], 401);
        assert_eq!(response["message"], "invalid password");
    }

    #[tokio::test]
    async fn missing_device_id_is_rejected() {
        let body = json!({"username": "baselineadmin", "password": "Baseline#2026"});
        let (status, _, response) = send(
            router(test_state(100)),
            Method::POST,
            "/auth/login",
            Some(body),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(response["message"], "device_id is required");
    }

    #[tokio::test]
    async fn private_routes_require_bearer_and_device() {
        let app = router(test_state(100));
        let (status, _, body) =
            send(app.clone(), Method::GET, "/private/identify", None, &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["message"], "authorization header is not provided");

        let login = login(app.clone(), "baselineuser", "device-1").await;
        let (status, _, body) = send(
            app,
            Method::GET,
            "/private/identify",
            None,
            &[("authorization", &bearer(&login))],
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["message"], "device_id is required");
    }

    #[tokio::test]
    async fn identify_returns_a_new_token_pair() {
        let app = router(test_state(100));
        let login = login(app.clone(), "baselineuser", "device-1").await;
        let authorization = bearer(&login);
        let (status, _, body) = send(
            app,
            Method::GET,
            "/private/identify",
            None,
            &[
                ("authorization", &authorization),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["code"], 200);
        assert_ne!(
            body["data"]["refresh_token"],
            login["data"]["refresh_token"]
        );
    }

    #[tokio::test]
    async fn refresh_token_is_rejected_as_an_access_token() {
        let app = router(test_state(100));
        let login = login(app.clone(), "baselineuser", "device-1").await;
        let refresh = login["data"]["refresh_token"].as_str().unwrap();
        let authorization = format!("Bearer {refresh}");
        let (status, _, body) = send(
            app,
            Method::GET,
            "/private/identify",
            None,
            &[
                ("authorization", &authorization),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            body["message"],
            "refresh token is not allowed for authorization header"
        );
    }

    #[tokio::test]
    async fn refresh_rotates_once_and_rejects_reuse() {
        let app = router(test_state(100));
        let login = login(app.clone(), "baselineuser", "device-1").await;
        let refresh = login["data"]["refresh_token"].as_str().unwrap();
        let body = json!({"refresh_token": refresh, "device_id": "device-1"});
        let (status, _, refreshed) = send(
            app.clone(),
            Method::POST,
            "/auth/refresh",
            Some(body.clone()),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(refreshed["data"]["refresh_token"], refresh);
        let (status, _, replay) = send(app, Method::POST, "/auth/refresh", Some(body), &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(replay["message"], "invalid refresh token");
    }

    #[tokio::test]
    async fn logout_revokes_the_session() {
        let app = router(test_state(100));
        let login = login(app.clone(), "baselineuser", "device-1").await;
        let authorization = bearer(&login);
        let (status, _, body) = send(
            app.clone(),
            Method::POST,
            "/auth/logout",
            Some(json!({"device_id": "device-1"})),
            &[("authorization", &authorization)],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["message"], "success");
        let (status, _, body) = send(
            app,
            Method::GET,
            "/private/identify",
            None,
            &[
                ("authorization", &authorization),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["message"], "session revoked");
    }

    #[tokio::test]
    async fn header_device_id_wins_over_the_body() {
        let app = router(test_state(100));
        let (status, _, login) = send(
            app.clone(),
            Method::POST,
            "/auth/login",
            Some(login_body("baselineuser", "body-device")),
            &[("x-device-id", "header-device")],
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let authorization = bearer(&login);
        let (status, _, body) = send(
            app.clone(),
            Method::GET,
            "/private/identify",
            None,
            &[
                ("authorization", &authorization),
                ("x-device-id", "header-device"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, _, body) = send(
            app,
            Method::GET,
            "/private/identify",
            None,
            &[
                ("authorization", &authorization),
                ("x-device-id", "body-device"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["message"], "session revoked");
    }

    #[tokio::test]
    async fn change_password_updates_the_bcrypt_hash() {
        let app = router(test_state(100));
        let login = login(app.clone(), "baselineuser", "device-1").await;
        let authorization = bearer(&login);
        let (status, _, body) = send(
            app.clone(),
            Method::PUT,
            "/private/profile/password",
            Some(json!({
                "old_password": "Baseline#2026",
                "new_password": "NewPassword#2026"
            })),
            &[
                ("authorization", &authorization),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, _, body) = send(
            app,
            Method::POST,
            "/auth/login",
            Some(json!({
                "username": "baselineuser",
                "password": "NewPassword#2026",
                "device_id": "device-1"
            })),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    #[tokio::test]
    async fn expired_tokens_are_rejected() {
        let app = router(test_state(100));
        let maker = PasetoMaker::new(&AuthConfig::default());
        let now = Utc::now();
        let (token, _) = maker
            .create_token(
                Uuid::from_u128(5),
                "baselineuser",
                Role::User,
                None,
                ACCESS_TOKEN_TYPE,
                Duration::from_secs(1),
                now - ChronoDuration::seconds(10),
            )
            .unwrap();
        let authorization = format!("Bearer {token}");
        let (status, _, body) = send(
            app,
            Method::GET,
            "/private/identify",
            None,
            &[
                ("authorization", &authorization),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["message"], "token has expired");
    }

    #[tokio::test]
    async fn invalid_tokens_are_rejected() {
        let (status, _, body) = send(
            router(test_state(100)),
            Method::GET,
            "/private/identify",
            None,
            &[
                ("authorization", "Bearer nope"),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["message"], "token is invalid");
    }

    #[tokio::test]
    async fn request_id_is_echoed() {
        let (status, headers, _) = send(
            router(test_state(100)),
            Method::GET,
            "/healthz",
            None,
            &[("x-request-id", "test-request-id")],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["x-request-id"], "test-request-id");
    }

    #[tokio::test]
    async fn cors_preflight_uses_the_allowlist() {
        let app = router(test_state(100));
        let (status, headers, _) = send(
            app,
            Method::OPTIONS,
            "/auth/login",
            None,
            &[
                ("origin", "http://localhost:5173"),
                ("access-control-request-method", "POST"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers["access-control-allow-origin"],
            "http://localhost:5173"
        );
    }

    #[tokio::test]
    async fn login_rate_limit_returns_429() {
        let app = router(test_state(1));
        let first = send(
            app.clone(),
            Method::POST,
            "/auth/login",
            Some(login_body("baselineuser", "device-1")),
            &[],
        )
        .await;
        assert_eq!(first.0, StatusCode::CREATED);
        let (status, headers, body) = send(
            app,
            Method::POST,
            "/auth/login",
            Some(login_body("baselineuser", "device-1")),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert!(headers.contains_key("retry-after"));
        assert_eq!(body["message"], "too many requests");
    }

    #[tokio::test]
    async fn body_limit_is_enforced() {
        let mut state = test_state(100);
        state.max_body_bytes = 32;
        let app = router(state);
        let (status, _, _) = send(
            app,
            Method::POST,
            "/auth/login",
            Some(login_body("baselineuser", "device-1")),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn asset_routes_require_authentication() {
        let app = router(test_state(100));
        let (status, _, body) = send(app.clone(), Method::GET, "/private/assets", None, &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], 401);

        let (status, _, _) = send(app, Method::POST, "/admin/assets", None, &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_tool_login_is_admin_only_and_multi_device() {
        let app = router(test_state(100));
        login(app.clone(), "baselineadmin", "device-1").await;

        // The regular route still enforces single-device for admins.
        let (status, _, _) = send(
            app.clone(),
            Method::POST,
            "/auth/login",
            Some(login_body("baselineadmin", "device-2")),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // The admin tool route keeps an independent session.
        let (status, _, body) = send(
            app.clone(),
            Method::POST,
            "/auth/login/admin",
            Some(login_body("baselineadmin", "device-2")),
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["data"]["user"]["username"], "baselineadmin");

        // It cannot be used by other roles.
        for username in ["baselinemonitor", "baselineadminsatker", "baselineuser"] {
            let (status, _, body) = send(
                app.clone(),
                Method::POST,
                "/auth/login/admin",
                Some(login_body(username, "device-9")),
                &[],
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{username}: {body}");
        }
    }

    #[tokio::test]
    async fn kick_endpoint_is_admin_only_and_public_route_is_gone() {
        let app = router(test_state(100));
        let target = Uuid::from_u128(99);
        // The legacy unauthenticated route must no longer exist.
        let (status, _, _) = send(
            app.clone(),
            Method::POST,
            &format!("/public/users/{target}/kick-session"),
            None,
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        for (username, expected) in [
            ("baselineuser", StatusCode::FORBIDDEN),
            ("baselinemonitor", StatusCode::FORBIDDEN),
            ("baselineadminsatker", StatusCode::FORBIDDEN),
            ("baselineadmin", StatusCode::SERVICE_UNAVAILABLE),
        ] {
            let session = login(app.clone(), username, "device-1").await;
            let (status, _, body) = send(
                app.clone(),
                Method::POST,
                &format!("/admin/users/{target}/kick-session"),
                None,
                &[
                    ("authorization", bearer(&session).as_str()),
                    ("x-device-id", "device-1"),
                ],
            )
            .await;
            assert_eq!(status, expected, "{username}: {body}");
        }
    }

    #[tokio::test]
    async fn online_users_are_available_to_authenticated_sessions() {
        let app = router(test_state(100));
        let session = login(app.clone(), "baselineuser", "device-1").await;
        let (status, _, body) = send(
            app,
            Method::GET,
            "/private/users/online",
            None,
            &[
                ("authorization", bearer(&session).as_str()),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["message"], "database is not available");
    }

    #[tokio::test]
    async fn asset_writes_are_admin_only() {
        let app = router(test_state(100));
        let asset_id = Uuid::nil();
        for username in ["baselineuser", "baselinemonitor", "baselineadminsatker"] {
            let session = login(app.clone(), username, "device-1").await;
            for path in [
                "/admin/assets".to_owned(),
                format!("/admin/assets/{asset_id}/source"),
            ] {
                let (status, _, body) = send(
                    app.clone(),
                    Method::POST,
                    &path,
                    None,
                    &[
                        ("authorization", bearer(&session).as_str()),
                        ("x-device-id", "device-1"),
                    ],
                )
                .await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{username} {path}: {body}");
            }
        }
    }

    #[tokio::test]
    async fn asset_reads_need_a_database() {
        let app = router(test_state(100));
        let session = login(app.clone(), "baselineuser", "device-1").await;
        let (status, _, body) = send(
            app,
            Method::GET,
            "/private/assets/manifest",
            None,
            &[
                ("authorization", bearer(&session).as_str()),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["message"], "database is not available");
    }

    #[tokio::test]
    async fn marker_unit_update_needs_a_database() {
        let app = router(test_state(100));
        let session = login(app.clone(), "baselineuser", "device-1").await;
        let marker_id = Uuid::nil();
        let unit_id = Uuid::new_v4().to_string();
        let (status, _, body) = send(
            app,
            Method::PUT,
            &format!("/private/markers/unit/{marker_id}"),
            Some(json!({ "unit_id": unit_id })),
            &[
                ("authorization", bearer(&session).as_str()),
                ("x-device-id", "device-1"),
            ],
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(body["message"], "database is not available");
    }

    #[tokio::test]
    async fn read_endpoints_require_a_database() {
        let (status, _, body) = send(
            router(test_state(100)),
            Method::GET,
            "/public/list/operasis",
            None,
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["message"], "database is not available");
    }

    #[tokio::test]
    async fn tile_route_is_registered() {
        let (status, _, _) = send(
            router(test_state(100)),
            Method::GET,
            "/tiles/unknown/0/0/0.png",
            None,
            &[],
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn uploads_are_served_without_path_traversal() {
        let dir = std::env::temp_dir().join(format!("twg-uploads-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ok.txt"), b"ok").unwrap();
        let mut state = test_state(100);
        state.uploads_dir = dir.clone();
        let app = router(state);
        let (status, headers, body) =
            send_raw(app.clone(), Method::GET, "/uploads/ok.txt", None, &[]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"ok");
        assert_eq!(headers["x-content-type-options"], "nosniff");

        let (status, _, _) = send_raw(app, Method::GET, "/uploads/../Cargo.toml", None, &[]).await;
        assert_ne!(status, StatusCode::OK);
        let _ = std::fs::remove_dir_all(dir);
    }
}
