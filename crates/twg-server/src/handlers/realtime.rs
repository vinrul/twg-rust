use std::time::Duration;

use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::time::{interval, timeout};
use twg_auth::AuthContext;
use twg_contracts::SingleResponse;
use twg_contracts::realtime::{
    ClientMessage, PublicLiveResponse, ServerMessage, WS_PATH, WS_PROTOCOL, WsTicketResponse,
};
use twg_domain::{Role, SessionSubject};
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::read_store;
use crate::middleware::header_device_id;
use crate::state::AppState;

const OUTBOUND_QUEUE: usize = 256;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(25);
const IDLE_TIMEOUT: Duration = Duration::from_secs(75);
const MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
pub struct WsTicketQuery {
    ticket: String,
}

pub async fn ws_ticket(
    State(state): State<AppState>,
    Extension(context): Extension<AuthContext>,
    headers: HeaderMap,
) -> Result<Json<SingleResponse<WsTicketResponse>>, ApiError> {
    let device_id = header_device_id(&headers)
        .ok_or_else(|| ApiError::unauthorized("device_id is required"))?;
    let (ticket, ttl) = state.realtime.tickets.issue(context.subject, device_id);
    Ok(Json(SingleResponse::success(WsTicketResponse {
        ticket,
        protocol: WS_PROTOCOL.to_owned(),
        expires_in: ttl.as_secs(),
        ws_path: WS_PATH.to_owned(),
    })))
}

pub async fn ws_upgrade(
    State(state): State<AppState>,
    Query(query): Query<WsTicketQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let claims = state
        .realtime
        .tickets
        .consume(&query.ticket)
        .ok_or_else(|| ApiError::unauthorized("invalid or expired ws ticket"))?;
    state
        .auth
        .validate_session(&claims.subject, &claims.device_id)
        .await?;
    Ok(ws
        .max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, state, claims.subject)))
}

