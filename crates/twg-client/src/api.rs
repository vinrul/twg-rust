use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use twg_contracts::auth::{LoginRequest, LoginResponse, RefreshTokenRequest};
use twg_contracts::{ListResponse, SingleResponse};

use crate::session::ClientSession;
use crate::store::{StoredSession, TokenStore};

const EXPIRY_MARGIN_SECONDS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    /// Raw request body (multipart uploads). Takes precedence over `body`.
    pub body_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportError(pub String);

impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A transport writes its result into this slot when the request finishes.
#[derive(Debug, Clone)]
pub struct PendingResponse {
    slot: Arc<Mutex<Option<Result<HttpResponse, TransportError>>>>,
}

impl PendingResponse {
    pub fn new() -> Self {
        Self {
            slot: Arc::new(Mutex::new(None)),
        }
    }

    pub fn complete(&self, result: Result<HttpResponse, TransportError>) {
        *self.slot.lock().expect("response slot poisoned") = Some(result);
    }

    pub fn try_take(&self) -> Option<Result<HttpResponse, TransportError>> {
        self.slot.lock().expect("response slot poisoned").take()
    }

    #[cfg(test)]
    pub fn is_ready(&self) -> bool {
        self.slot.lock().expect("response slot poisoned").is_some()
    }
}

impl Default for PendingResponse {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Transport: Send + Sync {
    fn execute(&self, request: HttpRequest) -> PendingResponse;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorKind {
    Transport,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Invalid,
    Server,
    Parse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub status: Option<u16>,
    pub message: String,
    pub retryable: bool,
}

impl ApiError {
    pub fn transport(message: impl Into<String>) -> Self {
        Self {
            kind: ApiErrorKind::Transport,
            status: None,
            message: message.into(),
            retryable: true,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            kind: ApiErrorKind::Unauthorized,
            status: Some(401),
            message: message.into(),
            retryable: false,
        }
    }

    pub fn parse(message: impl Into<String>) -> Self {
        Self {
            kind: ApiErrorKind::Parse,
            status: None,
            message: message.into(),
            retryable: false,
        }
    }

    fn from_status(status: u16, message: String) -> Self {
        let kind = match status {
            401 => ApiErrorKind::Unauthorized,
            403 => ApiErrorKind::Forbidden,
            404 => ApiErrorKind::NotFound,
            409 => ApiErrorKind::Conflict,
            400 | 422 => ApiErrorKind::Invalid,
            _ if status >= 500 => ApiErrorKind::Server,
            _ => ApiErrorKind::Transport,
        };
        Self {
            kind,
            status: Some(status),
            message,
            retryable: status >= 500,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status: u16,
    pub json: Value,
}

impl ApiResponse {
    pub fn data<T: DeserializeOwned>(&self) -> Result<T, ApiError> {
        let envelope: SingleResponse<T> = serde_json::from_value(self.json.clone())
            .map_err(|error| ApiError::parse(format!("invalid response data: {error}")))?;
        Ok(envelope.data)
    }

    pub fn list<T: DeserializeOwned>(&self) -> Result<(Vec<T>, usize), ApiError> {
        let envelope: ListResponse<T> = serde_json::from_value(self.json.clone())
            .map_err(|error| ApiError::parse(format!("invalid response list: {error}")))?;
        Ok((envelope.data, envelope.total))
    }
}

#[derive(Clone)]
pub struct ApiHandle {
    slot: Arc<Mutex<Option<Result<ApiResponse, ApiError>>>>,
}

impl ApiHandle {
    fn new() -> Self {
        Self {
            slot: Arc::new(Mutex::new(None)),
        }
    }

    fn complete(&self, result: Result<ApiResponse, ApiError>) {
        *self.slot.lock().expect("api handle poisoned") = Some(result);
    }

    pub fn try_take(&self) -> Option<Result<ApiResponse, ApiError>> {
        self.slot.lock().expect("api handle poisoned").take()
    }

    pub fn is_ready(&self) -> bool {
        self.slot.lock().expect("api handle poisoned").is_some()
    }
}

#[derive(Debug, Clone)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    access_expires_at: String,
    refresh_expires_at: String,
}

impl Tokens {
    fn access_valid(&self, now: DateTime<FixedOffset>) -> bool {
        expires_after(&self.access_expires_at, now, EXPIRY_MARGIN_SECONDS)
    }

