use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskLimit {
    pub policy_key: String,
    pub max_notional_usd: f64,
}
