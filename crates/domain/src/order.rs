use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderKey {
    pub order_id: String,
    pub market_id: String,
}