    fn refresh_valid(&self, now: DateTime<FixedOffset>) -> bool {
        expires_after(&self.refresh_expires_at, now, 0)
    }
}

fn expires_after(value: &str, now: DateTime<FixedOffset>, margin_seconds: i64) -> bool {
    DateTime::parse_from_rfc3339(value)
        .map(|expiry| expiry - chrono::Duration::seconds(margin_seconds) > now)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Normal,
    Login,
    Refresh,
    Logout,
}

#[derive(Debug, Clone)]
struct RequestSpec {
    method: HttpMethod,
    path: String,
    body: Option<Value>,
    /// Raw body for multipart uploads.
    body_bytes: Option<Vec<u8>>,
    content_type: Option<String>,
    kind: RequestKind,
}

struct InFlight {
    spec: RequestSpec,
    handle: ApiHandle,
    pending: PendingResponse,
    retried: bool,
}

struct Queued {
    spec: RequestSpec,
    handle: ApiHandle,
}

struct ApiInner {
    base_url: String,
    device_id: String,
    tokens: Option<Tokens>,
    session: Option<ClientSession>,
    store: Arc<dyn TokenStore>,
    in_flight: Vec<InFlight>,
    queued: VecDeque<Queued>,
    refresh: Option<PendingResponse>,
    refresh_waiters: Vec<InFlight>,
}

#[derive(Clone)]
pub struct ApiClient {
    inner: Arc<Mutex<ApiInner>>,
    transport: Arc<dyn Transport>,
}

impl ApiClient {
    pub fn new(
        base_url: impl Into<String>,
        device_id: impl Into<String>,
        transport: Arc<dyn Transport>,
        store: Arc<dyn TokenStore>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ApiInner {
                base_url: base_url.into().trim_end_matches('/').to_owned(),
                device_id: device_id.into(),
                tokens: None,
                session: None,
                store,
                in_flight: Vec::new(),
                queued: VecDeque::new(),
                refresh: None,
                refresh_waiters: Vec::new(),
            })),
            transport,
        }
    }

    pub fn restore(&self) -> Option<ClientSession> {
        let mut inner = self.inner.lock().expect("api state poisoned");
        let stored = inner.store.load()?;
        inner.device_id = stored.device_id.clone();
        inner.tokens = Some(Tokens {
            access_token: stored.access_token,
            refresh_token: stored.refresh_token,
            access_expires_at: stored.access_expires_at,
            refresh_expires_at: stored.refresh_expires_at,
        });
        let session = ClientSession {
            user_id: stored.user.id,
            name: stored.user.name,
            username: stored.user.username,
            role: stored.user.role,
            satker_id: stored.user.satker_id,
            profile_img: stored.user.profile_img,
        };
        inner.session = Some(session.clone());
        Some(session)
    }

    pub fn session(&self) -> Option<ClientSession> {
        self.inner
            .lock()
            .expect("api state poisoned")
            .session
            .clone()
    }

    pub fn device_id(&self) -> String {
        self.inner
            .lock()
            .expect("api state poisoned")
            .device_id
            .clone()
    }

    pub fn clear_session(&self) {
        let mut inner = self.inner.lock().expect("api state poisoned");
        inner.tokens = None;
        inner.session = None;
        inner.store.clear();
    }

