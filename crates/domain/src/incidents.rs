use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncidentTimelineStage {
    Signal,
    Order,
    Fill,
    Pnl,
    RiskAction,
}

impl IncidentTimelineStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Signal => "signal",
            Self::Order => "order",
            Self::Fill => "fill",
            Self::Pnl => "pnl",
            Self::RiskAction => "risk_action",
        }
    }

    pub fn parse(value: &str) -> Result<Self, IncidentContractError> {
        match value {
            "signal" => Ok(Self::Signal),
            "order" => Ok(Self::Order),
            "fill" => Ok(Self::Fill),
            "pnl" => Ok(Self::Pnl),
            "risk_action" => Ok(Self::RiskAction),
            _ => Err(IncidentContractError::invalid_payload(format!(
                "unknown incident timeline stage `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncidentReasonCode {
    Ready,
    EmptyWindow,
    InvalidPayload,
    Unauthorized,
    DependencyUnavailable,
    StaleEvidence,
}

impl IncidentReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ready => "incident_ready",
            Self::EmptyWindow => "incident_empty_window",
            Self::InvalidPayload => "incident_invalid_payload",
            Self::Unauthorized => "incident_unauthorized",
            Self::DependencyUnavailable => "incident_dependency_unavailable",
            Self::StaleEvidence => "incident_stale_evidence",
        }
    }

    pub fn parse(value: &str) -> Result<Self, IncidentContractError> {
        match value {
            "incident_ready" => Ok(Self::Ready),
            "incident_empty_window" => Ok(Self::EmptyWindow),
            "incident_invalid_payload" => Ok(Self::InvalidPayload),
            "incident_unauthorized" => Ok(Self::Unauthorized),
            "incident_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "incident_stale_evidence" => Ok(Self::StaleEvidence),
            _ => Err(IncidentContractError::invalid_payload(format!(
                "unknown incident reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<IncidentValidationIssue>,
}

impl IncidentContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: IncidentReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<IncidentValidationIssue>,
    ) -> Self {
        Self {
            code: IncidentReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: IncidentReasonCode::Unauthorized.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: IncidentReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn stale_evidence(message: impl Into<String>) -> Self {
        Self {
            code: IncidentReasonCode::StaleEvidence.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentQueryFilters {
    pub market_id: Option<String>,
    pub order_id: Option<String>,
    pub alpha_id: Option<String>,
    pub actor_id: Option<String>,
    pub start_ts: String,
    pub end_ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentTimelineEvent {
    pub event_id: String,
    pub occurred_at: String,
    pub stage: IncidentTimelineStage,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub summary: String,
    pub recommended_next_action: String,
    pub severity: String,
    pub market_id: Option<String>,
    pub order_id: Option<String>,
    pub alpha_id: Option<String>,
    pub actor_id: Option<String>,
    pub run_id: Option<String>,
    pub snapshot_id: Option<String>,
}

pub fn normalize_incident_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn build_incident_query_filters(
    market_id: Option<&str>,
    order_id: Option<&str>,
    alpha_id: Option<&str>,
    actor_id: Option<&str>,
    start_ts: Option<&str>,
    end_ts: Option<&str>,
    now_utc: &str,
) -> Result<IncidentQueryFilters, IncidentContractError> {
    let now = parse_timestamp_or_error("now_utc", now_utc)?;
    let (start, end) = match (
        start_ts.map(str::trim).filter(|value| !value.is_empty()),
        end_ts.map(str::trim).filter(|value| !value.is_empty()),
    ) {
        (None, None) => (now - Duration::hours(24), now),
        (Some(start), Some(end)) => (
            parse_timestamp_or_error("start_ts", start)?,
            parse_timestamp_or_error("end_ts", end)?,
        ),
        (Some(_), None) | (None, Some(_)) => {
            return Err(IncidentContractError::invalid_payload_with_issues(
                "start_ts and end_ts must be provided together",
                vec![
                    IncidentValidationIssue {
                        field: "start_ts",
                        code: IncidentReasonCode::InvalidPayload.code(),
                        message: "start_ts and end_ts must be provided together".to_string(),
                    },
                    IncidentValidationIssue {
                        field: "end_ts",
                        code: IncidentReasonCode::InvalidPayload.code(),
                        message: "start_ts and end_ts must be provided together".to_string(),
                    },
                ],
            ));
        }
    };
    if end <= start {
        return Err(IncidentContractError::invalid_payload_with_issues(
            "end_ts must be greater than start_ts",
            vec![IncidentValidationIssue {
                field: "end_ts",
                code: IncidentReasonCode::InvalidPayload.code(),
                message: "end_ts must be greater than start_ts".to_string(),
            }],
        ));
    }

    Ok(IncidentQueryFilters {
        market_id: normalize_optional_identifier("market_id", market_id)?,
        order_id: normalize_optional_identifier("order_id", order_id)?,
        alpha_id: normalize_optional_identifier("alpha_id", alpha_id)?,
        actor_id: normalize_optional_identifier("actor_id", actor_id)?,
        start_ts: format_timestamp(start),
        end_ts: format_timestamp(end),
    })
}

pub fn validate_incident_timeline_event(
    event: &IncidentTimelineEvent,
) -> Result<(), IncidentContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty(&mut field_errors, "event_id", &event.event_id);
    validate_non_empty(&mut field_errors, "source", &event.source);
    validate_non_empty(&mut field_errors, "reason_code", &event.reason_code);
    validate_non_empty(&mut field_errors, "correlation_id", &event.correlation_id);
    validate_non_empty(&mut field_errors, "summary", &event.summary);
    validate_non_empty(
        &mut field_errors,
        "recommended_next_action",
        &event.recommended_next_action,
    );
    validate_timestamp_utc(&mut field_errors, "occurred_at", &event.occurred_at);
    validate_canonical_identifier(&mut field_errors, "event_id", &event.event_id);

    if let Some(market_id) = event.market_id.as_deref() {
        validate_canonical_identifier(&mut field_errors, "market_id", market_id);
    }
    if let Some(order_id) = event.order_id.as_deref() {
        validate_canonical_identifier(&mut field_errors, "order_id", order_id);
    }
    if let Some(alpha_id) = event.alpha_id.as_deref() {
        validate_canonical_identifier(&mut field_errors, "alpha_id", alpha_id);
    }
    if let Some(actor_id) = event.actor_id.as_deref() {
        validate_canonical_identifier(&mut field_errors, "actor_id", actor_id);
    }
    if let Some(run_id) = event.run_id.as_deref() {
        validate_canonical_identifier(&mut field_errors, "run_id", run_id);
    }
    if let Some(snapshot_id) = event.snapshot_id.as_deref() {
        validate_canonical_identifier(&mut field_errors, "snapshot_id", snapshot_id);
    }

    if !matches!(
        event.severity.as_str(),
        "normal" | "warning" | "critical" | "degraded"
    ) {
        field_errors.push(IncidentValidationIssue {
            field: "severity",
            code: IncidentReasonCode::InvalidPayload.code(),
            message: "severity must be one of: normal, warning, critical, degraded".to_string(),
        });
    }

    if field_errors.is_empty() {
        return Ok(());
    }

    Err(IncidentContractError::invalid_payload_with_issues(
        "incident timeline event failed validation",
        field_errors,
    ))
}

pub fn apply_incident_query(
    events: &[IncidentTimelineEvent],
    filters: &IncidentQueryFilters,
) -> Result<Vec<IncidentTimelineEvent>, IncidentContractError> {
    let start = parse_timestamp_or_error("start_ts", &filters.start_ts)?;
    let end = parse_timestamp_or_error("end_ts", &filters.end_ts)?;
    if end <= start {
        return Err(IncidentContractError::invalid_payload(
            "end_ts must be greater than start_ts",
        ));
    }

    let mut matched = Vec::new();
    for event in events {
        validate_incident_timeline_event(event)?;
        let occurred_at = parse_timestamp_or_error("occurred_at", &event.occurred_at)?;
        if occurred_at < start || occurred_at >= end {
            continue;
        }
        if filters
            .market_id
            .as_deref()
            .is_some_and(|value| event.market_id.as_deref() != Some(value))
        {
            continue;
        }
        if filters
            .order_id
            .as_deref()
            .is_some_and(|value| event.order_id.as_deref() != Some(value))
        {
            continue;
        }
        if filters
            .alpha_id
            .as_deref()
            .is_some_and(|value| event.alpha_id.as_deref() != Some(value))
        {
            continue;
        }
        if filters
            .actor_id
            .as_deref()
            .is_some_and(|value| event.actor_id.as_deref() != Some(value))
        {
            continue;
        }
        matched.push(event.clone());
    }

    Ok(order_incident_timeline_events(matched))
}

pub fn order_incident_timeline_events(
    mut events: Vec<IncidentTimelineEvent>,
) -> Vec<IncidentTimelineEvent> {
    events.sort_by(|left, right| {
        let left_time = parse_timestamp(&left.occurred_at).ok();
        let right_time = parse_timestamp(&right.occurred_at).ok();
        match (left_time, right_time) {
            (Some(left_time), Some(right_time)) => right_time
                .cmp(&left_time)
                .then_with(|| left.event_id.cmp(&right.event_id)),
            _ => right
                .occurred_at
                .cmp(&left.occurred_at)
                .then_with(|| left.event_id.cmp(&right.event_id)),
        }
    });
    events
}

fn normalize_optional_identifier(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, IncidentContractError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let normalized = normalize_incident_identifier(raw);
    if normalized.is_empty() {
        return Err(IncidentContractError::invalid_payload_with_issues(
            format!("{field} cannot be blank"),
            vec![IncidentValidationIssue {
                field,
                code: IncidentReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }
    if !is_canonical_identifier(&normalized) {
        return Err(IncidentContractError::invalid_payload_with_issues(
            format!("{field} must contain 3-120 canonical characters"),
            vec![IncidentValidationIssue {
                field,
                code: IncidentReasonCode::InvalidPayload.code(),
                message: format!("{field} must contain 3-120 canonical characters"),
            }],
        ));
    }
    Ok(Some(normalized))
}

fn is_canonical_identifier(value: &str) -> bool {
    let normalized = normalize_incident_identifier(value);
    (3..=120).contains(&normalized.len())
        && normalized.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || "._:-".contains(character)
        })
}

fn validate_non_empty(
    field_errors: &mut Vec<IncidentValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(IncidentValidationIssue {
            field,
            code: IncidentReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_canonical_identifier(
    field_errors: &mut Vec<IncidentValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if !is_canonical_identifier(value) {
        field_errors.push(IncidentValidationIssue {
            field,
            code: IncidentReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain 3-120 canonical characters"),
        });
    }
}

fn validate_timestamp_utc(
    field_errors: &mut Vec<IncidentValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_timestamp(value).is_err() {
        field_errors.push(IncidentValidationIssue {
            field,
            code: IncidentReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, ()> {
    let parsed = OffsetDateTime::parse(value.trim(), &Rfc3339).map_err(|_| ())?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(());
    }
    Ok(parsed)
}

fn parse_timestamp_or_error(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, IncidentContractError> {
    parse_timestamp(value).map_err(|_| {
        IncidentContractError::invalid_payload_with_issues(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![IncidentValidationIssue {
                field,
                code: IncidentReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .expect("RFC3339 formatting for UTC timestamp must succeed")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event(
        event_id: &str,
        occurred_at: &str,
        stage: IncidentTimelineStage,
    ) -> IncidentTimelineEvent {
        IncidentTimelineEvent {
            event_id: event_id.to_string(),
            occurred_at: occurred_at.to_string(),
            stage,
            source: "incident.forensics.v1".to_string(),
            reason_code: "reconciliation_non_critical_mismatch".to_string(),
            correlation_id: "corr-incident-001".to_string(),
            summary: "Detected non-critical mismatch in incident chain".to_string(),
            recommended_next_action:
                "Inspect reconciliation mismatch and verify order lifecycle evidence".to_string(),
            severity: "warning".to_string(),
            market_id: Some("market-btc-election".to_string()),
            order_id: Some("order-42".to_string()),
            alpha_id: Some("alpha-momentum".to_string()),
            actor_id: Some("ops-1".to_string()),
            run_id: Some("run-001".to_string()),
            snapshot_id: Some("snapshot-001".to_string()),
        }
    }

    #[test]
    fn incident_reason_code_parsing_supports_canonical_values() {
        assert_eq!(
            IncidentReasonCode::parse("incident_ready").expect("incident_ready should parse"),
            IncidentReasonCode::Ready
        );
        assert_eq!(
            IncidentReasonCode::parse("incident_empty_window")
                .expect("incident_empty_window should parse"),
            IncidentReasonCode::EmptyWindow
        );
        assert_eq!(
            IncidentReasonCode::parse("incident_dependency_unavailable")
                .expect("incident_dependency_unavailable should parse"),
            IncidentReasonCode::DependencyUnavailable
        );
        assert_eq!(
            IncidentReasonCode::parse("incident_unknown")
                .expect_err("unsupported reason should fail")
                .code,
            IncidentReasonCode::InvalidPayload.code()
        );
    }

    #[test]
    fn incident_query_defaults_to_last_24h_window() {
        let filters = build_incident_query_filters(
            None,
            None,
            None,
            None,
            None,
            None,
            "2026-04-06T15:00:00Z",
        )
        .expect("defaults should resolve");

        assert_eq!(filters.start_ts, "2026-04-05T15:00:00Z");
        assert_eq!(filters.end_ts, "2026-04-06T15:00:00Z");
    }

    #[test]
    fn incident_query_rejects_half_open_time_windows() {
        let error = build_incident_query_filters(
            None,
            None,
            None,
            None,
            Some("2026-04-06T14:00:00Z"),
            None,
            "2026-04-06T15:00:00Z",
        )
        .expect_err("half-open window should fail");

        assert_eq!(error.code, IncidentReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "start_ts")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "end_ts")
        );
    }

    #[test]
    fn timeline_query_is_start_inclusive_end_exclusive() {
        let filters = build_incident_query_filters(
            Some("market-btc-election"),
            None,
            None,
            None,
            Some("2026-04-06T14:00:00Z"),
            Some("2026-04-06T15:00:00Z"),
            "2026-04-06T15:00:00Z",
        )
        .expect("filters should resolve");

        let events = vec![
            sample_event(
                "event-start",
                "2026-04-06T14:00:00Z",
                IncidentTimelineStage::Signal,
            ),
            sample_event(
                "event-end",
                "2026-04-06T15:00:00Z",
                IncidentTimelineStage::Order,
            ),
        ];
        let result = apply_incident_query(&events, &filters).expect("query should succeed");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].event_id, "event-start");
    }

    #[test]
    fn timeline_ordering_is_deterministic_with_tie_break_event_id() {
        let ordered = order_incident_timeline_events(vec![
            sample_event(
                "event-b",
                "2026-04-06T14:30:00Z",
                IncidentTimelineStage::Order,
            ),
            sample_event(
                "event-a",
                "2026-04-06T14:30:00Z",
                IncidentTimelineStage::Signal,
            ),
            sample_event(
                "event-c",
                "2026-04-06T14:31:00Z",
                IncidentTimelineStage::Fill,
            ),
        ]);

        assert_eq!(ordered[0].event_id, "event-c");
        assert_eq!(ordered[1].event_id, "event-a");
        assert_eq!(ordered[2].event_id, "event-b");
    }

    #[test]
    fn timeline_ordering_uses_timestamp_semantics_for_mixed_precision() {
        let ordered = order_incident_timeline_events(vec![
            sample_event(
                "event-zero-ms",
                "2026-04-06T14:30:00Z",
                IncidentTimelineStage::Signal,
            ),
            sample_event(
                "event-with-ms",
                "2026-04-06T14:30:00.100Z",
                IncidentTimelineStage::Order,
            ),
        ]);

        assert_eq!(ordered[0].event_id, "event-with-ms");
        assert_eq!(ordered[1].event_id, "event-zero-ms");
    }

    #[test]
    fn timeline_query_returns_explicit_empty_when_window_has_no_matches() {
        let filters = build_incident_query_filters(
            Some("market-no-activity"),
            None,
            None,
            None,
            Some("2026-04-06T14:00:00Z"),
            Some("2026-04-06T15:00:00Z"),
            "2026-04-06T15:00:00Z",
        )
        .expect("filters should resolve");
        let result = apply_incident_query(
            &[sample_event(
                "event-a",
                "2026-04-06T14:30:00Z",
                IncidentTimelineStage::Signal,
            )],
            &filters,
        )
        .expect("query should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn timeline_event_validation_rejects_invalid_severity() {
        let mut event = sample_event(
            "event-a",
            "2026-04-06T14:30:00Z",
            IncidentTimelineStage::Signal,
        );
        event.severity = "urgent".to_string();
        let error = validate_incident_timeline_event(&event).expect_err("severity should fail");
        assert_eq!(error.code, IncidentReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "severity")
        );
    }
}
