use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

use tokio::sync::mpsc;
use twg_contracts::realtime::{LiveSession, ServerMessage};
use twg_domain::SessionSubject;
use uuid::Uuid;

const IDEMPOTENCY_LIMIT: usize = 1024;

struct ConnectionContext {
    sender: mpsc::Sender<ServerMessage>,
    subject: SessionSubject,
}

#[derive(Default)]
struct HubState {
    connections: HashMap<Uuid, ConnectionContext>,
    subscriptions: HashMap<Uuid, HashSet<Uuid>>,
    sequences: HashMap<Uuid, u64>,
    history: HashMap<Uuid, VecDeque<ServerMessage>>,
    idempotency: HashMap<(Uuid, String), u64>,
    idempotency_order: HashMap<Uuid, VecDeque<String>>,
    /// Latest play snapshot per scenario, published by a play-only client.
    play_state: HashMap<Uuid, String>,
    /// Connections currently playing the scenario from the Simulasi screen.
    play_connections: HashMap<Uuid, HashSet<Uuid>>,
    /// Public big-screen session driven by an admin/admin_satker client.
    live: Option<LiveSession>,
    /// Connections watching the big-screen session.
    live_watchers: HashSet<Uuid>,
    /// Monotonic counter for [`LiveSession::session_version`].
    live_sessions: u64,
}

/// In-memory realtime fan-out.
///
/// A connection has one bounded outbound queue. Broadcasting uses `try_send`;
/// a client that cannot keep up is disconnected instead of blocking the hub.
pub struct Hub {
    inner: Mutex<HubState>,
    history_limit: usize,
}

impl Hub {
    pub fn new(history_limit: usize) -> Self {
        Self {
            inner: Mutex::new(HubState::default()),
            history_limit: history_limit.max(16),
        }
    }

    pub fn register(
        &self,
        connection_id: Uuid,
        sender: mpsc::Sender<ServerMessage>,
        subject: SessionSubject,
    ) {
        let mut state = self.inner.lock().expect("hub poisoned");
        state
            .connections
            .insert(connection_id, ConnectionContext { sender, subject });
    }

    /// Removes a connection and returns the scenarios it was subscribed to.
    pub fn unregister(&self, connection_id: Uuid) -> Vec<Uuid> {
        let mut state = self.inner.lock().expect("hub poisoned");
        remove_connection(&mut state, connection_id)
    }

    pub fn subscribe(&self, connection_id: Uuid, skenario_id: Uuid) -> bool {
        let mut state = self.inner.lock().expect("hub poisoned");
        if !state.connections.contains_key(&connection_id) {
            return false;
        }
        state
            .subscriptions
            .entry(skenario_id)
            .or_default()
            .insert(connection_id)
    }

    pub fn unsubscribe(&self, connection_id: Uuid, skenario_id: Uuid) -> bool {
        let mut state = self.inner.lock().expect("hub poisoned");
        let Some(subscribers) = state.subscriptions.get_mut(&skenario_id) else {
            return false;
        };
        let removed = subscribers.remove(&connection_id);
        if subscribers.is_empty() {
            state.subscriptions.remove(&skenario_id);
        }
        removed
    }

    pub fn is_subscribed(&self, connection_id: Uuid, skenario_id: Uuid) -> bool {
        self.inner
            .lock()
            .expect("hub poisoned")
            .subscriptions
            .get(&skenario_id)
            .is_some_and(|subscribers| subscribers.contains(&connection_id))
    }

    /// Registers a play-only connection and returns the latest snapshot, if
    /// any client has published one for the current session.
    pub fn join_play(&self, connection_id: Uuid, skenario_id: Uuid) -> Option<String> {
        let mut state = self.inner.lock().expect("hub poisoned");
        if !state.connections.contains_key(&connection_id) {
            return None;
        }
        state
            .play_connections
            .entry(skenario_id)
            .or_default()
            .insert(connection_id);
        state.play_state.get(&skenario_id).cloned()
    }