    pub fn login(&self, username: &str, password: &str) -> ApiHandle {
        let device_id = self.device_id();
        self.submit(RequestSpec {
            method: HttpMethod::Post,
            path: "/auth/login".to_owned(),
            body: Some(json!(LoginRequest {
                username: username.to_owned(),
                password: password.to_owned(),
                device_id,
            })),
            body_bytes: None,
            content_type: None,
            kind: RequestKind::Login,
        })
    }

    /// Login used by the online-admin tool: admin-only and independent of the
    /// single-device rule enforced by [`ApiClient::login`].
    pub fn login_admin_tool(&self, username: &str, password: &str) -> ApiHandle {
        let device_id = self.device_id();
        self.submit(RequestSpec {
            method: HttpMethod::Post,
            path: "/auth/login/admin".to_owned(),
            body: Some(json!(LoginRequest {
                username: username.to_owned(),
                password: password.to_owned(),
                device_id,
            })),
            body_bytes: None,
            content_type: None,
            kind: RequestKind::Login,
        })
    }

    pub fn logout(&self) -> ApiHandle {
        let device_id = self.device_id();
        self.submit(RequestSpec {
            method: HttpMethod::Post,
            path: "/auth/logout".to_owned(),
            body: Some(json!({ "device_id": device_id })),
            body_bytes: None,
            content_type: None,
            kind: RequestKind::Logout,
        })
    }

    pub fn identify(&self) -> ApiHandle {
        self.request(HttpMethod::Get, "/private/identify", None)
    }

    pub fn request(
        &self,
        method: HttpMethod,
        path: impl Into<String>,
        body: Option<Value>,
    ) -> ApiHandle {
        self.submit(RequestSpec {
            method,
            path: path.into(),
            body,
            body_bytes: None,
            content_type: None,
            kind: RequestKind::Normal,
        })
    }

    /// Sends a raw body (multipart/form-data) instead of JSON. Used by the
    /// admin asset upload, which cannot be represented as a JSON document.
    pub fn upload(
        &self,
        path: impl Into<String>,
        content_type: impl Into<String>,
        body: Vec<u8>,
    ) -> ApiHandle {
        self.submit(RequestSpec {
            method: HttpMethod::Post,
            path: path.into(),
            body: None,
            body_bytes: Some(body),
            content_type: Some(content_type.into()),
            kind: RequestKind::Normal,
        })
    }

    fn submit(&self, spec: RequestSpec) -> ApiHandle {
        let handle = ApiHandle::new();
        let mut inner = self.inner.lock().expect("api state poisoned");
        inner.enqueue(Queued {
            spec,
            handle: handle.clone(),
        });
        inner.start_queued(&*self.transport);
        handle
    }

    /// Advances refresh state and in-flight requests. Call once per frame.
    pub fn tick(&self) {
        let mut inner = self.inner.lock().expect("api state poisoned");
        inner.poll_refresh(&*self.transport);
        inner.poll_in_flight(&*self.transport);
        inner.start_queued(&*self.transport);
        if inner.refresh.is_none() && !inner.refresh_waiters.is_empty() && inner.refresh_available()
        {
            inner.start_refresh(&*self.transport);
        }
        if inner.refresh.is_none()
            && !inner.refresh_waiters.is_empty()
            && !inner.refresh_available()
        {
            inner.fail_waiters(ApiError::unauthorized("session expired"));
            inner.clear_session();
        }
    }

    #[cfg(test)]
    pub fn in_flight_count(&self) -> usize {
        self.inner
            .lock()
            .expect("api state poisoned")
            .in_flight
            .len()
    }
}

impl ApiInner {
    fn enqueue(&mut self, queued: Queued) {
        match queued.spec.kind {
            RequestKind::Login | RequestKind::Refresh => self.queued.push_back(queued),
            RequestKind::Normal | RequestKind::Logout => {
                let now = Utc::now().fixed_offset();
                let access_valid = self
                    .tokens
                    .as_ref()
                    .is_some_and(|tokens| tokens.access_valid(now));
                if access_valid || self.refresh_available() {
                    self.queued.push_back(queued);
                } else {
                    queued
                        .handle
                        .complete(Err(ApiError::unauthorized("session expired")));
                }
            }
        }
    }

