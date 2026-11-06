use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use twg_domain::SessionSubject;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TicketClaims {
    pub subject: SessionSubject,
    pub device_id: String,
}

struct TicketEntry {
    claims: TicketClaims,
    expires_at: Instant,
}

/// Short-lived, single-use tickets for WebSocket upgrades.
///
/// Tickets are kept in memory so they are consumed exactly once. Deployments
/// with more than one server instance need a shared store (or a signed ticket)
/// before horizontal scaling.
pub struct TicketStore {
    tickets: Mutex<HashMap<String, TicketEntry>>,
    ttl: Duration,
}

impl TicketStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            tickets: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn issue(&self, subject: SessionSubject, device_id: &str) -> (String, Duration) {
        let ticket = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let now = Instant::now();
        let mut tickets = self.tickets.lock().expect("ticket store poisoned");
        tickets.retain(|_, entry| entry.expires_at > now);
        tickets.insert(
            ticket.clone(),
            TicketEntry {
                claims: TicketClaims {
                    subject,
                    device_id: device_id.to_owned(),
                },
                expires_at: now + self.ttl,
            },
        );
        (ticket, self.ttl)
    }

    /// Consumes a ticket. A second call with the same ticket always returns
    /// `None`, even if the first WebSocket upgrade failed afterwards.
    pub fn consume(&self, ticket: &str) -> Option<TicketClaims> {
        let now = Instant::now();
        let mut tickets = self.tickets.lock().expect("ticket store poisoned");
        tickets.retain(|_, entry| entry.expires_at > now);
        let entry = tickets.remove(ticket)?;
        if entry.expires_at <= now {
            return None;
        }
        Some(entry.claims)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.tickets.lock().expect("ticket store poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use twg_domain::{Role, SessionSubject};
    use uuid::Uuid;

    use super::TicketStore;

    fn subject() -> SessionSubject {
        SessionSubject {
            user_id: Uuid::from_u128(1),
            role: Role::Admin,
            satker_id: None,
        }
    }

    #[test]
    fn ticket_is_single_use() {
        let store = TicketStore::new(Duration::from_secs(30));
        let (ticket, ttl) = store.issue(subject(), "device-1");
        assert_eq!(ttl, Duration::from_secs(30));
        assert_eq!(store.len(), 1);

        let claims = store.consume(&ticket).expect("first consume");
        assert_eq!(claims.subject.user_id, subject().user_id);
        assert_eq!(claims.device_id, "device-1");
        assert!(store.consume(&ticket).is_none());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn unknown_ticket_is_rejected() {
        let store = TicketStore::new(Duration::from_secs(30));
        assert!(store.consume("nope").is_none());
    }

    #[test]
    fn expired_ticket_is_rejected() {
        let store = TicketStore::new(Duration::from_millis(1));
        let (ticket, _) = store.issue(subject(), "device-1");
        std::thread::sleep(Duration::from_millis(5));
        assert!(store.consume(&ticket).is_none());
    }
}
