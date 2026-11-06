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
    Ping {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
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
    fn protocol_constant_is_stable() {
        assert_eq!(WS_PROTOCOL, "twg.ws.v1");
    }
}
