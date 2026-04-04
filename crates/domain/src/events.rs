use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainEventEnvelope {
    pub event_id: String,
    pub occurred_at: String,
    pub event_name: String,
}
