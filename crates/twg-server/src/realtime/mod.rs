use std::time::Duration;

use twg_contracts::realtime::ServerMessage;

use crate::config::CentrifugoConfig;

mod hub;
mod legacy;
mod ticket;

pub use hub::Hub;
pub use legacy::LegacyPublisher;
pub use ticket::{TicketClaims, TicketStore};

/// Shared realtime state: ticket issuance, fan-out, history, and optional
/// dual-publish to the legacy Centrifugo channel.
pub struct Realtime {
    pub tickets: TicketStore,
    pub hub: Hub,
    legacy: Option<LegacyPublisher>,
}

impl Realtime {
    pub fn new(
        ticket_ttl: Duration,
        history_limit: usize,
        centrifugo: Option<CentrifugoConfig>,
    ) -> Self {
        Self {
            tickets: TicketStore::new(ticket_ttl),
            hub: Hub::new(history_limit),
            legacy: centrifugo.map(LegacyPublisher::new),
        }
    }

    pub fn publish_legacy(&self, event: &ServerMessage) {
        if let Some(publisher) = &self.legacy {
            publisher.publish(event);
        }
    }
}
