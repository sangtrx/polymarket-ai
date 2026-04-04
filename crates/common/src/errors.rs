use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MachineError {
    pub code: &'static str,
    pub message: String,
    pub timestamp_utc: String,
}
