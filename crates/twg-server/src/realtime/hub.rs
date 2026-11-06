use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;

use tokio::sync::mpsc;
use twg_contracts::realtime::ServerMessage;
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
        let mut affected = Vec::new();
        let delivered = {
            let mut state = self.inner.lock().expect("hub poisoned");
            let Some(subscribers) = state.subscriptions.get(&skenario_id) else {
                return 0;
            };
            let recipients = subscribers.iter().copied().collect::<Vec<_>>();
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
    let mut affected = Vec::new();
    state.subscriptions.retain(|skenario_id, subscribers| {
        if subscribers.remove(&connection_id) {
            affected.push(*skenario_id);
        }
        !subscribers.is_empty()
    });
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
}