    /// Removes a play-only connection. The last participant leaving ends the
    /// session, so the next one starts again from the authored state.
    pub fn leave_play(&self, connection_id: Uuid, skenario_id: Uuid) {
        let mut state = self.inner.lock().expect("hub poisoned");
        let Some(participants) = state.play_connections.get_mut(&skenario_id) else {
            return;
        };
        participants.remove(&connection_id);
        if participants.is_empty() {
            state.play_connections.remove(&skenario_id);
            state.play_state.remove(&skenario_id);
        }
    }

    pub fn play_state(&self, skenario_id: Uuid) -> Option<String> {
        self.inner
            .lock()
            .expect("hub poisoned")
            .play_state
            .get(&skenario_id)
            .cloned()
    }

    pub fn set_play_state(&self, skenario_id: Uuid, data: String) {
        self.inner
            .lock()
            .expect("hub poisoned")
            .play_state
            .insert(skenario_id, data);
    }

    /// Starts a fresh public big-screen session for `skenario_id`. Any
    /// previous session is replaced; the big screen reloads the scenario when
    /// the returned version changes.
    pub fn start_live(&self, skenario_id: Uuid) -> LiveSession {
        let mut state = self.inner.lock().expect("hub poisoned");
        state.live_sessions = state.live_sessions.saturating_add(1);
        let live = LiveSession {
            scenario_id: skenario_id,
            session_version: state.live_sessions,
            revision: 0,
            state: None,
            state_version: 0,
            camera: None,
            camera_version: 0,
        };
        state.live = Some(live.clone());
        drop(state);
        self.broadcast_live();
        live
    }

    /// Marks the loaded scenario as reloaded. Big screens reload the same
    /// scenario and wait for the next state snapshot.
    pub fn reload_live(&self, skenario_id: Uuid) {
        {
            let mut state = self.inner.lock().expect("hub poisoned");
            let Some(live) = state.live.as_mut() else {
                return;
            };
            if live.scenario_id != skenario_id {
                return;
            }
            live.revision = live.revision.saturating_add(1);
            live.state = None;
            live.state_version = live.state_version.saturating_add(1);
        }
        self.broadcast_live();
    }

    /// Stores the latest play snapshot for the big screen. Updates from a
    /// different scenario than the active live session are ignored.
    pub fn set_live_state(&self, skenario_id: Uuid, data: String) {
        {
            let mut state = self.inner.lock().expect("hub poisoned");
            let Some(live) = state.live.as_mut() else {
                return;
            };
            if live.scenario_id != skenario_id {
                return;
            }
            live.state = Some(data);
            live.state_version = live.state_version.saturating_add(1);
        }
        self.broadcast_live();
    }

    /// Stores a camera update published by an admin/admin_satker client.
    pub fn set_live_camera(&self, skenario_id: Uuid, data: String) {
        {
            let mut state = self.inner.lock().expect("hub poisoned");
            let Some(live) = state.live.as_mut() else {
                return;
            };
            if live.scenario_id != skenario_id {
                return;
            }
            live.camera = Some(data);
            live.camera_version = live.camera_version.saturating_add(1);
        }
        self.broadcast_live();
    }

    pub fn live(&self) -> Option<LiveSession> {
        self.inner.lock().expect("hub poisoned").live.clone()
    }

    /// Registers a connection as a big-screen watcher and sends the current
    /// snapshot on the same lock, so a concurrent broadcast cannot overtake
    /// the initial message. Returns false when the connection is gone or its
    /// outbound queue is full.
    pub fn watch_live(&self, connection_id: Uuid, request_id: Option<String>) -> bool {
        let mut state = self.inner.lock().expect("hub poisoned");
        let Some(connection) = state.connections.get(&connection_id) else {
            return false;
        };
        let sender = connection.sender.clone();
        if sender
            .try_send(ServerMessage::Live {
                request_id,
                session: state.live.clone(),
            })
            .is_err()
        {
            return false;
        }
        state.live_watchers.insert(connection_id);
        true
    }

    pub fn unwatch_live(&self, connection_id: Uuid) {
        self.inner
            .lock()
            .expect("hub poisoned")
            .live_watchers
            .remove(&connection_id);
    }

