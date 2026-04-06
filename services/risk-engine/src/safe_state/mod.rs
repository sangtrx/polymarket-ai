#![cfg_attr(not(test), allow(dead_code))]

use serde::Serialize;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DrawdownProtectiveModeSignal {
    pub active: bool,
    pub reason_code: String,
    pub correlation_id: String,
    pub triggered_at_utc: String,
}

pub trait RuntimeSafeStateSignalPort: Send + Sync {
    fn signal_drawdown_protective_mode(&self, signal: DrawdownProtectiveModeSignal);
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySafeStateSignals {
    latest_drawdown_signal: Arc<RwLock<Option<DrawdownProtectiveModeSignal>>>,
}

impl InMemorySafeStateSignals {
    pub fn latest_drawdown_signal(&self) -> Option<DrawdownProtectiveModeSignal> {
        self.latest_drawdown_signal
            .read()
            .expect("safe-state drawdown signal should not be poisoned")
            .clone()
    }
}

impl RuntimeSafeStateSignalPort for InMemorySafeStateSignals {
    fn signal_drawdown_protective_mode(&self, signal: DrawdownProtectiveModeSignal) {
        *self
            .latest_drawdown_signal
            .write()
            .expect("safe-state drawdown signal should not be poisoned") = Some(signal.clone());
        emit_safe_state_telemetry(&signal);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopSafeStateSignals;

impl RuntimeSafeStateSignalPort for NoopSafeStateSignals {
    fn signal_drawdown_protective_mode(&self, signal: DrawdownProtectiveModeSignal) {
        emit_safe_state_telemetry(&signal);
    }
}

fn emit_safe_state_telemetry(signal: &DrawdownProtectiveModeSignal) {
    let event = SafeStateTelemetryEvent {
        event_name: "risk_safe_state_drawdown_protective_mode_v1",
        action: "drawdown_protective_mode_signal",
        outcome: if signal.active { "deny" } else { "allow" },
        reason_code: &signal.reason_code,
        correlation_id: &signal.correlation_id,
        timestamp_utc: &signal.triggered_at_utc,
    };
    println!(
        "{}",
        serde_json::to_string(&event).expect("safe-state telemetry should serialize")
    );
}

#[derive(Debug, Serialize)]
struct SafeStateTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    outcome: &'a str,
    reason_code: &'a str,
    correlation_id: &'a str,
    timestamp_utc: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_safe_state_signals_store_latest_drawdown_protective_transition() {
        let safe_state = InMemorySafeStateSignals::default();
        safe_state.signal_drawdown_protective_mode(DrawdownProtectiveModeSignal {
            active: true,
            reason_code: "pretrade_drawdown_stop_triggered".to_string(),
            correlation_id: "corr-safe-state-001".to_string(),
            triggered_at_utc: "2026-04-06T00:00:00Z".to_string(),
        });

        let latest = safe_state
            .latest_drawdown_signal()
            .expect("latest signal should be retained in memory");
        assert!(latest.active);
        assert_eq!(latest.reason_code, "pretrade_drawdown_stop_triggered");
    }
}
