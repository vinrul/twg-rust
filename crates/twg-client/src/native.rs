//! Native transports: blocking `ureq` HTTP on a worker thread and a
//! `tungstenite` WebSocket driven by one non-blocking poll loop.

use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use crate::api::{
    HttpMethod, HttpRequest, HttpResponse, PendingResponse, Transport, TransportError,
};
use crate::realtime::{RealtimeStatus, WsConnection, WsTransport};
use crate::store::{StoredSession, TokenStore};

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const WS_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// `ureq` based transport. Each request runs on its own thread so Bevy systems
/// never block on the network.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(HTTP_TIMEOUT))
            .build()
            .into();
        Self { agent }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for UreqTransport {
    fn execute(&self, request: HttpRequest) -> PendingResponse {
        let pending = PendingResponse::new();
        let slot = pending.clone();
        let agent = self.agent.clone();
        std::thread::spawn(move || slot.complete(run_request(&agent, request)));
        pending
    }
}

fn run_request(agent: &ureq::Agent, request: HttpRequest) -> Result<HttpResponse, TransportError> {
    macro_rules! prepare {
        ($builder:expr) => {{
            let mut builder = $builder;
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            builder
        }};
    }
    let result = match request.method {
        HttpMethod::Get => prepare!(agent.get(&request.url)).call(),
        HttpMethod::Delete => prepare!(agent.delete(&request.url)).call(),
        HttpMethod::Post => {
            prepare!(agent.post(&request.url)).send(request.body.unwrap_or_default())
        }
        HttpMethod::Put => prepare!(agent.put(&request.url)).send(request.body.unwrap_or_default()),
        HttpMethod::Patch => {
            prepare!(agent.patch(&request.url)).send(request.body.unwrap_or_default())
        }
    };
    let mut response = result.map_err(|error| TransportError(error.to_string()))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| TransportError(error.to_string()))?;
    Ok(HttpResponse { status, body })
}

/// `tungstenite` based transport. Only one thread owns the socket: it polls for
/// inbound frames and flushes queued outbound frames on the same loop, which
/// keeps reads non-blocking with respect to UI writes.
pub struct TungsteniteTransport;

impl WsTransport for TungsteniteTransport {
    fn connect(&self, url: String) -> WsConnection {
        let connection = WsConnection::new();
        let worker = connection.clone();
        std::thread::spawn(move || run_socket(url, worker));
        connection
    }
}

fn run_socket(url: String, connection: WsConnection) {
    let mut socket = match connect(&url) {
        Ok(socket) => socket,
        Err(error) => {
            connection.set_status(RealtimeStatus::Error(error));
            connection.close();
            return;
        }
    };
    connection.set_status(RealtimeStatus::Connected);
    let mut failures: u32 = 0;
    loop {
        if connection.is_closed() {
            break;
        }
        for payload in connection.drain_outbound() {
            match socket.send(tungstenite::Message::Text(payload.clone().into())) {
                Ok(()) => {}
                Err(tungstenite::Error::Io(error))
                    if error.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    connection.requeue_outbound(payload);
                    std::thread::sleep(WS_POLL_INTERVAL);
                    break;
                }
                Err(error) => {
                    connection.set_status(RealtimeStatus::Error(error.to_string()));
                    connection.close();
                    return;
                }
            }
        }
        match socket.read() {
            Ok(tungstenite::Message::Text(text)) => {
                failures = 0;
                connection.push_inbound(text.to_string());
            }
            Ok(tungstenite::Message::Ping(payload)) => {
                failures = 0;
                if let Err(error) = socket.send(tungstenite::Message::Pong(payload)) {
                    connection.set_status(RealtimeStatus::Error(error.to_string()));
                    connection.close();
                    return;
                }
            }
            Ok(tungstenite::Message::Pong(_)) => failures = 0,
            Ok(tungstenite::Message::Close(_)) => {
                connection.close();
                return;
            }
            Ok(_) => failures = 0,
            Err(tungstenite::Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                std::thread::sleep(WS_POLL_INTERVAL);
            }
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                connection.close();
                return;
            }
            Err(error) => {
                failures = failures.saturating_add(1);
                if failures > 3 {
                    connection.set_status(RealtimeStatus::Error(error.to_string()));
                    connection.close();
                    return;
                }
                std::thread::sleep(WS_POLL_INTERVAL);
            }
        }
    }
}

type NativeSocket =
    tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