    fn refresh_available(&self) -> bool {
        let now = Utc::now().fixed_offset();
        self.tokens
            .as_ref()
            .is_some_and(|tokens| tokens.refresh_valid(now))
    }

    fn start_queued(&mut self, transport: &dyn Transport) {
        while let Some(queued) = self.queued.front() {
            let needs_auth = matches!(queued.spec.kind, RequestKind::Normal | RequestKind::Logout);
            if needs_auth {
                let now = Utc::now().fixed_offset();
                let cannot_start = match &self.tokens {
                    Some(tokens) if tokens.access_valid(now) => false,
                    Some(tokens)
                        if !tokens.refresh_token.is_empty() && tokens.refresh_valid(now) =>
                    {
                        true
                    }
                    _ => {
                        let queued = self.queued.pop_front().expect("front exists");
                        queued
                            .handle
                            .complete(Err(ApiError::unauthorized("session expired")));
                        continue;
                    }
                };
                if cannot_start {
                    if self.refresh.is_none() {
                        self.start_refresh(transport);
                    }
                    break;
                }
            }
            let queued = self.queued.pop_front().expect("front exists");
            let http = self.build_http(&queued.spec);
            let pending = transport.execute(http);
            self.in_flight.push(InFlight {
                spec: queued.spec,
                handle: queued.handle,
                pending,
                retried: false,
            });
        }
    }

    fn start_refresh(&mut self, transport: &dyn Transport) {
        if self.refresh.is_some() {
            return;
        }
        let Some(tokens) = &self.tokens else {
            return;
        };
        let spec = RequestSpec {
            method: HttpMethod::Post,
            path: "/auth/refresh".to_owned(),
            body: Some(json!(RefreshTokenRequest {
                refresh_token: tokens.refresh_token.clone(),
                device_id: self.device_id.clone(),
            })),
            body_bytes: None,
            content_type: None,
            kind: RequestKind::Refresh,
        };
        self.refresh = Some(transport.execute(self.build_http(&spec)));
    }

    fn build_http(&self, spec: &RequestSpec) -> HttpRequest {
        let mut headers = vec![
            ("accept".to_owned(), "application/json".to_owned()),
            ("x-device-id".to_owned(), self.device_id.clone()),
        ];
        if spec.body.is_some() {
            headers.push(("content-type".to_owned(), "application/json".to_owned()));
        }
        if let Some(content_type) = &spec.content_type {
            headers.push(("content-type".to_owned(), content_type.clone()));
        }
        if matches!(spec.kind, RequestKind::Normal | RequestKind::Logout)
            && let Some(tokens) = &self.tokens
        {
            headers.push((
                "authorization".to_owned(),
                format!("Bearer {}", tokens.access_token),
            ));
        }
        HttpRequest {
            method: spec.method,
            url: format!("{}{}", self.base_url, spec.path),
            headers,
            body: spec.body.as_ref().map(ToString::to_string),
            body_bytes: spec.body_bytes.clone(),
        }
    }

    fn poll_refresh(&mut self, transport: &dyn Transport) {
        let Some(pending) = self.refresh.take() else {
            return;
        };
        let Some(result) = pending.try_take() else {
            self.refresh = Some(pending);
            return;
        };
        match result {
            Ok(response) if (200..300).contains(&response.status) => {
                match apply_login_response(self, &response) {
                    Ok(()) => {
                        let waiters = std::mem::take(&mut self.refresh_waiters);
                        for waiter in waiters {
                            self.queued.push_back(Queued {
                                spec: waiter.spec,
                                handle: waiter.handle,
                            });
                        }
                        self.start_queued(transport);
                    }
                    Err(error) => {
                        self.fail_waiters(error.clone());
                        self.fail_queued(error);
                        self.clear_session();
                    }
                }
            }
            Ok(response) => {
                let error = map_error(&response);
                self.fail_waiters(error.clone());
                self.fail_queued(error);
                self.clear_session();
            }
            Err(error) => {
                let error = ApiError::transport(error.to_string());
                self.fail_waiters(error.clone());
                self.fail_queued(error);
                self.clear_session();
            }
        }
    }

