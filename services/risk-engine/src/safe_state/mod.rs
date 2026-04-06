#![cfg_attr(not(test), allow(dead_code))]

use domain::risk::{EmergencyControlAction, EmergencyControlTriggerSource};
use serde::Serialize;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DrawdownProtectiveModeSignal {
    pub active: bool,
    pub reason_code: String,
    pub correlation_id: String,
    pub triggered_at_utc: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EmergencySafeStateSignal {
    pub action: EmergencyControlAction,
    pub trigger_source: EmergencyControlTriggerSource,
    pub reason_code: String,
    pub correlation_id: String,
    pub triggered_at_utc: String,
}

pub trait RuntimeSafeStateSignalPort: Send + Sync {
    fn signal_drawdown_protective_mode(&self, signal: DrawdownProtectiveModeSignal);
    fn signal_emergency_safe_state(&self, signal: EmergencySafeStateSignal);
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySafeStateSignals {
    latest_drawdown_signal: Arc<RwLock<Option<DrawdownProtectiveModeSignal>>>,
    latest_emergency_signal: Arc<RwLock<Option<EmergencySafeStateSignal>>>,
}

impl InMemorySafeStateSignals {
    pub fn latest_drawdown_signal(&self) -> Option<DrawdownProtectiveModeSignal> {
        self.latest_drawdown_signal
            .read()
            .expect("safe-state drawdown signal should not be poisoned")
            .clone()
    }

    pub fn latest_emergency_signal(&self) -> Option<EmergencySafeStateSignal> {
        self.latest_emergency_signal
            .read()
            .expect("safe-state emergency signal should not be poisoned")
            .clone()
    }
}

impl RuntimeSafeStateSignalPort for InMemorySafeStateSignals {
    fn signal_drawdown_protective_mode(&self, signal: DrawdownProtectiveModeSignal) {
        *self
            .latest_drawdown_signal
            .write()
            .expect("safe-state drawdown signal should not be poisoned") = Some(signal.clone());
        emit_drawdown_safe_state_telemetry(&signal);
    }

    fn signal_emergency_safe_state(&self, signal: EmergencySafeStateSignal) {
        *self
            .latest_emergency_signal
            .write()
            .expect("safe-state emergency signal should not be poisoned") = Some(signal.clone());
        emit_emergency_safe_state_telemetry(&signal);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopSafeStateSignals;

impl RuntimeSafeStateSignalPort for NoopSafeStateSignals {
    fn signal_drawdown_protective_mode(&self, signal: DrawdownProtectiveModeSignal) {
        emit_drawdown_safe_state_telemetry(&signal);
    }

    fn signal_emergency_safe_state(&self, signal: EmergencySafeStateSignal) {
        emit_emergency_safe_state_telemetry(&signal);
    }
}

fn emit_drawdown_safe_state_telemetry(signal: &DrawdownProtectiveModeSignal) {
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

fn emit_emergency_safe_state_telemetry(signal: &EmergencySafeStateSignal) {
    let event = EmergencySafeStateTelemetryEvent {
        event_name: "risk_safe_state_emergency_signal_v1",
        action: signal.action.as_str(),
        trigger_source: signal.trigger_source.as_str(),
        outcome: "deny",
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

#[derive(Debug, Serialize)]
struct EmergencySafeStateTelemetryEvent<'a> {
    event_name: &'a str,
    action: &'a str,
    trigger_source: &'a str,
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

    #[test]
    fn in_memory_safe_state_signals_store_latest_emergency_transition() {
        let safe_state = InMemorySafeStateSignals::default();
        safe_state.signal_emergency_safe_state(EmergencySafeStateSignal {
            action: EmergencyControlAction::Pause,
            trigger_source: EmergencyControlTriggerSource::StaleFeed,
            reason_code: "emergency_control_stale_feed_triggered".to_string(),
            correlation_id: "corr-safe-state-emergency-001".to_string(),
            triggered_at_utc: "2026-04-06T00:00:00Z".to_string(),
        });

        let latest = safe_state
            .latest_emergency_signal()
            .expect("latest emergency signal should be retained in memory");
        assert_eq!(latest.action, EmergencyControlAction::Pause);
        assert_eq!(
            latest.trigger_source,
            EmergencyControlTriggerSource::StaleFeed
        );
        assert_eq!(latest.reason_code, "emergency_control_stale_feed_triggered");
    }
}
