use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalEnvelope {
    pub request_id: String,
    pub proposer_actor_id: String,
    pub approver_actor_id: Option<String>,
}