    fn poll_in_flight(&mut self, transport: &dyn Transport) {
        let mut still_running = Vec::new();
        let in_flight = std::mem::take(&mut self.in_flight);
        for item in in_flight {
            let is_logout = item.spec.kind == RequestKind::Logout;
            let Some(result) = item.pending.try_take() else {
                still_running.push(item);
                continue;
            };
            match result {
                Err(error) => {
                    if is_logout {
                        self.clear_session();
                    }
                    item.handle
                        .complete(Err(ApiError::transport(error.to_string())));
                }
                Ok(response) => {
                    if is_logout {
                        self.clear_session();
                        item.handle.complete(response_result(&response));
                        continue;
                    }
                    if response.status == 401
                        && item.spec.kind == RequestKind::Normal
                        && !item.retried
                        && self.refresh_available()
                    {
                        let mut item = item;
                        item.retried = true;
                        self.refresh_waiters.push(item);
                        if self.refresh.is_none() {
                            self.start_refresh(transport);
                        }
                        continue;
                    }
                    if item.spec.kind == RequestKind::Login
                        && (200..300).contains(&response.status)
                        && let Err(error) = apply_login_response(self, &response)
                    {
                        item.handle.complete(Err(error));
                        continue;
                    }
                    item.handle.complete(response_result(&response));
                }
            }
        }
        self.in_flight = still_running;
    }

    fn fail_waiters(&mut self, error: ApiError) {
        for waiter in self.refresh_waiters.drain(..) {
            waiter.handle.complete(Err(error.clone()));
        }
    }

    fn fail_queued(&mut self, error: ApiError) {
        while let Some(queued) = self.queued.pop_front() {
            queued.handle.complete(Err(error.clone()));
        }
    }

    fn clear_session(&mut self) {
        self.tokens = None;
        self.session = None;
        self.store.clear();
    }
}

fn apply_login_response(inner: &mut ApiInner, response: &HttpResponse) -> Result<(), ApiError> {
    if !(200..300).contains(&response.status) {
        return Err(map_error(response));
    }
    let json = serde_json::from_str(&response.body)
        .map_err(|error| ApiError::parse(format!("invalid json response: {error}")))?;
    let api_response = ApiResponse {
        status: response.status,
        json,
    };
    let login: LoginResponse = api_response.data()?;
    let device_id = inner.device_id.clone();
    let stored = StoredSession {
        access_token: login.access_token.clone(),
        refresh_token: login.refresh_token.clone(),
        access_expires_at: login.access_token_expires_at.to_rfc3339(),
        refresh_expires_at: login.refresh_token_expires_at.to_rfc3339(),
        device_id,
        user: login.user.clone(),
    };
    inner.tokens = Some(Tokens {
        access_token: stored.access_token.clone(),
        refresh_token: stored.refresh_token.clone(),
        access_expires_at: stored.access_expires_at.clone(),
        refresh_expires_at: stored.refresh_expires_at.clone(),
    });
    inner.session = Some(ClientSession {
        user_id: login.user.id,
        name: login.user.name,
        username: login.user.username,
        role: login.user.role,
        satker_id: login.user.satker_id,
        profile_img: login.user.profile_img,
    });
    inner.store.save(&stored);
    Ok(())
}

fn response_result(response: &HttpResponse) -> Result<ApiResponse, ApiError> {
    if !(200..300).contains(&response.status) {
        return Err(map_error(response));
    }
    let json = if response.body.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&response.body)
            .map_err(|error| ApiError::parse(format!("invalid json response: {error}")))?
    };
    Ok(ApiResponse {
        status: response.status,
        json,
    })
}

