use crate::governance::{GovernancePermission, GovernanceRole, RolePermissionMatrix};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub const DEFAULT_REPORTING_LIMIT: i64 = 200;
pub const MAX_REPORTING_LIMIT: i64 = 500;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportingReasonCode {
    Ready,
    EmptyWindow,
    InvalidPayload,
    Unauthorized,
    DependencyUnavailable,
    StaleDependency,
    PersistenceUnavailable,
    EvidenceUnavailable,
}

impl ReportingReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ready => "reporting_ready",
            Self::EmptyWindow => "reporting_empty_window",
            Self::InvalidPayload => "reporting_invalid_payload",
            Self::Unauthorized => "reporting_unauthorized",
            Self::DependencyUnavailable => "reporting_dependency_unavailable",
            Self::StaleDependency => "reporting_stale_dependency",
            Self::PersistenceUnavailable => "reporting_persistence_unavailable",
            Self::EvidenceUnavailable => "reporting_evidence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReportingContractError> {
        match value {
            "reporting_ready" => Ok(Self::Ready),
            "reporting_empty_window" => Ok(Self::EmptyWindow),
            "reporting_invalid_payload" => Ok(Self::InvalidPayload),
            "reporting_unauthorized" => Ok(Self::Unauthorized),
            "reporting_dependency_unavailable" => Ok(Self::DependencyUnavailable),
            "reporting_stale_dependency" => Ok(Self::StaleDependency),
            "reporting_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "reporting_evidence_unavailable" => Ok(Self::EvidenceUnavailable),
            _ => Err(ReportingContractError::invalid_payload(format!(
                "unknown reporting reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReportingValidationIssue>,
}

impl ReportingContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<ReportingValidationIssue>,
    ) -> Self {
        Self {
            code: ReportingReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::Unauthorized.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn dependency_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::DependencyUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn stale_dependency(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::StaleDependency.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn persistence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::PersistenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn evidence_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReportingReasonCode::EvidenceUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingReadQuery {
    pub start_inclusive_utc: String,
    pub end_exclusive_utc: String,
    pub market_id: Option<String>,
    pub alpha_id: Option<String>,
    pub correlation_id: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportingEvidenceMetadata {
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub run_id: Option<String>,
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TradeReportingRow {
    pub report_row_id: String,
    pub order_id: String,
    pub trade_id: String,
    pub market_id: String,
    pub asset_id: String,
    pub lifecycle_state: String,
    pub event_status: String,
    pub occurred_at_utc: String,
    pub evidence: ReportingEvidenceMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionReportingRow {
    pub report_row_id: String,
    pub market_id: String,
    pub net_exposure: f64,
    pub gross_exposure: f64,
    pub open_order_count: i64,
    pub run_status: String,
    pub window_started_at_utc: String,
    pub window_ended_at_utc: String,
    pub evidence: ReportingEvidenceMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RiskEventReportingRow {
    pub report_row_id: String,
    pub event_type: String,
    pub severity: String,
    pub outcome: String,
    pub market_id: Option<String>,
    pub event_at_utc: String,
    pub evidence: ReportingEvidenceMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceReportingRow {
    pub report_row_id: String,
    pub market_id: String,
    pub alpha_id: String,
    pub period_scope: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub realized_pnl_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub gross_pnl_usd: f64,
    pub net_pnl_usd: f64,
    pub fees_usd: f64,
    pub rebates_usd: f64,
    pub incentives_usd: f64,
    pub evidence: ReportingEvidenceMetadata,
}

pub fn normalize_reporting_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn parse_utc_timestamp(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, ReportingContractError> {
    parse_timestamp(value).map_err(|_| {
        ReportingContractError::invalid_payload_with_issues(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            }],
        )
    })
}

pub fn format_utc_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .to_offset(UtcOffset::UTC)
        .format(&Rfc3339)
        .expect("RFC3339 formatting for UTC timestamp must succeed")
}

pub fn build_reporting_read_query(
    start_inclusive_utc: &str,
    end_exclusive_utc: &str,
    market_id: Option<&str>,
    alpha_id: Option<&str>,
    correlation_id: Option<&str>,
    limit: i64,
) -> Result<ReportingReadQuery, ReportingContractError> {
    let query = ReportingReadQuery {
        start_inclusive_utc: start_inclusive_utc.trim().to_string(),
        end_exclusive_utc: end_exclusive_utc.trim().to_string(),
        market_id: normalize_optional_identifier("market_id", market_id)?,
        alpha_id: normalize_optional_identifier("alpha_id", alpha_id)?,
        correlation_id: normalize_optional_identifier("correlation_id", correlation_id)?,
        limit,
    };
    validate_reporting_read_query(&query)?;
    Ok(query)
}

pub fn validate_reporting_read_query(
    query: &ReportingReadQuery,
) -> Result<(), ReportingContractError> {
    let mut field_errors = Vec::new();
    let start = parse_timestamp_with_issues(
        &mut field_errors,
        "start_inclusive_utc",
        &query.start_inclusive_utc,
    );
    let end = parse_timestamp_with_issues(
        &mut field_errors,
        "end_exclusive_utc",
        &query.end_exclusive_utc,
    );
    if let (Some(start), Some(end)) = (start, end)
        && end <= start
    {
        field_errors.push(ReportingValidationIssue {
            field: "end_exclusive_utc",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: "end_exclusive_utc must be greater than start_inclusive_utc".to_string(),
        });
    }
    if !(1..=MAX_REPORTING_LIMIT).contains(&query.limit) {
        field_errors.push(ReportingValidationIssue {
            field: "limit",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: format!("limit must be between 1 and {MAX_REPORTING_LIMIT}"),
        });
    }

    for (field, value) in [
        ("market_id", query.market_id.as_deref()),
        ("alpha_id", query.alpha_id.as_deref()),
        ("correlation_id", query.correlation_id.as_deref()),
    ] {
        if let Some(value) = value {
            validate_canonical_identifier_field(&mut field_errors, field, value);
        }
    }

    if field_errors.is_empty() {
        return Ok(());
    }
    Err(ReportingContractError::invalid_payload_with_issues(
        "reporting query filters failed validation",
        field_errors,
    ))
}

pub fn validate_trade_reporting_row(row: &TradeReportingRow) -> Result<(), ReportingContractError> {
    let mut field_errors = Vec::new();
    validate_canonical_identifier_field(&mut field_errors, "report_row_id", &row.report_row_id);
    validate_canonical_identifier_field(&mut field_errors, "order_id", &row.order_id);
    validate_canonical_identifier_field(&mut field_errors, "trade_id", &row.trade_id);
    validate_canonical_identifier_field(&mut field_errors, "market_id", &row.market_id);
    validate_canonical_identifier_field(&mut field_errors, "asset_id", &row.asset_id);
    validate_non_empty_field(&mut field_errors, "lifecycle_state", &row.lifecycle_state);
    validate_non_empty_field(&mut field_errors, "event_status", &row.event_status);
    validate_timestamp_field(&mut field_errors, "occurred_at_utc", &row.occurred_at_utc);
    validate_reporting_evidence_metadata(&mut field_errors, &row.evidence);

    if field_errors.is_empty() {
        return Ok(());
    }
    Err(ReportingContractError::invalid_payload_with_issues(
        "trade reporting row failed validation",
        field_errors,
    ))
}

pub fn validate_position_reporting_row(
    row: &PositionReportingRow,
) -> Result<(), ReportingContractError> {
    let mut field_errors = Vec::new();
    validate_canonical_identifier_field(&mut field_errors, "report_row_id", &row.report_row_id);
    validate_canonical_identifier_field(&mut field_errors, "market_id", &row.market_id);
    validate_non_empty_field(&mut field_errors, "run_status", &row.run_status);
    validate_timestamp_field(
        &mut field_errors,
        "window_started_at_utc",
        &row.window_started_at_utc,
    );
    validate_timestamp_field(
        &mut field_errors,
        "window_ended_at_utc",
        &row.window_ended_at_utc,
    );
    validate_finite_number(&mut field_errors, "net_exposure", row.net_exposure);
    validate_finite_number(&mut field_errors, "gross_exposure", row.gross_exposure);
    if row.open_order_count < 0 {
        field_errors.push(ReportingValidationIssue {
            field: "open_order_count",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: "open_order_count cannot be negative".to_string(),
        });
    }
    validate_reporting_evidence_metadata(&mut field_errors, &row.evidence);

    let started = parse_timestamp_with_issues(
        &mut field_errors,
        "window_started_at_utc",
        &row.window_started_at_utc,
    );
    let ended = parse_timestamp_with_issues(
        &mut field_errors,
        "window_ended_at_utc",
        &row.window_ended_at_utc,
    );
    if let (Some(started), Some(ended)) = (started, ended)
        && ended < started
    {
        field_errors.push(ReportingValidationIssue {
            field: "window_ended_at_utc",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: "window_ended_at_utc must be >= window_started_at_utc".to_string(),
        });
    }

    if field_errors.is_empty() {
        return Ok(());
    }
    Err(ReportingContractError::invalid_payload_with_issues(
        "position reporting row failed validation",
        field_errors,
    ))
}

pub fn validate_risk_event_reporting_row(
    row: &RiskEventReportingRow,
) -> Result<(), ReportingContractError> {
    let mut field_errors = Vec::new();
    validate_canonical_identifier_field(&mut field_errors, "report_row_id", &row.report_row_id);
    validate_non_empty_field(&mut field_errors, "event_type", &row.event_type);
    validate_non_empty_field(&mut field_errors, "outcome", &row.outcome);
    validate_timestamp_field(&mut field_errors, "event_at_utc", &row.event_at_utc);
    validate_reporting_evidence_metadata(&mut field_errors, &row.evidence);
    if let Some(market_id) = row.market_id.as_deref() {
        validate_canonical_identifier_field(&mut field_errors, "market_id", market_id);
    }
    if !matches!(
        row.severity.as_str(),
        "normal" | "warning" | "critical" | "degraded"
    ) {
        field_errors.push(ReportingValidationIssue {
            field: "severity",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: "severity must be one of: normal, warning, critical, degraded".to_string(),
        });
    }

    if field_errors.is_empty() {
        return Ok(());
    }
    Err(ReportingContractError::invalid_payload_with_issues(
        "risk-event reporting row failed validation",
        field_errors,
    ))
}

pub fn validate_performance_reporting_row(
    row: &PerformanceReportingRow,
) -> Result<(), ReportingContractError> {
    const EPSILON: f64 = 1e-9;

    let mut field_errors = Vec::new();
    validate_canonical_identifier_field(&mut field_errors, "report_row_id", &row.report_row_id);
    validate_canonical_identifier_field(&mut field_errors, "market_id", &row.market_id);
    validate_canonical_identifier_field(&mut field_errors, "alpha_id", &row.alpha_id);
    validate_non_empty_field(&mut field_errors, "period_scope", &row.period_scope);
    if !matches!(row.period_scope.as_str(), "1h" | "24h" | "30d") {
        field_errors.push(ReportingValidationIssue {
            field: "period_scope",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: "period_scope must be one of: 1h, 24h, 30d".to_string(),
        });
    }
    validate_timestamp_field(&mut field_errors, "period_start_utc", &row.period_start_utc);
    validate_timestamp_field(&mut field_errors, "period_end_utc", &row.period_end_utc);
    validate_finite_number(&mut field_errors, "realized_pnl_usd", row.realized_pnl_usd);
    validate_finite_number(
        &mut field_errors,
        "unrealized_pnl_usd",
        row.unrealized_pnl_usd,
    );
    validate_finite_number(&mut field_errors, "gross_pnl_usd", row.gross_pnl_usd);
    validate_finite_number(&mut field_errors, "net_pnl_usd", row.net_pnl_usd);
    validate_finite_number(&mut field_errors, "fees_usd", row.fees_usd);
    validate_finite_number(&mut field_errors, "rebates_usd", row.rebates_usd);
    validate_finite_number(&mut field_errors, "incentives_usd", row.incentives_usd);
    validate_reporting_evidence_metadata(&mut field_errors, &row.evidence);

    let expected_gross = row.realized_pnl_usd + row.unrealized_pnl_usd;
    if (row.gross_pnl_usd - expected_gross).abs() > EPSILON {
        field_errors.push(ReportingValidationIssue {
            field: "gross_pnl_usd",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: "gross_pnl_usd must equal realized_pnl_usd + unrealized_pnl_usd".to_string(),
        });
    }
    let expected_net = row.gross_pnl_usd - (row.fees_usd - row.rebates_usd - row.incentives_usd);
    if (row.net_pnl_usd - expected_net).abs() > EPSILON {
        field_errors.push(ReportingValidationIssue {
            field: "net_pnl_usd",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: "net_pnl_usd must align with gross and cost components".to_string(),
        });
    }

    let started =
        parse_timestamp_with_issues(&mut field_errors, "period_start_utc", &row.period_start_utc);
    let ended =
        parse_timestamp_with_issues(&mut field_errors, "period_end_utc", &row.period_end_utc);
    if let (Some(started), Some(ended)) = (started, ended)
        && ended <= started
    {
        field_errors.push(ReportingValidationIssue {
            field: "period_end_utc",
            code: ReportingReasonCode::InvalidPayload.code(),
            message: "period_end_utc must be greater than period_start_utc".to_string(),
        });
    }

    if field_errors.is_empty() {
        return Ok(());
    }
    Err(ReportingContractError::invalid_payload_with_issues(
        "performance reporting row failed validation",
        field_errors,
    ))
}

pub fn sort_trade_reporting_rows(rows: &mut [TradeReportingRow]) {
    rows.sort_by(|left, right| {
        right
            .occurred_at_utc
            .cmp(&left.occurred_at_utc)
            .then_with(|| left.market_id.cmp(&right.market_id))
            .then_with(|| left.order_id.cmp(&right.order_id))
            .then_with(|| left.trade_id.cmp(&right.trade_id))
            .then_with(|| left.report_row_id.cmp(&right.report_row_id))
    });
}

pub fn sort_position_reporting_rows(rows: &mut [PositionReportingRow]) {
    rows.sort_by(|left, right| {
        right
            .evidence
            .as_of_utc
            .cmp(&left.evidence.as_of_utc)
            .then_with(|| left.market_id.cmp(&right.market_id))
            .then_with(|| left.evidence.run_id.cmp(&right.evidence.run_id))
            .then_with(|| left.report_row_id.cmp(&right.report_row_id))
    });
}

pub fn sort_risk_event_reporting_rows(rows: &mut [RiskEventReportingRow]) {
    rows.sort_by(|left, right| {
        right
            .event_at_utc
            .cmp(&left.event_at_utc)
            .then_with(|| left.event_type.cmp(&right.event_type))
            .then_with(|| left.report_row_id.cmp(&right.report_row_id))
    });
}

pub fn sort_performance_reporting_rows(rows: &mut [PerformanceReportingRow]) {
    rows.sort_by(|left, right| {
        right
            .period_end_utc
            .cmp(&left.period_end_utc)
            .then_with(|| left.market_id.cmp(&right.market_id))
            .then_with(|| left.alpha_id.cmp(&right.alpha_id))
            .then_with(|| left.report_row_id.cmp(&right.report_row_id))
    });
}

pub fn authorize_reporting_read(role: &str) -> Result<(), ReportingContractError> {
    let role = GovernanceRole::parse(role)
        .map_err(|error| ReportingContractError::unauthorized(error.message))?;
    let matrix = RolePermissionMatrix::canonical();
    let allowed = matrix
        .has_permission(role, GovernancePermission::ReadAnalytics)
        .map_err(|error| ReportingContractError::unauthorized(error.message))?;
    if !allowed {
        return Err(ReportingContractError::unauthorized(
            "role does not permit reporting read-model access",
        ));
    }
    Ok(())
}

fn validate_reporting_evidence_metadata(
    field_errors: &mut Vec<ReportingValidationIssue>,
    evidence: &ReportingEvidenceMetadata,
) {
    validate_timestamp_field(field_errors, "as_of_utc", &evidence.as_of_utc);
    validate_non_empty_field(field_errors, "source", &evidence.source);
    validate_non_empty_field(field_errors, "reason_code", &evidence.reason_code);
    validate_non_empty_field(field_errors, "correlation_id", &evidence.correlation_id);
    validate_canonical_identifier_field(field_errors, "correlation_id", &evidence.correlation_id);

    if !evidence.source.trim().is_empty() {
        let normalized = normalize_reporting_identifier(&evidence.source);
        if !normalized.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || "._:-".contains(character)
        }) {
            field_errors.push(ReportingValidationIssue {
                field: "source",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "source contains unsupported characters".to_string(),
            });
        }
    }

    if !evidence.reason_code.trim().is_empty() {
        let normalized = normalize_reporting_identifier(&evidence.reason_code);
        if !normalized.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || "._:-".contains(character)
        }) {
            field_errors.push(ReportingValidationIssue {
                field: "reason_code",
                code: ReportingReasonCode::InvalidPayload.code(),
                message: "reason_code contains unsupported characters".to_string(),
            });
        }
    }

    if let Some(run_id) = evidence.run_id.as_deref() {
        validate_canonical_identifier_field(field_errors, "run_id", run_id);
    }
    if let Some(snapshot_id) = evidence.snapshot_id.as_deref() {
        validate_canonical_identifier_field(field_errors, "snapshot_id", snapshot_id);
    }
}

fn normalize_optional_identifier(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, ReportingContractError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let normalized = normalize_reporting_identifier(raw);
    if normalized.is_empty() {
        return Err(ReportingContractError::invalid_payload_with_issues(
            format!("{field} cannot be blank"),
            vec![ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }

    let mut field_errors = Vec::new();
    validate_canonical_identifier_field(&mut field_errors, field, &normalized);
    if field_errors.is_empty() {
        return Ok(Some(normalized));
    }
    Err(ReportingContractError::invalid_payload_with_issues(
        format!("{field} failed canonical validation"),
        field_errors,
    ))
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, ()> {
    let parsed = OffsetDateTime::parse(value.trim(), &Rfc3339).map_err(|_| ())?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(());
    }
    Ok(parsed)
}

fn parse_timestamp_with_issues(
    field_errors: &mut Vec<ReportingValidationIssue>,
    field: &'static str,
    value: &str,
) -> Option<OffsetDateTime> {
    match parse_timestamp(value) {
        Ok(parsed) => Some(parsed),
        Err(_) => {
            field_errors.push(ReportingValidationIssue {
                field,
                code: ReportingReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp"),
            });
            None
        }
    }
}

fn validate_non_empty_field(
    field_errors: &mut Vec<ReportingValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(ReportingValidationIssue {
            field,
            code: ReportingReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_timestamp_field(
    field_errors: &mut Vec<ReportingValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_timestamp(value).is_err() {
        field_errors.push(ReportingValidationIssue {
            field,
            code: ReportingReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_finite_number(
    field_errors: &mut Vec<ReportingValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(ReportingValidationIssue {
            field,
            code: ReportingReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    }
}

fn validate_canonical_identifier_field(
    field_errors: &mut Vec<ReportingValidationIssue>,
    field: &'static str,
    value: &str,
) {
    validate_non_empty_field(field_errors, field, value);

    let normalized = normalize_reporting_identifier(value);
    if !(3..=160).contains(&normalized.len()) {
        field_errors.push(ReportingValidationIssue {
            field,
            code: ReportingReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain 3-160 canonical characters"),
        });
    }
    if !normalized.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character)
    }) {
        field_errors.push(ReportingValidationIssue {
            field,
            code: ReportingReasonCode::InvalidPayload.code(),
            message: format!("{field} contains unsupported characters"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_evidence() -> ReportingEvidenceMetadata {
        ReportingEvidenceMetadata {
            as_of_utc: "2026-04-07T02:24:01Z".to_string(),
            source: "reporting.read-models.v1".to_string(),
            reason_code: "reporting_ready".to_string(),
            correlation_id: "corr-reporting-001".to_string(),
            run_id: Some("run-reporting-001".to_string()),
            snapshot_id: Some("snapshot-reporting-001".to_string()),
        }
    }

    fn sample_trade_row(report_row_id: &str, occurred_at_utc: &str) -> TradeReportingRow {
        TradeReportingRow {
            report_row_id: report_row_id.to_string(),
            order_id: "order-reporting-001".to_string(),
            trade_id: format!("trade-{report_row_id}"),
            market_id: "market-btc-election".to_string(),
            asset_id: "asset-btc".to_string(),
            lifecycle_state: "filled".to_string(),
            event_status: "matched".to_string(),
            occurred_at_utc: occurred_at_utc.to_string(),
            evidence: sample_evidence(),
        }
    }

    fn sample_position_row() -> PositionReportingRow {
        PositionReportingRow {
            report_row_id: "snapshot-reporting-001".to_string(),
            market_id: "market-btc-election".to_string(),
            net_exposure: 12.4,
            gross_exposure: 20.1,
            open_order_count: 2,
            run_status: "succeeded".to_string(),
            window_started_at_utc: "2026-04-07T01:00:00Z".to_string(),
            window_ended_at_utc: "2026-04-07T02:00:00Z".to_string(),
            evidence: sample_evidence(),
        }
    }

    fn sample_risk_row(report_row_id: &str, event_at_utc: &str) -> RiskEventReportingRow {
        RiskEventReportingRow {
            report_row_id: report_row_id.to_string(),
            event_type: "pretrade_gate".to_string(),
            severity: "critical".to_string(),
            outcome: "deny".to_string(),
            market_id: Some("market-btc-election".to_string()),
            event_at_utc: event_at_utc.to_string(),
            evidence: sample_evidence(),
        }
    }

    fn sample_performance_row() -> PerformanceReportingRow {
        PerformanceReportingRow {
            report_row_id: "snapshot-performance-001".to_string(),
            market_id: "market-btc-election".to_string(),
            alpha_id: "alpha-momentum".to_string(),
            period_scope: "24h".to_string(),
            period_start_utc: "2026-04-06T02:24:01Z".to_string(),
            period_end_utc: "2026-04-07T02:24:01Z".to_string(),
            realized_pnl_usd: 100.0,
            unrealized_pnl_usd: 20.0,
            gross_pnl_usd: 120.0,
            net_pnl_usd: 110.0,
            fees_usd: 8.0,
            rebates_usd: 1.0,
            incentives_usd: 3.0,
            evidence: sample_evidence(),
        }
    }

    #[test]
    fn reporting_reason_codes_round_trip() {
        for code in [
            ReportingReasonCode::Ready,
            ReportingReasonCode::EmptyWindow,
            ReportingReasonCode::InvalidPayload,
            ReportingReasonCode::Unauthorized,
            ReportingReasonCode::DependencyUnavailable,
            ReportingReasonCode::StaleDependency,
            ReportingReasonCode::PersistenceUnavailable,
            ReportingReasonCode::EvidenceUnavailable,
        ] {
            let parsed = ReportingReasonCode::parse(code.code()).expect("reason code must parse");
            assert_eq!(parsed, code);
        }
    }

    #[test]
    fn query_builder_enforces_window_boundaries_and_limits() {
        let query = build_reporting_read_query(
            "2026-04-07T01:00:00Z",
            "2026-04-07T02:00:00Z",
            Some(" MARKET-BTC-ELECTION "),
            None,
            Some(" CORR-001 "),
            DEFAULT_REPORTING_LIMIT,
        )
        .expect("query should build");
        assert_eq!(query.market_id.as_deref(), Some("market-btc-election"));
        assert_eq!(query.correlation_id.as_deref(), Some("corr-001"));

        let error = build_reporting_read_query(
            "2026-04-07T02:00:00Z",
            "2026-04-07T02:00:00Z",
            None,
            None,
            None,
            0,
        )
        .expect_err("invalid window and limit should fail");
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "end_exclusive_utc")
        );
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "limit")
        );
    }

    #[test]
    fn query_builder_rejects_malformed_identifiers() {
        let error = build_reporting_read_query(
            "2026-04-07T01:00:00Z",
            "2026-04-07T02:00:00Z",
            Some("market/btc-election"),
            None,
            Some("corr/reporting-001"),
            DEFAULT_REPORTING_LIMIT,
        )
        .expect_err("malformed identifiers should fail");
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market_id")
        );
    }

    #[test]
    fn query_builder_rejects_non_utc_offsets() {
        let error = build_reporting_read_query(
            "2026-04-07T01:00:00+01:00",
            "2026-04-07T02:00:00Z",
            Some("market-btc-election"),
            None,
            Some("corr-reporting-001"),
            DEFAULT_REPORTING_LIMIT,
        )
        .expect_err("non-UTC timestamps should fail");
        assert_eq!(error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "start_inclusive_utc")
        );
    }

    #[test]
    fn normalization_helpers_are_deterministic() {
        assert_eq!(
            normalize_reporting_identifier("  Market-BTC:Election "),
            "market-btc:election"
        );
        let parsed = parse_utc_timestamp("as_of_utc", "2026-04-07T02:24:01Z")
            .expect("timestamp should parse");
        assert_eq!(format_utc_timestamp(parsed), "2026-04-07T02:24:01Z");
    }

    #[test]
    fn trade_row_sorting_uses_stable_tie_breakers() {
        let mut rows = vec![
            sample_trade_row("trade-row-2", "2026-04-07T02:00:00Z"),
            sample_trade_row("trade-row-1", "2026-04-07T02:00:00Z"),
            sample_trade_row("trade-row-3", "2026-04-07T01:59:59Z"),
        ];
        rows[0].order_id = "order-b".to_string();
        rows[1].order_id = "order-a".to_string();

        sort_trade_reporting_rows(&mut rows);
        assert_eq!(rows[0].order_id, "order-a");
        assert_eq!(rows[1].order_id, "order-b");
        assert_eq!(rows[2].report_row_id, "trade-row-3");
    }

    #[test]
    fn row_validation_catches_missing_evidence_and_numeric_contracts() {
        let mut trade = sample_trade_row("trade-row-1", "2026-04-07T02:00:00Z");
        trade.evidence.source = " ".to_string();
        let trade_error =
            validate_trade_reporting_row(&trade).expect_err("missing source evidence should fail");
        assert_eq!(trade_error.code, ReportingReasonCode::InvalidPayload.code());
        assert!(
            trade_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "source")
        );

        let mut performance = sample_performance_row();
        performance.net_pnl_usd = f64::NAN;
        let perf_error = validate_performance_reporting_row(&performance)
            .expect_err("non-finite net_pnl_usd should fail");
        assert!(
            perf_error
                .field_errors
                .iter()
                .any(|issue| issue.field == "net_pnl_usd")
        );
    }

    #[test]
    fn authorization_gate_maps_to_reporting_unauthorized_reason() {
        let error =
            authorize_reporting_read("guest").expect_err("unknown role should fail authorization");
        assert_eq!(error.code, ReportingReasonCode::Unauthorized.code());
        assert!(authorize_reporting_read("read_only_analytics").is_ok());
    }

    #[test]
    fn position_and_risk_rows_validate_successfully_with_canonical_payloads() {
        assert!(validate_position_reporting_row(&sample_position_row()).is_ok());
        assert!(
            validate_risk_event_reporting_row(&sample_risk_row(
                "risk-event-1",
                "2026-04-07T02:10:00Z"
            ))
            .is_ok()
        );
    }

    #[test]
    fn risk_event_sorting_is_deterministic() {
        let mut rows = vec![
            sample_risk_row("risk-event-2", "2026-04-07T02:10:00Z"),
            sample_risk_row("risk-event-1", "2026-04-07T02:10:00Z"),
            sample_risk_row("risk-event-3", "2026-04-07T02:09:59Z"),
        ];
        rows[0].event_type = "safety_control".to_string();
        rows[1].event_type = "pretrade_gate".to_string();
        sort_risk_event_reporting_rows(&mut rows);
        assert_eq!(rows[0].event_type, "pretrade_gate");
        assert_eq!(rows[1].event_type, "safety_control");
        assert_eq!(rows[2].report_row_id, "risk-event-3");
    }
}
