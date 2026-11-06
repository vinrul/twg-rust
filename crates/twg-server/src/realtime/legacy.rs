use std::sync::Arc;

use serde_json::json;
use tracing::{debug, warn};
use twg_contracts::realtime::ServerMessage;

use crate::config::CentrifugoConfig;

/// Mirrors accepted events to the legacy Centrifugo channel so old React
/// clients keep working during the migration.
#[derive(Clone)]
pub struct LegacyPublisher {
    config: CentrifugoConfig,
    agent: Arc<ureq::Agent>,
}

impl LegacyPublisher {
    pub fn new(config: CentrifugoConfig) -> Self {
        Self {
            config,
            agent: Arc::new(ureq::Agent::config_builder().build().new_agent()),
        }
    }

    pub fn publish(&self, event: &ServerMessage) {
        let Ok(payload) = serde_json::to_value(event) else {
            return;
        };
        let config = self.config.clone();
        let agent = self.agent.clone();
        tokio::task::spawn_blocking(move || {
            let body = json!({
                "method": "publish",
                "params": {
                    "channel": config.channel,
                    "data": payload,
                }
            });
            match agent
                .post(&config.address)
                .header("X-API-Key", &config.key)
                .send_json(&body)
            {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!(channel = %config.channel, "legacy publish ok");
                    } else {
                        warn!(
                            channel = %config.channel,
                            status = %response.status(),
                            "legacy publish rejected"
                        );
                    }
                }
                Err(error) => {
                    warn!(channel = %config.channel, error = %error, "legacy publish failed");
                }
            }
        });
    }
}