fn map_error(response: &HttpResponse) -> ApiError {
    let message = serde_json::from_str::<Value>(&response.body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("HTTP {}", response.status));
    ApiError::from_status(response.status, message)
}

pub fn retry_hint(error: &ApiError) -> &'static str {
    if error.retryable {
        "Coba lagi."
    } else {
        "Periksa input atau hubungi admin."
    }
}

pub fn reconnect_delay(attempt: u32) -> Duration {
    let base = 500u64.saturating_mul(1u64 << attempt.min(6));
    Duration::from_millis(base.min(30_000))
}

#[cfg(test)]
mod tests {
    use super::{
        ApiClient, HttpRequest, HttpResponse, PendingResponse, Transport, TransportError,
        reconnect_delay,
    };
    use crate::store::{MemoryTokenStore, TokenStore};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct ScriptedTransport {
        requests: Mutex<Vec<HttpRequest>>,
        responses: Mutex<Vec<(u16, String)>>,
    }

    impl ScriptedTransport {
        fn new(responses: Vec<(u16, String)>) -> Self {
            Self {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses),
            }
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl Transport for ScriptedTransport {
        fn execute(&self, request: HttpRequest) -> PendingResponse {
            self.requests.lock().unwrap().push(request);
            let pending = PendingResponse::new();
            let response = self.responses.lock().unwrap().remove(0);
            pending.complete(Ok(HttpResponse {
                status: response.0,
                body: response.1,
            }));
            pending
        }
    }

    fn login_body(access: &str, refresh: &str) -> String {
        format!(
            r#"{{"code":200,"message":"success","data":{{"user":{{"id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa1","name":"Admin","username":"baselineadmin","role":"admin","profile_img":"","units":[],"created_at":"2026-01-01T00:00:00+00:00","updated_at":"2026-01-01T00:00:00+00:00"}},"access_token":"{access}","refresh_token":"{refresh}","access_token_expires_at":"2099-01-01T00:00:00+00:00","refresh_token_expires_at":"2099-01-08T00:00:00+00:00"}}}}"#
        )
    }

    #[test]
    fn login_stores_session_and_tokens() {
        let transport = Arc::new(ScriptedTransport::new(vec![(201, login_body("a1", "r1"))]));
        let store = Arc::new(MemoryTokenStore::new());
        let api = ApiClient::new(
            "http://localhost",
            "device-1",
            transport.clone(),
            store.clone(),
        );
        let handle = api.login("baselineadmin", "secret");
        api.tick();
        let response = handle.try_take().expect("login response").expect("ok");
        assert_eq!(response.status, 201);
        assert_eq!(api.session().unwrap().username, "baselineadmin");
        assert_eq!(store.load().unwrap().access_token, "a1");

        let requests = transport.requests();
        assert_eq!(requests[0].url, "http://localhost/auth/login");
        assert!(
            !requests[0]
                .headers
                .iter()
                .any(|(name, _)| name == "authorization")
        );
    }

    #[test]
    fn expired_access_token_refreshes_before_request() {
        let transport = Arc::new(ScriptedTransport::new(vec![
            (200, login_body("a2", "r2")),
            (200, login_body("a3", "r3")),
            (
                200,
                r#"{"code":200,"message":"success","data":[]}"#.to_owned(),
            ),
        ]));
        let store = Arc::new(MemoryTokenStore::new());
        let api = ApiClient::new("http://localhost", "device-1", transport.clone(), store);
        // Seed an expired session directly.
        {
            let login = api.login("x", "y");
            api.tick();
            let _ = login.try_take();
        }
        API_TEST_EXPIRE.with_session_tokens(&api);

        let handle = api.request(super::HttpMethod::Get, "/private/users", None);
        api.tick();
        api.tick();
        assert!(handle.try_take().is_some());
        let requests = transport.requests();
        assert!(
            requests
                .iter()
                .any(|request| request.url.ends_with("/auth/refresh"))
        );
        let authenticated = requests
            .iter()
            .find(|request| request.url.ends_with("/private/users"))
            .expect("users request");
        assert!(
            authenticated
                .headers
                .iter()
                .any(|(name, value)| name == "authorization" && value == "Bearer a3")
        );
    }

