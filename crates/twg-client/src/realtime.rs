use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use uuid::Uuid;

use crate::api::reconnect_delay;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RealtimeStatus {
    #[default]
    Disabled,
    Connecting,
    Connected,
    Reconnecting {
        attempt: u32,
        retry_in_ms: u64,
    },
    Closed,
    Error(String),
}

impl RealtimeStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Disabled => "Realtime nonaktif".to_owned(),
            Self::Connecting => "Menghubungkan...".to_owned(),
            Self::Connected => "Realtime tersambung".to_owned(),
            Self::Reconnecting {
                attempt,
                retry_in_ms,
            } => format!("Menyambung ulang ({attempt}) dalam {retry_in_ms} ms"),
            Self::Closed => "Realtime terputus".to_owned(),
            Self::Error(message) => format!("Realtime error: {message}"),
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }
}

/// Transport-agnostic WebSocket connection.
///
/// Native and web transports own the socket and write incoming text frames into
/// `inbound`. The UI drains them each frame, so no async runtime is required in
/// the client crate.
#[derive(Clone)]
pub struct WsConnection {
    inbound: Arc<Mutex<VecDeque<String>>>,
    outbound: Arc<Mutex<VecDeque<String>>>,
    status: Arc<Mutex<RealtimeStatus>>,
    closed: Arc<AtomicBool>,
}

impl WsConnection {
    pub fn new() -> Self {
        Self {
            inbound: Arc::new(Mutex::new(VecDeque::new())),
            outbound: Arc::new(Mutex::new(VecDeque::new())),
            status: Arc::new(Mutex::new(RealtimeStatus::Connecting)),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn status(&self) -> RealtimeStatus {
        self.status.lock().expect("ws status poisoned").clone()
    }

    pub fn set_status(&self, status: RealtimeStatus) {
        *self.status.lock().expect("ws status poisoned") = status;
    }

    pub fn push_inbound(&self, payload: String) {
        self.inbound
            .lock()
            .expect("ws inbound poisoned")
            .push_back(payload);
    }

    pub fn send_text(&self, payload: String) -> bool {
        if self.closed.load(Ordering::Relaxed) {
            return false;
        }
        self.outbound
            .lock()
            .expect("ws outbound poisoned")
            .push_back(payload);
        true
    }

    pub fn poll_inbound(&self) -> Option<String> {
        self.inbound
            .lock()
            .expect("ws inbound poisoned")
            .pop_front()
    }

    pub fn drain_outbound(&self) -> Vec<String> {
        self.outbound
            .lock()
            .expect("ws outbound poisoned")
            .drain(..)
            .collect()
    }

    /// Puts a frame back at the head after a would-block write.
    pub fn requeue_outbound(&self, payload: String) {
        self.outbound
            .lock()
            .expect("ws outbound poisoned")
            .push_front(payload);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        *self.status.lock().expect("ws status poisoned") = RealtimeStatus::Closed;
    }
}

impl Default for WsConnection {
    fn default() -> Self {
        Self::new()
    }
}

pub trait WsTransport: Send + Sync {
    fn connect(&self, url: String) -> WsConnection;
}

#[derive(Debug, Clone, Default)]
pub struct RealtimeState {
    pub status: RealtimeStatus,
    pub last_sequence: HashMap<Uuid, u64>,
    pub events_received: u64,
    pub reconnect_attempt: u32,
}

impl RealtimeState {
    pub fn note_event(&mut self) {
        self.events_received = self.events_received.saturating_add(1);
    }

    pub fn note_sequence(&mut self, skenario_id: Uuid, sequence: u64) {
        let current = self.last_sequence.entry(skenario_id).or_insert(0);
        if sequence > *current {
            *current = sequence;
        }
    }

    pub fn schedule_reconnect(&mut self) {
        self.reconnect_attempt = self.reconnect_attempt.saturating_add(1);
        let delay = reconnect_delay(self.reconnect_attempt - 1);
        self.status = RealtimeStatus::Reconnecting {
            attempt: self.reconnect_attempt,
            retry_in_ms: delay.as_millis() as u64,
        };
    }

    pub fn reset_reconnect(&mut self) {
        self.reconnect_attempt = 0;
    }

    pub fn retry_delay(&self) -> Duration {
        reconnect_delay(self.reconnect_attempt.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::{RealtimeState, RealtimeStatus, WsConnection};
    use uuid::Uuid;

    #[test]
    fn queue_round_trips() {
        let connection = WsConnection::new();
        connection.push_inbound("{\"type\":\"hello\"}".to_owned());
        assert_eq!(
            connection.poll_inbound().as_deref(),
            Some("{\"type\":\"hello\"}")
        );
        assert!(connection.send_text("{}".to_owned()));
        assert_eq!(connection.drain_outbound(), vec!["{}".to_owned()]);
    }

    #[test]
    fn close_blocks_new_messages() {
        let connection = WsConnection::new();
        connection.close();
        assert!(connection.is_closed());
        assert_eq!(connection.status(), RealtimeStatus::Closed);
        assert!(!connection.send_text("{}".to_owned()));
    }

    #[test]
    fn sequences_are_monotonic() {
        let mut state = RealtimeState::default();
        let scenario = Uuid::from_u128(1);
        state.note_sequence(scenario, 5);
        state.note_sequence(scenario, 3);
        assert_eq!(state.last_sequence[&scenario], 5);
        state.note_event();
        assert_eq!(state.events_received, 1);
    }

    #[test]
    fn reconnect_backoff_grows_and_resets() {
        let mut state = RealtimeState::default();
        state.schedule_reconnect();
        assert_eq!(state.retry_delay().as_millis(), 500);
        state.schedule_reconnect();
        assert_eq!(state.retry_delay().as_millis(), 1000);
        state.reset_reconnect();
        assert_eq!(state.retry_delay().as_millis(), 500);
    }
}
