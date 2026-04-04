use std::time::{SystemTime, UNIX_EPOCH};

pub fn timestamp_utc() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{elapsed}")
}