fn connect(url: &str) -> Result<NativeSocket, String> {
    let (host, port) = host_and_port(url)?;
    let stream = TcpStream::connect((host.as_str(), port))
        .map_err(|error| format!("websocket connect {host}:{port}: {error}"))?;
    let _ = stream.set_nodelay(true);
    let probe = stream
        .try_clone()
        .map_err(|error| format!("websocket clone socket: {error}"))?;
    let (socket, _) = tungstenite::client_tls_with_config(url, stream, None, None)
        .map_err(|error| format!("websocket handshake: {error}"))?;
    probe
        .set_nonblocking(true)
        .map_err(|error| format!("websocket nonblocking: {error}"))?;
    Ok(socket)
}

fn host_and_port(url: &str) -> Result<(String, u16), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("invalid websocket url: {url}"))?;
    let default_port = match scheme {
        "wss" | "https" => 443,
        "ws" | "http" => 80,
        _ => return Err(format!("unsupported websocket scheme: {scheme}")),
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port
            .parse()
            .map_err(|error| format!("invalid websocket port: {error}"))?;
        (host.to_owned(), port)
    } else {
        (authority.to_owned(), default_port)
    };
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    if host.is_empty() {
        return Err(format!("invalid websocket url: {url}"));
    }
    Ok((host, port))
}

/// Session store under the user profile, with an env override for tests.
pub struct FileTokenStore {
    path: PathBuf,
}

impl FileTokenStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> PathBuf {
        if let Ok(path) = std::env::var("TWG_TOKEN_FILE") {
            return path.into();
        }
        let base = if cfg!(windows) {
            std::env::var("APPDATA").map(PathBuf::from)
        } else {
            std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|_| std::env::var("HOME").map(|home| PathBuf::from(home).join(".config")))
        };
        base.unwrap_or_else(|_| PathBuf::from("."))
            .join("twg")
            .join("session.json")
    }
}

impl Default for FileTokenStore {
    fn default() -> Self {
        Self::new(Self::default_path())
    }
}

impl TokenStore for FileTokenStore {
    fn load(&self) -> Option<StoredSession> {
        let json = std::fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&json).ok()
    }

    fn save(&self, session: &StoredSession) {
        let Some(parent) = self.path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let Ok(json) = serde_json::to_string_pretty(session) else {
            return;
        };
        let temporary = self.path.with_extension("json.tmp");
        if std::fs::write(&temporary, json).is_ok() {
            let _ = std::fs::rename(&temporary, &self.path);
        }
    }

    fn clear(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A device id that survives restarts, stored next to the session file.
pub fn persisted_device_id() -> String {
    let path = FileTokenStore::default_path().with_file_name("device-id");
    if let Ok(value) = std::fs::read_to_string(&path) {
        let value = value.trim();
        if !value.is_empty() {
            return value.to_owned();
        }
    }
    let device_id = crate::store::generate_device_id();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, &device_id);
    device_id
}

#[cfg(test)]
mod tests {
    use super::{FileTokenStore, host_and_port};
    use crate::store::{StoredSession, TokenStore};
    use twg_contracts::auth::UserIdentity;
    use twg_domain::Role;
    use uuid::Uuid;

    #[test]
    fn websocket_urls_split_host_and_port() {
        assert_eq!(
            host_and_port("ws://127.0.0.1:8010/ws?ticket=abc").unwrap(),
            ("127.0.0.1".to_owned(), 8010)
        );
        assert_eq!(
            host_and_port("wss://example.test/ws").unwrap(),
            ("example.test".to_owned(), 443)
        );
    }

    #[test]
    fn file_store_round_trips() {
        let directory = std::env::temp_dir().join(format!("twg-store-{}", Uuid::new_v4()));
        let path = directory.join("session.json");
        let store = FileTokenStore::new(&path);
        let session = StoredSession {
            access_token: "access".to_owned(),
            refresh_token: "refresh".to_owned(),
            access_expires_at: "2026-01-01T00:00:00+00:00".to_owned(),
            refresh_expires_at: "2026-01-08T00:00:00+00:00".to_owned(),
            device_id: "device".to_owned(),
            user: UserIdentity {
                id: Uuid::nil(),
                name: "User".to_owned(),
                username: "user".to_owned(),
                role: Role::Admin,
                satker_id: None,
                profile_img: String::new(),
                units: Vec::new(),
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            },
        };
        store.save(&session);
        assert_eq!(store.load().unwrap().access_token, "access");
        store.clear();
        assert!(store.load().is_none());
        let _ = std::fs::remove_dir_all(directory);
    }
}