    // Helper that mutates the hidden token state for the expiry test.
    struct ApiTestExpire;
    impl ApiTestExpire {
        fn with_session_tokens(&self, api: &ApiClient) {
            let mut inner = api.inner.lock().unwrap();
            if let Some(tokens) = &mut inner.tokens {
                tokens.access_expires_at = "2000-01-01T00:00:00+00:00".to_owned();
                tokens.refresh_expires_at = "2099-01-01T00:00:00+00:00".to_owned();
            }
        }
    }
    static API_TEST_EXPIRE: ApiTestExpire = ApiTestExpire;

    #[test]
    fn unauthorized_request_refreshes_and_retries_once() {
        let transport = Arc::new(ScriptedTransport::new(vec![
            (201, login_body("a1", "r1")),
            (401, r#"{"code":401,"message":"expired"}"#.to_owned()),
            (200, login_body("a2", "r2")),
            (
                200,
                r#"{"code":200,"message":"success","data":[]}"#.to_owned(),
            ),
        ]));
        let api = ApiClient::new(
            "http://localhost",
            "device-1",
            transport.clone(),
            Arc::new(MemoryTokenStore::new()),
        );
        let login = api.login("x", "y");
        api.tick();
        assert!(login.try_take().unwrap().is_ok());

        let handle = api.request(super::HttpMethod::Get, "/private/users", None);
        for _ in 0..4 {
            api.tick();
            if handle.is_ready() {
                break;
            }
        }
        assert!(handle.try_take().expect("retried response").is_ok());
        let requests = transport.requests();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.url.ends_with("/auth/refresh"))
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.url.ends_with("/private/users"))
                .count(),
            2
        );
    }

    #[test]
    fn logout_clears_session_and_store() {
        let transport = Arc::new(ScriptedTransport::new(vec![
            (201, login_body("a1", "r1")),
            (200, String::new()),
        ]));
        let store = Arc::new(MemoryTokenStore::new());
        let api = ApiClient::new(
            "http://localhost",
            "device-1",
            transport.clone(),
            store.clone(),
        );
        let login = api.login("x", "y");
        api.tick();
        assert!(login.try_take().unwrap().is_ok());
        assert!(api.session().is_some());

        let logout = api.logout();
        api.tick();
        assert!(logout.try_take().unwrap().is_ok());
        assert!(api.session().is_none());
        assert!(store.load().is_none());
        let requests = transport.requests();
        assert!(
            requests
                .iter()
                .any(|request| request.url.ends_with("/auth/logout"))
        );
    }

    #[test]
    fn transport_error_is_retryable() {
        struct Failing;
        impl Transport for Failing {
            fn execute(&self, _request: HttpRequest) -> PendingResponse {
                let pending = PendingResponse::new();
                pending.complete(Err(TransportError("boom".to_owned())));
                pending
            }
        }
        let api = ApiClient::new(
            "http://localhost",
            "device-1",
            Arc::new(Failing),
            Arc::new(MemoryTokenStore::new()),
        );
        let handle = api.login("x", "y");
        api.tick();
        let error = handle.try_take().unwrap().unwrap_err();
        assert!(error.retryable);
    }

    #[test]
    fn backoff_is_bounded() {
        assert_eq!(reconnect_delay(0), Duration::from_millis(500));
        assert_eq!(reconnect_delay(2), Duration::from_millis(2000));
        assert_eq!(reconnect_delay(20), Duration::from_millis(30_000));
    }
}
