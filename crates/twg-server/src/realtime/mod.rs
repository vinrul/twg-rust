use std::time::Duration;

mod hub;
mod ticket;

pub use hub::Hub;
pub use ticket::{TicketClaims, TicketStore};

/// Shared realtime state: ticket issuance, fan-out, and per-scenario history.
pub struct Realtime {
    pub tickets: TicketStore,
    pub hub: Hub,
}

impl Realtime {
    pub fn new(ticket_ttl: Duration, history_limit: usize) -> Self {
        Self {
            tickets: TicketStore::new(ticket_ttl),
            hub: Hub::new(history_limit),
        }
    }
}
