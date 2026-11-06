//! Browser transports: `fetch` for HTTP, a native `WebSocket`, and
//! `localStorage` for the session.

use std::sync::{Arc, Mutex};

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};

use crate::api::{HttpRequest, HttpResponse, PendingResponse, Transport, TransportError};
use crate::realtime::{RealtimeStatus, WsConnection, WsTransport};
use crate::store::{StoredSession, TokenStore};

const SESSION_KEY: &str = "twg.session";
const DEVICE_KEY: &str = "twg.device_id";

pub struct FetchTransport;

impl Transport for FetchTransport {
    fn execute(&self, request: HttpRequest) -> PendingResponse {
        let pending = PendingResponse::new();
        let slot = pending.clone();
        spawn_local(async move {
            slot.complete(run(request).await);
        });
        pending
    }
}

async fn run(request: HttpRequest) -> Result<HttpResponse, TransportError> {
    let init = web_sys::RequestInit::new();
    init.set_method(request.method.as_str());
    init.set_mode(web_sys::RequestMode::Cors);
    let headers = web_sys::Headers::new().map_err(|error| js_error("headers", error))?;
    for (name, value) in &request.headers {
        headers
            .set(name, value)
            .map_err(|error| js_error("header", error))?;
    }
    init.set_headers(headers.as_ref());
    if let Some(body) = &request.body {
        init.set_body(&JsValue::from_str(body));
    }
    let request = web_sys::Request::new_with_str_and_init(&request.url, &init)
        .map_err(|error| js_error("request", error))?;
    let window = web_sys::window().ok_or_else(|| TransportError("no browser window".to_owned()))?;
    let promise = window.fetch_with_request(&request);
    let value = JsFuture::from(promise)
        .await
        .map_err(|error| js_error("fetch", error))?;
    let response: web_sys::Response = value
        .dyn_into()
        .map_err(|error| js_error("response", error))?;
    let status = response.status();
    let body = JsFuture::from(
        response
            .text()
            .map_err(|error| js_error("response body", error))?,
    )
    .await
    .map_err(|error| js_error("response body", error))?;
    Ok(HttpResponse {
        status,
        body: body.as_string().unwrap_or_default(),
    })
}

fn js_error(context: &str, error: JsValue) -> TransportError {
    let message = error
        .as_string()
        .or_else(|| {
            js_sys::JSON::stringify(&error)
                .ok()
                .and_then(|value| value.as_string())
        })
        .unwrap_or_else(|| "unknown error".to_owned());
    TransportError(format!("{context}: {message}"))
}

pub struct BrowserWsTransport;

impl WsTransport for BrowserWsTransport {
    fn connect(&self, url: String) -> WsConnection {
        let connection = WsConnection::new();
        let socket = match web_sys::WebSocket::new(&url) {
            Ok(socket) => socket,
            Err(error) => {
                connection.set_status(RealtimeStatus::Error(js_error("websocket", error).0));
                connection.close();
                return connection;
            }
        };

        let open = connection.clone();
        let on_open = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            open.set_status(RealtimeStatus::Connected);
        });
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        let message = connection.clone();
        let on_message = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |event: web_sys::MessageEvent| {
                if let Some(text) = event.data().as_string() {
                    message.push_inbound(text);
                }
            },
        );
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        let failed = connection.clone();
        let on_error =
            wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                failed.set_status(RealtimeStatus::Error("websocket error".to_owned()));
            });
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        let closed = connection.clone();
        let on_close =
            wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                closed.close();
            });
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        on_close.forget();

        // Send queued frames from a timer so `WsConnection::send_text` can stay
        // synchronous for the Bevy systems that call it.
        let interval_handle = Arc::new(Mutex::new(None::<i32>));
        let handle_holder = interval_handle.clone();
        let outbound = connection.clone();
        let sender = socket.clone();
        let flush = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            if outbound.is_closed() {
                if let Some(window) = web_sys::window()
                    && let Some(handle) = *handle_holder.lock().expect("interval poisoned")
                {
                    window.clear_interval_with_handle(handle);
                }
                return;
            }
            for payload in outbound.drain_outbound() {
                if sender.send_with_str(&payload).is_err() {
                    break;
                }
            }
        });
        if let Some(window) = web_sys::window()
            && let Ok(handle) = window.set_interval_with_callback_and_timeout_and_arguments_0(
                flush.as_ref().unchecked_ref(),
                16,
            )
        {
            *interval_handle.lock().expect("interval poisoned") = Some(handle);
        }
        flush.forget();
        connection
    }
}

pub struct LocalStorageTokenStore {
    key: String,
}

impl LocalStorageTokenStore {
    pub fn new() -> Self {
        Self {
            key: SESSION_KEY.to_owned(),
        }
    }
}

impl Default for LocalStorageTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

impl TokenStore for LocalStorageTokenStore {
    fn load(&self) -> Option<StoredSession> {
        let value = storage()?.get_item(&self.key).ok().flatten()?;
        serde_json::from_str(&value).ok()
    }

    fn save(&self, session: &StoredSession) {
        let Ok(value) = serde_json::to_string(session) else {
            return;
        };
        if let Some(storage) = storage() {
            let _ = storage.set_item(&self.key, &value);
        }
    }

    fn clear(&self) {
        if let Some(storage) = storage() {
            let _ = storage.remove_item(&self.key);
        }
    }
}

pub fn persisted_device_id() -> String {
    if let Some(storage) = storage()
        && let Ok(Some(value)) = storage.get_item(DEVICE_KEY)
        && !value.trim().is_empty()
    {
        return value;
    }
    let device_id = crate::store::generate_device_id();
    if let Some(storage) = storage() {
        let _ = storage.set_item(DEVICE_KEY, &device_id);
    }
    device_id
}

/// Base URL from `?api=<origin>`, otherwise the page origin.
pub fn api_base_url() -> String {
    if let Some(value) = query_param("api")
        && !value.trim().is_empty()
    {
        return value.trim_end_matches('/').to_owned();
    }
    web_sys::window()
        .and_then(|window| window.location().origin().ok())
        .unwrap_or_else(|| "http://127.0.0.1:8010".to_owned())
}

pub fn stress_requested() -> bool {
    query_param("stress").is_some_and(|value| !value.trim().is_empty())
        || query_param("benchmark").is_some()
        || query_param("memory").is_some()
}

pub fn query_param(name: &str) -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let search = search.trim_start_matches('?');
    for pair in search.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if percent_decode(key) == name {
            return Some(percent_decode(value));
        }
    }
    None
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push(high * 16 + low);
            index += 3;
            continue;
        }
        if bytes[index] == b'+' {
            output.push(b' ');
        } else {
            output.push(bytes[index]);
        }
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