    /// Ends the big-screen session, e.g. when the simulating admin closes the
    /// play screen. Other scenarios are left untouched.
    pub fn clear_live(&self, skenario_id: Uuid) {
        {
            let mut state = self.inner.lock().expect("hub poisoned");
            if !state
                .live
                .as_ref()
                .is_some_and(|live| live.scenario_id == skenario_id)
            {
                return;
            }
            state.live = None;
        }
        self.broadcast_live();
    }

    fn broadcast_live(&self) {
        let (watchers, session) = {
            let state = self.inner.lock().expect("hub poisoned");
            if state.live_watchers.is_empty() {
                return;
            }
            (
                state.live_watchers.iter().copied().collect::<Vec<_>>(),
                state.live.clone(),
            )
        };
        self.send_to(
            watchers,
            ServerMessage::Live {
                request_id: None,
                session,
            },
        );
    }

    pub fn send(&self, connection_id: Uuid, message: ServerMessage) -> bool {
        let sender = {
            let state = self.inner.lock().expect("hub poisoned");
            state
                .connections
                .get(&connection_id)
                .map(|connection| connection.sender.clone())
        };
        sender.is_some_and(|sender| sender.try_send(message).is_ok())
    }

    pub fn broadcast(&self, skenario_id: Uuid, message: ServerMessage) -> usize {
        let recipients = {
            let state = self.inner.lock().expect("hub poisoned");
            state
                .subscriptions
                .get(&skenario_id)
                .map(|subscribers| subscribers.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default()
        };
        self.send_to(recipients, message)
    }

    /// Sends a play event only to the scenario's play-only connections.
    pub fn broadcast_play(
        &self,
        skenario_id: Uuid,
        except: Option<Uuid>,
        message: ServerMessage,
    ) -> usize {
        let recipients = {
            let state = self.inner.lock().expect("hub poisoned");
            state
                .play_connections
                .get(&skenario_id)
                .map(|connections| {
                    connections
                        .iter()
                        .copied()
                        .filter(|connection| Some(*connection) != except)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        self.send_to(recipients, message)
    }

    fn send_to(&self, recipients: Vec<Uuid>, message: ServerMessage) -> usize {
        let mut affected = Vec::new();
        let delivered = {
            let mut state = self.inner.lock().expect("hub poisoned");
            let mut slow = Vec::new();
            let mut delivered = 0;
            for connection_id in &recipients {
                let Some(connection) = state.connections.get(connection_id) else {
                    slow.push(*connection_id);
                    continue;
                };
                if connection.sender.try_send(message.clone()).is_ok() {
                    delivered += 1;
                } else {
                    slow.push(*connection_id);
                }
            }
            for connection_id in slow {
                affected.extend(remove_connection(&mut state, connection_id));
            }
            delivered
        };
        affected.sort_unstable();
        affected.dedup();
        for scenario in affected {
            self.broadcast_presence(scenario);
        }
        delivered
    }

    pub fn broadcast_presence(&self, skenario_id: Uuid) {
        let (count, user_ids) = self.presence(skenario_id);
        if count == 0 {
            return;
        }
        self.broadcast(
            skenario_id,
            ServerMessage::Presence {
                request_id: None,
                skenario_id,
                count,
                user_ids,
            },
        );
    }

    pub fn presence(&self, skenario_id: Uuid) -> (usize, Vec<Uuid>) {
        let state = self.inner.lock().expect("hub poisoned");
        let Some(subscribers) = state.subscriptions.get(&skenario_id) else {
            return (0, Vec::new());
        };
        let mut users = subscribers
            .iter()
            .filter_map(|connection_id| {
                state
                    .connections
                    .get(connection_id)
                    .map(|connection| connection.subject.user_id)
            })
            .collect::<Vec<_>>();
        users.sort_unstable();
        users.dedup();
        (subscribers.len(), users)
    }

    pub fn current_sequence(&self, skenario_id: Uuid) -> u64 {
        self.inner
            .lock()
            .expect("hub poisoned")
            .sequences
            .get(&skenario_id)
            .copied()
            .unwrap_or(0)
    }

    pub fn next_sequence(&self, skenario_id: Uuid) -> u64 {
        let mut state = self.inner.lock().expect("hub poisoned");
        let sequence = state.sequences.entry(skenario_id).or_insert(0);
        *sequence += 1;
        *sequence
    }

    pub fn record_event(&self, event: ServerMessage) {
        let Some(skenario_id) = event_scenario(&event) else {
            return;
        };
        let mut state = self.inner.lock().expect("hub poisoned");
        let history = state.history.entry(skenario_id).or_default();
        history.push_back(event);
        while history.len() > self.history_limit {
            history.pop_front();
        }
    }

    /// Returns `None` when the requested sequence is older than the retained
    /// history, which tells the client to resynchronize from a snapshot.
    pub fn history_after(&self, skenario_id: Uuid, after: u64) -> Option<Vec<ServerMessage>> {
        let state = self.inner.lock().expect("hub poisoned");
        let current = state.sequences.get(&skenario_id).copied().unwrap_or(0);
        if after > current {
            return None;
        }
        let Some(history) = state.history.get(&skenario_id) else {
            return if after == current {
                Some(Vec::new())
            } else {
                None
            };
        };
        let Some(oldest) = history.front().and_then(ServerMessage::sequence) else {
            return if after == current {
                Some(Vec::new())
            } else {
                None
            };
        };
        if after.saturating_add(1) < oldest {
            return None;
        }
        Some(
            history
                .iter()
                .filter(|event| event.sequence().is_some_and(|sequence| sequence > after))
                .cloned()
                .collect(),
        )
    }

    pub fn idempotent_sequence(&self, skenario_id: Uuid, key: &str) -> Option<u64> {
        self.inner
            .lock()
            .expect("hub poisoned")
            .idempotency
            .get(&(skenario_id, key.to_owned()))
            .copied()
    }

    pub fn remember_idempotency(&self, skenario_id: Uuid, key: &str, sequence: u64) {
        let mut state = self.inner.lock().expect("hub poisoned");
        let was_known = state
            .idempotency
            .contains_key(&(skenario_id, key.to_owned()));
        state
            .idempotency
            .insert((skenario_id, key.to_owned()), sequence);
        if was_known {
            return;
        }
        let mut evicted = Vec::new();
        {
            let order = state.idempotency_order.entry(skenario_id).or_default();
            order.push_back(key.to_owned());
            while order.len() > IDEMPOTENCY_LIMIT {
                if let Some(oldest) = order.pop_front() {
                    evicted.push(oldest);
                }
            }
        }
        for oldest in evicted {
            state.idempotency.remove(&(skenario_id, oldest));
        }
    }

    #[cfg(test)]
    pub fn connection_count(&self) -> usize {
        self.inner.lock().expect("hub poisoned").connections.len()
    }

    #[cfg(test)]
    pub fn subscriber_count(&self, skenario_id: Uuid) -> usize {
        self.inner
            .lock()
            .expect("hub poisoned")
            .subscriptions
            .get(&skenario_id)
            .map(HashSet::len)
            .unwrap_or(0)
    }
}

fn remove_connection(state: &mut HubState, connection_id: Uuid) -> Vec<Uuid> {
    state.connections.remove(&connection_id);
    state.live_watchers.remove(&connection_id);
    let mut affected = Vec::new();
    state.subscriptions.retain(|skenario_id, subscribers| {
        if subscribers.remove(&connection_id) {
            affected.push(*skenario_id);
        }
        !subscribers.is_empty()
    });
    let mut ended = Vec::new();
    state.play_connections.retain(|skenario_id, participants| {
        if participants.remove(&connection_id) && participants.is_empty() {
            ended.push(*skenario_id);
        }
        !participants.is_empty()
    });
    for skenario_id in ended {
        state.play_state.remove(&skenario_id);
    }
    affected
}

fn event_scenario(message: &ServerMessage) -> Option<Uuid> {
    match message {
        ServerMessage::Event { skenario_id, .. } => Some(*skenario_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;
    use twg_contracts::realtime::ServerMessage;
    use twg_domain::{Role, SessionSubject};
    use uuid::Uuid;

    use super::Hub;

    fn subject(user_id: u128) -> SessionSubject {
        SessionSubject {
            user_id: Uuid::from_u128(user_id),
            role: Role::Admin,
            satker_id: None,
        }
    }

    fn event(skenario_id: Uuid, sequence: u64) -> ServerMessage {
        ServerMessage::Event {
            sequence,
            operasi_id: Uuid::from_u128(9),
            skenario_id,
            marker_id: Uuid::from_u128(10),
            command: "MOVE".to_owned(),
            data: "{}".to_owned(),
            is_save: 0,
            satker_id: None,
        }
    }

    #[test]
    fn sequences_are_monotonic_per_scenario() {
        let hub = Hub::new(16);
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        assert_eq!(hub.next_sequence(first), 1);
        assert_eq!(hub.next_sequence(first), 2);
        assert_eq!(hub.next_sequence(second), 1);
        assert_eq!(hub.current_sequence(first), 2);
    }

    #[test]
    fn subscribe_broadcast_and_presence() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        let connection = Uuid::from_u128(2);
        let (sender, mut receiver) = mpsc::channel(4);
        hub.register(connection, sender, subject(3));

        assert!(hub.subscribe(connection, scenario));
        assert_eq!(hub.subscriber_count(scenario), 1);
        assert_eq!(hub.presence(scenario), (1, vec![Uuid::from_u128(3)]));

        assert_eq!(hub.broadcast(scenario, event(scenario, 1)), 1);
        let message = receiver.try_recv().expect("broadcast message");
        assert_eq!(message.sequence(), Some(1));
    }

    #[test]
    fn resume_within_history_returns_missing_events() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        for sequence in 1..=3 {
            hub.next_sequence(scenario);
            hub.record_event(event(scenario, sequence));
        }
        let events = hub.history_after(scenario, 1).expect("history");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence(), Some(2));
        assert!(hub.history_after(scenario, 3).expect("empty").is_empty());
    }

    #[test]
    fn resume_beyond_history_requires_resync() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        for sequence in 1..=20 {
            hub.next_sequence(scenario);
            hub.record_event(event(scenario, sequence));
        }
        assert!(hub.history_after(scenario, 0).is_none());
        assert!(hub.history_after(scenario, 19).is_some());
    }

    #[test]
    fn idempotency_remembers_last_sequence() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        assert_eq!(hub.idempotent_sequence(scenario, "k"), None);
        hub.remember_idempotency(scenario, "k", 7);
        assert_eq!(hub.idempotent_sequence(scenario, "k"), Some(7));
    }

    #[test]
    fn slow_consumer_is_disconnected() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        let slow = Uuid::from_u128(2);
        let (sender, _receiver) = mpsc::channel(1);
        hub.register(slow, sender, subject(3));
        hub.subscribe(slow, scenario);
        hub.next_sequence(scenario);

        assert_eq!(hub.broadcast(scenario, event(scenario, 1)), 1);
        assert_eq!(hub.broadcast(scenario, event(scenario, 1)), 0);
        assert_eq!(hub.connection_count(), 0);
        assert_eq!(hub.subscriber_count(scenario), 0);
    }