async fn handle_socket(socket: WebSocket, state: AppState, subject: SessionSubject) {
    let connection_id = Uuid::new_v4();
    let (sender, mut receiver) = mpsc::channel(OUTBOUND_QUEUE);
    state
        .realtime
        .hub
        .register(connection_id, sender.clone(), subject);

    let hello = ServerMessage::Hello {
        connection_id,
        protocol: WS_PROTOCOL.to_owned(),
        user_id: subject.user_id,
        role: subject.role,
    };
    let _ = sender.try_send(hello);
    drop(sender);

    let (mut outbound, mut inbound) = socket.split();
    let (disconnect_tx, mut disconnect_rx) = tokio::sync::oneshot::channel::<()>();
    let send_task = tokio::spawn(async move {
        let mut heartbeat = interval(HEARTBEAT_INTERVAL);
        loop {
            tokio::select! {
                message = receiver.recv() => {
                    let Some(message) = message else { break };
                    let Ok(payload) = serde_json::to_string(&message) else { continue };
                    if outbound.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }
                _ = heartbeat.tick() => {
                    if outbound.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = disconnect_tx.send(());
    });

    let mut affected = Vec::new();
    loop {
        let incoming = tokio::select! {
            incoming = timeout(IDLE_TIMEOUT, inbound.next()) => match incoming {
                Ok(Some(Ok(message))) => message,
                Ok(Some(Err(_))) | Ok(None) | Err(_) => break,
            },
            _ = &mut disconnect_rx => break,
        };
        match incoming {
            Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                Ok(message) => {
                    process_message(&state, subject, connection_id, message).await;
                }
                Err(_) => {
                    send_error(
                        &state,
                        connection_id,
                        None,
                        "invalid_message",
                        "message does not match the twg.ws.v1 protocol",
                        false,
                    );
                }
            },
            Message::Binary(_) => {
                send_error(
                    &state,
                    connection_id,
                    None,
                    "unsupported",
                    "binary messages are not supported",
                    false,
                );
            }
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => break,
        }
    }
    affected.extend(state.realtime.hub.unregister(connection_id));
    send_task.abort();
    for skenario_id in affected {
        state.realtime.hub.broadcast_presence(skenario_id);
    }
}

async fn process_message(
    state: &AppState,
    subject: SessionSubject,
    connection_id: Uuid,
    message: ClientMessage,
) {
    match message {
        ClientMessage::Subscribe {
            request_id,
            skenario_id,
            last_sequence,
        } => {
            subscribe(
                state,
                subject,
                connection_id,
                request_id,
                skenario_id,
                last_sequence,
            )
            .await;
        }
        ClientMessage::Unsubscribe {
            request_id,
            skenario_id,
        } => {
            if state.realtime.hub.unsubscribe(connection_id, skenario_id) {
                let sequence = state.realtime.hub.current_sequence(skenario_id);
                send_ack(
                    state,
                    connection_id,
                    request_id,
                    skenario_id,
                    sequence,
                    false,
                );
                state.realtime.hub.broadcast_presence(skenario_id);
            } else {
                send_error(
                    state,
                    connection_id,
                    request_id,
                    "not_subscribed",
                    "connection is not subscribed to this scenario",
                    false,
                );
            }
        }
        ClientMessage::Publish {
            request_id,
            operasi_id,
            skenario_id,
            marker_id,
            command,
            data,
            is_save,
            idempotency_key,
        } => {
            publish(
                state,
                subject,
                connection_id,
                request_id,
                operasi_id,
                skenario_id,
                marker_id,
                command,
                data,
                is_save,
                idempotency_key,
            )
            .await;
        }
        ClientMessage::Resume {
            request_id,
            skenario_id,
            last_sequence,
        } => {
            subscribe(
                state,
                subject,
                connection_id,
                request_id,
                skenario_id,
                Some(last_sequence),
            )
            .await;
        }
        ClientMessage::Presence {
            request_id,
            skenario_id,
        } => {
            if authorize_scenario(
                state,
                subject,
                connection_id,
                request_id.clone(),
                skenario_id,
            )
            .await
            .is_some()
            {
                let (count, user_ids) = state.realtime.hub.presence(skenario_id);
                let _ = state.realtime.hub.send(
                    connection_id,
                    ServerMessage::Presence {
                        request_id,
                        skenario_id,
                        count,
                        user_ids,
                    },
                );
            }
        }
        ClientMessage::Play {
            request_id,
            skenario_id,
            action,
            data,
        } => {
            play(
                state,
                subject,
                connection_id,
                request_id,
                skenario_id,
                action,
                data,
            )
            .await;
        }
        ClientMessage::Ping { request_id } => {
            let _ = state
                .realtime
                .hub
                .send(connection_id, ServerMessage::Pong { request_id });
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn subscribe(
    state: &AppState,
    subject: SessionSubject,
    connection_id: Uuid,
    request_id: Option<String>,
    skenario_id: Uuid,
    last_sequence: Option<u64>,
) {
    let Some(_scenario) = authorize_scenario(
        state,
        subject,
        connection_id,
        request_id.clone(),
        skenario_id,
    )
    .await
    else {
        return;
    };

    state.realtime.hub.subscribe(connection_id, skenario_id);
    let sequence = state.realtime.hub.current_sequence(skenario_id);
    send_ack(
        state,
        connection_id,
        request_id.clone(),
        skenario_id,
        sequence,
        false,
    );
    state.realtime.hub.broadcast_presence(skenario_id);

    if let Some(last) = last_sequence {
        match state.realtime.hub.history_after(skenario_id, last) {
            Some(events) => {
                for event in events {
                    let _ = state.realtime.hub.send(connection_id, event);
                }
            }
            None => {
                let snapshot = match read_store(state) {
                    Ok(store) => store.snapshot(skenario_id).await.ok().flatten(),
                    Err(_) => None,
                };
                let _ = state.realtime.hub.send(
                    connection_id,
                    ServerMessage::Resync {
                        request_id,
                        skenario_id,
                        sequence,
                        reason: "history_gap".to_owned(),
                        snapshot,
                    },
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn publish(
    state: &AppState,
    subject: SessionSubject,
    connection_id: Uuid,
    request_id: Option<String>,
    operasi_id: Uuid,
    skenario_id: Uuid,
    marker_id: Uuid,
    command: String,
    data: String,
    is_save: i32,
    idempotency_key: Option<String>,
) {
    let Some(scenario) = authorize_scenario(
        state,
        subject,
        connection_id,
        request_id.clone(),
        skenario_id,
    )
    .await
    else {
        return;
    };
    if scenario.operasi.id != operasi_id {
        send_error(
            state,
            connection_id,
            request_id,
            "operasi_mismatch",
            "operasi_id does not match the scenario",
            false,
        );
        return;
    }
    let marker_scenario = match read_store(state) {
        Ok(store) => match store.marker_scenario_id(marker_id).await {
            Ok(value) => value,
            Err(error) => {
                tracing::error!(error = %error, "ws marker lookup failed");
                send_error(
                    state,
                    connection_id,
                    request_id,
                    "internal",
                    "internal server error",
                    true,
                );
                return;
            }
        },
        Err(_) => {
            send_error(
                state,
                connection_id,
                request_id,
                "unavailable",
                "database is not available",
                true,
            );
            return;
        }
    };
    match marker_scenario {
        None => {
            send_error(
                state,
                connection_id,
                request_id,
                "marker_not_found",
                "marker not found",
                false,
            );
            return;
        }
        Some(value) if value != skenario_id => {
            send_error(
                state,
                connection_id,
                request_id,
                "marker_mismatch",
                "marker does not belong to this scenario",
                false,
            );
            return;
        }
        Some(_) => {}
    }

    let command = command.trim().to_owned();
    if command.is_empty() {
        send_error(
            state,
            connection_id,
            request_id,
            "invalid_command",
            "command must not be empty",
            false,
        );
        return;
    }
    let is_save = if command.eq_ignore_ascii_case("reload") {
        0
    } else {
        is_save
    };

    if let Some(key) = &idempotency_key
        && let Some(sequence) = state.realtime.hub.idempotent_sequence(skenario_id, key)
    {
        send_ack(
            state,
            connection_id,
            request_id,
            skenario_id,
            sequence,
            true,
        );
        return;
    }

    if is_save == 1 {
        let Some(store) = state.write_store.as_deref() else {
            send_error(
                state,
                connection_id,
                request_id,
                "unavailable",
                "database is not available",
                true,
            );
            return;
        };
        if let Err(error) = store
            .save_command(is_save, operasi_id, skenario_id, marker_id, &command, &data)
            .await
        {
            tracing::error!(error = %error, "ws command persistence failed");
            send_error(
                state,
                connection_id,
                request_id,
                "persist_failed",
                "command could not be saved",
                true,
            );
            return;
        }
    }

    let sequence = state.realtime.hub.next_sequence(skenario_id);
    let event = ServerMessage::Event {
        sequence,
        operasi_id,
        skenario_id,
        marker_id,
        command,
        data,
        is_save,
        satker_id: scenario.satker_id,
    };
    state.realtime.hub.record_event(event.clone());
    state.realtime.hub.broadcast(skenario_id, event.clone());
    if let Some(key) = &idempotency_key {
        state
            .realtime
            .hub
            .remember_idempotency(skenario_id, key, sequence);
    }
    send_ack(
        state,
        connection_id,
        request_id,
        skenario_id,
        sequence,
        false,
    );
}

#[allow(clippy::too_many_arguments)]
async fn play(
    state: &AppState,
    subject: SessionSubject,
    connection_id: Uuid,
    request_id: Option<String>,
    skenario_id: Uuid,
    action: String,
    data: String,
) {
    if authorize_scenario(
        state,
        subject,
        connection_id,
        request_id.clone(),
        skenario_id,
    )
    .await
    .is_none()
    {
        return;
    }
    let action = action.trim().to_ascii_lowercase();
    let can_drive_big_screen = matches!(subject.role, Role::Admin | Role::AdminSatker);
    match action.as_str() {
        "join" => {
            if let Some(data) = state.realtime.hub.join_play(connection_id, skenario_id)
                && !data.is_empty()
            {
                let _ = state.realtime.hub.send(
                    connection_id,
                    ServerMessage::PlayEvent {
                        sequence: state.realtime.hub.current_sequence(skenario_id),
                        skenario_id,
                        actor: Uuid::nil(),
                        action: "state".to_owned(),
                        data,
                    },
                );
            }
            // Ask the clients already playing for a fresh snapshot. The new
            // joiner receives it through the normal state broadcast.
            let _ = state.realtime.hub.broadcast_play(
                skenario_id,
                Some(connection_id),
                ServerMessage::PlayEvent {
                    sequence: state.realtime.hub.current_sequence(skenario_id),
                    skenario_id,
                    actor: connection_id,
                    action: "sync".to_owned(),
                    data: String::new(),
                },
            );
        }
        "leave" => state.realtime.hub.leave_play(connection_id, skenario_id),
        "started" => {
            if !can_drive_big_screen {
                send_error(
                    state,
                    connection_id,
                    request_id,
                    "forbidden",
                    "only admin and admin_satker can start the big-screen session",
                    false,
                );
                return;
            }
            state.realtime.hub.start_live(skenario_id);
            if !data.is_empty() {
                state.realtime.hub.set_live_camera(skenario_id, data);
            }
        }
        "camera" => {
            if !can_drive_big_screen {
                send_error(
                    state,
                    connection_id,
                    request_id,
                    "forbidden",
                    "only admin and admin_satker can move the big-screen camera",
                    false,
                );
                return;
            }
            if data.is_empty() {
                send_error(
                    state,
                    connection_id,
                    request_id,
                    "invalid_payload",
                    "camera payload is required",
                    false,
                );
                return;
            }
            state.realtime.hub.set_live_camera(skenario_id, data);
        }
        "reload" => {
            if !can_drive_big_screen {
                send_error(
                    state,
                    connection_id,
                    request_id,
                    "forbidden",
                    "only admin and admin_satker can reload the big-screen session",
                    false,
                );
                return;
            }
            state.realtime.hub.reload_live(skenario_id);
        }
        "stop" => {
            if !can_drive_big_screen {
                send_error(
                    state,
                    connection_id,
                    request_id,
                    "forbidden",
                    "only admin and admin_satker can stop the big-screen session",
                    false,
                );
                return;
            }
            state.realtime.hub.clear_live(skenario_id);
        }
        "state" => {
            state.realtime.hub.set_play_state(skenario_id, data.clone());
            state.realtime.hub.set_live_state(skenario_id, data.clone());
            state.realtime.hub.broadcast_play(
                skenario_id,
                None,
                ServerMessage::PlayEvent {
                    sequence: state.realtime.hub.current_sequence(skenario_id),
                    skenario_id,
                    actor: connection_id,
                    action: "state".to_owned(),
                    data,
                },
            );
        }
        "sync" => {
            if let Some(data) = state.realtime.hub.play_state(skenario_id) {
                let _ = state.realtime.hub.send(
                    connection_id,
                    ServerMessage::PlayEvent {
                        sequence: state.realtime.hub.current_sequence(skenario_id),
                        skenario_id,
                        actor: Uuid::nil(),
                        action: "state".to_owned(),
                        data,
                    },
                );
            }
        }
        _ => send_error(
            state,
            connection_id,
            request_id,
            "invalid_action",
            "unknown play action",
            false,
        ),
    }
}

async fn authorize_scenario(
    state: &AppState,
    subject: SessionSubject,
    connection_id: Uuid,
    request_id: Option<String>,
    skenario_id: Uuid,
) -> Option<twg_contracts::scenarios::Scenario> {
    let store = match read_store(state) {
        Ok(store) => store,
        Err(_) => {
            send_error(
                state,
                connection_id,
                request_id,
                "unavailable",
                "database is not available",
                true,
            );
            return None;
        }
    };
    let scenario = match store.get_scenario(skenario_id).await {
        Ok(Some(scenario)) => scenario,
        Ok(None) => {
            send_error(
                state,
                connection_id,
                request_id,
                "not_found",
                "skenario not found",
                false,
            );
            return None;
        }
        Err(error) => {
            tracing::error!(error = %error, "ws scenario lookup failed");
            send_error(
                state,
                connection_id,
                request_id,
                "internal",
                "internal server error",
                true,
            );
            return None;
        }
    };
    if !subject.can_access_satker(scenario.satker_id) {
        send_error(
            state,
            connection_id,
            request_id,
            "forbidden",
            "cannot access this scenario",
            false,
        );
        return None;
    }
    Some(scenario)
}

fn send_ack(
    state: &AppState,
    connection_id: Uuid,
    request_id: Option<String>,
    skenario_id: Uuid,
    sequence: u64,
    duplicate: bool,
) {
    let _ = state.realtime.hub.send(
        connection_id,
        ServerMessage::Ack {
            request_id,
            skenario_id,
            sequence,
            duplicate,
        },
    );
}

fn send_error(
    state: &AppState,
    connection_id: Uuid,
    request_id: Option<String>,
    code: &str,
    message: &str,
    retryable: bool,
) {
    let _ = state.realtime.hub.send(
        connection_id,
        ServerMessage::Error {
            request_id,
            code: code.to_owned(),
            message: message.to_owned(),
            retryable,
        },
    );
}

/// Public read-only endpoint used by the big-screen page. It deliberately
/// carries no authentication and only exposes the live session started by an
/// admin on the authenticated WebSocket.
pub async fn public_live(
    State(state): State<AppState>,
) -> Json<SingleResponse<PublicLiveResponse>> {
    Json(SingleResponse::success(PublicLiveResponse {
        session: state.realtime.hub.live(),
    }))
}
