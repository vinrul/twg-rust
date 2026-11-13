use serde::{Deserialize, Serialize};
use twg_domain::Role;
use uuid::Uuid;

use crate::snapshots::Snapshot;

pub const WS_PROTOCOL: &str = "twg.ws.v1";
pub const WS_PATH: &str = "/ws";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsTicketResponse {
    pub ticket: String,
    pub protocol: String,
    pub expires_in: u64,
    pub ws_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Subscribe {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        skenario_id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_sequence: Option<u64>,
    },
    Unsubscribe {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        skenario_id: Uuid,
    },
    Publish {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        operasi_id: Uuid,
        skenario_id: Uuid,
        marker_id: Uuid,
        command: String,
        #[serde(default)]
        data: String,
        #[serde(default)]
        is_save: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        idempotency_key: Option<String>,
    },
    Resume {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        skenario_id: Uuid,
        last_sequence: u64,
    },
    Presence {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        skenario_id: Uuid,
    },
    /// Play-session message for the play-only screen. `action` is `join`,
    /// `leave`, `state`, or `sync`; `data` carries an opaque JSON snapshot.
    Play {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        skenario_id: Uuid,
        action: String,
        #[serde(default)]
        data: String,
    },
    /// Starts receiving the public big-screen session. Any authenticated role
    /// may watch; only admin/admin_satker may publish its scenario or camera.
    WatchLive {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    UnwatchLive {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    Ping {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
}

/// Read-only snapshot of the simulation currently driving the big-screen page.
/// It is only sent over an authenticated WebSocket connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveSession {
    pub scenario_id: Uuid,
    /// Increases every time an admin starts (or reloads) the live session.
    pub session_version: u64,
    /// Increases when an admin reloads the scenario without starting a new
    /// session.
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Monotonic version of `state`; clients only apply changed snapshots.
    pub state_version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    /// Monotonic version of `camera`; clients only apply changed cameras.
    pub camera_version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Hello {
        connection_id: Uuid,
        protocol: String,
        user_id: Uuid,
        role: Role,
    },
    Ack {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        skenario_id: Uuid,
        sequence: u64,
        duplicate: bool,
    },
    Event {
        sequence: u64,
        operasi_id: Uuid,
        skenario_id: Uuid,
        marker_id: Uuid,
        command: String,
        data: String,
        is_save: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        satker_id: Option<Uuid>,
    },
    Presence {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        skenario_id: Uuid,
        count: usize,
        user_ids: Vec<Uuid>,
    },
    /// Play state broadcast between clients in the play-only screen. Editor
    /// play sessions neither send nor apply these messages.
    PlayEvent {
        sequence: u64,
        skenario_id: Uuid,
        /// Connection that produced the event; receivers ignore their own.
        actor: Uuid,
        action: String,
        #[serde(default)]
        data: String,
    },
    /// Big-screen session snapshot. Sent once on `watch_live`, then pushed on
    /// every scenario, state, camera, reload, or stop change.
    Live {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<LiveSession>,
    },
    Resync {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        skenario_id: Uuid,
        sequence: u64,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snapshot: Option<Snapshot>,
    },
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        code: String,
        message: String,
        retryable: bool,
    },
    Pong {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
}

impl ServerMessage {
    pub fn sequence(&self) -> Option<u64> {
        match self {
            Self::Event { sequence, .. } => Some(*sequence),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientMessage, ServerMessage, WS_PROTOCOL};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn client_message_uses_typed_tag() {
        let id = Uuid::from_u128(1);
        let value = serde_json::to_value(ClientMessage::Subscribe {
            request_id: Some("r1".to_owned()),
            skenario_id: id,
            last_sequence: Some(7),
        })
        .unwrap();
        assert_eq!(
            value,
            json!({
                "type": "subscribe",
                "request_id": "r1",
                "skenario_id": id,
                "last_sequence": 7
            })
        );
    }

    #[test]
    fn server_event_keeps_legacy_command_fields() {
        let id = Uuid::from_u128(2);
        let value = serde_json::to_value(ServerMessage::Event {
            sequence: 3,
            operasi_id: id,
            skenario_id: id,
            marker_id: id,
            command: "MOVE".to_owned(),
            data: "{\"x\":1}".to_owned(),
            is_save: 0,
            satker_id: None,
        })
        .unwrap();
        assert_eq!(value["type"], "event");
        assert_eq!(value["command"], "MOVE");
        assert_eq!(value["sequence"], 3);
        assert!(value.get("satker_id").is_none());
    }

    #[test]
    fn play_messages_round_trip() {
        let id = Uuid::from_u128(3);
        let value = serde_json::to_value(ClientMessage::Play {
            request_id: None,
            skenario_id: id,
            action: "join".to_owned(),
            data: String::new(),
        })
        .unwrap();
        assert_eq!(value["type"], "play");
        assert_eq!(value["action"], "join");
        assert!(value.get("request_id").is_none());

        let event = ServerMessage::PlayEvent {
            sequence: 0,
            skenario_id: id,
            actor: id,
            action: "state".to_owned(),
            data: "{}".to_owned(),
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["type"], "play_event");
        assert_eq!(value["action"], "state");
    }

    #[test]
    fn live_watch_messages_round_trip() {
        let id = Uuid::from_u128(4);
        let value = serde_json::to_value(ClientMessage::WatchLive { request_id: None }).unwrap();
        assert_eq!(value, json!({ "type": "watch_live" }));

        let value = serde_json::to_value(ServerMessage::Live {
            request_id: None,
            session: Some(super::LiveSession {
                scenario_id: id,
                session_version: 3,
                revision: 1,
                state: Some("{\"units\":[]}".to_owned()),
                state_version: 7,
                camera: None,
                camera_version: 0,
            }),
        })
        .unwrap();
        assert_eq!(value["type"], "live");
        assert_eq!(value["session"]["scenario_id"], id.to_string());
        assert_eq!(value["session"]["state_version"], 7);
        assert!(value["session"].get("camera").is_none());

        let stopped = serde_json::to_value(ServerMessage::Live {
            request_id: None,
            session: None,
        })
        .unwrap();
        assert_eq!(stopped, json!({ "type": "live" }));
    }

    #[test]
    fn protocol_constant_is_stable() {
        assert_eq!(WS_PROTOCOL, "twg.ws.v1");
    }
}