    #[test]
    fn unregister_cleans_subscriptions() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        let connection = Uuid::from_u128(2);
        let (sender, _receiver) = mpsc::channel(4);
        hub.register(connection, sender, subject(3));
        hub.subscribe(connection, scenario);
        assert_eq!(hub.unregister(connection), vec![scenario]);
        assert_eq!(hub.subscriber_count(scenario), 0);
    }

    fn play_event(scenario: Uuid, action: &str) -> ServerMessage {
        ServerMessage::PlayEvent {
            sequence: 0,
            skenario_id: scenario,
            actor: Uuid::from_u128(9),
            action: action.to_owned(),
            data: "{}".to_owned(),
        }
    }

    #[test]
    fn play_state_is_shared_only_with_play_connections() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        let player = Uuid::from_u128(2);
        let editor = Uuid::from_u128(3);
        let (player_sender, mut player_receiver) = mpsc::channel(4);
        hub.register(player, player_sender, subject(4));
        let (editor_sender, mut editor_receiver) = mpsc::channel(4);
        hub.register(editor, editor_sender, subject(5));
        hub.subscribe(editor, scenario);
        hub.subscribe(player, scenario);

        assert_eq!(hub.join_play(player, scenario), None);
        hub.set_play_state(scenario, "{\"units\":[]}".to_owned());
        assert_eq!(hub.play_state(scenario).as_deref(), Some("{\"units\":[]}"));
        assert_eq!(
            hub.broadcast_play(scenario, None, play_event(scenario, "state")),
            1
        );
        assert!(player_receiver.try_recv().is_ok());
        assert!(editor_receiver.try_recv().is_err());

        hub.leave_play(player, scenario);
        assert!(hub.play_state(scenario).is_none());
    }

    #[test]
    fn disconnecting_the_last_player_clears_the_play_state() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        let player = Uuid::from_u128(2);
        let (sender, _receiver) = mpsc::channel(4);
        hub.register(player, sender, subject(3));
        hub.join_play(player, scenario);
        hub.set_play_state(scenario, "{}".to_owned());

        hub.unregister(player);
        assert!(hub.play_state(scenario).is_none());
    }

    #[test]
    fn live_session_versions_are_monotonic_and_scoped_to_the_scenario() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        let other = Uuid::from_u128(2);

        let started = hub.start_live(scenario);
        assert_eq!(started.scenario_id, scenario);
        assert_eq!(started.session_version, 1);
        assert_eq!(started.state_version, 0);

        hub.set_live_state(scenario, "{\"units\":[]}".to_owned());
        assert_eq!(hub.live().unwrap().state_version, 1);
        hub.set_live_state(other, "ignored".to_owned());
        assert_eq!(hub.live().unwrap().state.as_deref(), Some("{\"units\":[]}"));
        assert_eq!(hub.live().unwrap().state_version, 1);

        hub.set_live_camera(scenario, "{\"zoom\":13}".to_owned());
        assert_eq!(hub.live().unwrap().camera_version, 1);
        hub.reload_live(scenario);
        let reloaded = hub.live().unwrap();
        assert_eq!(reloaded.revision, 1);
        assert!(reloaded.state.is_none());
        assert_eq!(reloaded.state_version, 2);

        let restarted = hub.start_live(scenario);
        assert_eq!(restarted.session_version, 2);
        assert_eq!(restarted.revision, 0);
        assert!(restarted.state.is_none());
        assert!(restarted.camera.is_none());

        hub.clear_live(other);
        assert!(hub.live().is_some());
        hub.clear_live(scenario);
        assert!(hub.live().is_none());
    }

    #[test]
    fn live_updates_are_pushed_to_watchers() {
        let hub = Hub::new(16);
        let scenario = Uuid::from_u128(1);
        let watcher = Uuid::from_u128(2);
        let (sender, mut receiver) = mpsc::channel(8);
        hub.register(watcher, sender, subject(3));
        assert!(hub.watch_live(watcher, None));
        assert!(matches!(
            receiver.try_recv().expect("initial live message"),
            ServerMessage::Live { session: None, .. }
        ));

        hub.start_live(scenario);
        assert!(matches!(
            receiver.try_recv().expect("started live message"),
            ServerMessage::Live {
                session: Some(_),
                ..
            }
        ));

        hub.set_live_state(scenario, "{}".to_owned());
        let message = receiver.try_recv().expect("state live message");
        assert!(matches!(
            message,
            ServerMessage::Live {
                session: Some(session),
                ..
            } if session.state.as_deref() == Some("{}")
        ));

        hub.set_live_camera(scenario, "{\"zoom\":13}".to_owned());
        let message = receiver.try_recv().expect("camera live message");
        assert!(matches!(
            message,
            ServerMessage::Live {
                session: Some(session),
                ..
            } if session.camera_version == 1
        ));

        hub.clear_live(scenario);
        assert!(matches!(
            receiver.try_recv().expect("stopped live message"),
            ServerMessage::Live { session: None, .. }
        ));

        hub.unwatch_live(watcher);
        hub.start_live(scenario);
        assert!(receiver.try_recv().is_err());
    }
}
