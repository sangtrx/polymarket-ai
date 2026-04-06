use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::{Duration, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttributionPeriod {
    OneHour,
    TwentyFourHours,
    ThirtyDays,
}

impl AttributionPeriod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneHour => "1h",
            Self::TwentyFourHours => "24h",
            Self::ThirtyDays => "30d",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AttributionContractError> {
        match value {
            "1h" => Ok(Self::OneHour),
            "24h" => Ok(Self::TwentyFourHours),
            "30d" => Ok(Self::ThirtyDays),
            _ => Err(AttributionContractError::invalid_payload(format!(
                "unsupported attribution period `{value}`; expected one of: 1h, 24h, 30d"
            ))),
        }
    }

    const fn duration(self) -> Duration {
        match self {
            Self::OneHour => Duration::hours(1),
            Self::TwentyFourHours => Duration::hours(24),
            Self::ThirtyDays => Duration::days(30),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AttributionReasonCode {
    Ready,
    EmptyWindow,
    InvalidPayload,
    Unauthorized,
    ProjectionUnavailable,
    StaleSource,
    PersistenceUnavailable,
}

impl AttributionReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Ready => "attribution_ready",
            Self::EmptyWindow => "attribution_empty_window",
            Self::InvalidPayload => "attribution_invalid_payload",
            Self::Unauthorized => "attribution_unauthorized",
            Self::ProjectionUnavailable => "attribution_projection_unavailable",
            Self::StaleSource => "attribution_stale_source",
            Self::PersistenceUnavailable => "attribution_persistence_unavailable",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AttributionContractError> {
        match value {
            "attribution_ready" => Ok(Self::Ready),
            "attribution_empty_window" => Ok(Self::EmptyWindow),
            "attribution_invalid_payload" => Ok(Self::InvalidPayload),
            "attribution_unauthorized" => Ok(Self::Unauthorized),
            "attribution_projection_unavailable" => Ok(Self::ProjectionUnavailable),
            "attribution_stale_source" => Ok(Self::StaleSource),
            "attribution_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            _ => Err(AttributionContractError::invalid_payload(format!(
                "unknown attribution reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttributionValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttributionContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<AttributionValidationIssue>,
}

impl AttributionContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: AttributionReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn invalid_payload_with_issues(
        message: impl Into<String>,
        field_errors: Vec<AttributionValidationIssue>,
    ) -> Self {
        Self {
            code: AttributionReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributionObservation {
    pub market_id: String,
    pub alpha_id: String,
    pub period_end_utc: String,
    pub realized_pnl_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub fees_usd: f64,
    pub rebates_usd: f64,
    pub incentives_usd: f64,
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub snapshot_id: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttributionQueryScope {
    pub market_id: Option<String>,
    pub alpha_id: Option<String>,
    pub period: AttributionPeriod,
    pub as_of_utc: String,
    pub start_inclusive_utc: String,
    pub end_exclusive_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributionCostBreakdown {
    pub fees_usd: f64,
    pub rebates_usd: f64,
    pub incentives_usd: f64,
    pub net_cost_impact_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttributionRow {
    pub market_id: String,
    pub alpha_id: String,
    pub period: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub realized_pnl_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub gross_pnl_usd: f64,
    pub net_pnl_usd: f64,
    pub costs: AttributionCostBreakdown,
    pub as_of_utc: String,
    pub source: String,
    pub reason_code: String,
    pub correlation_id: String,
    pub snapshot_id: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone)]
struct AttributionAccumulator {
    realized_pnl_usd: f64,
    unrealized_pnl_usd: f64,
    fees_usd: f64,
    rebates_usd: f64,
    incentives_usd: f64,
    latest_period_end: OffsetDateTime,
    source: String,
    reason_code: String,
    correlation_id: String,
    snapshot_id: Option<String>,
    run_id: Option<String>,
}

pub fn normalize_attribution_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn build_query_scope(
    period: AttributionPeriod,
    as_of_utc: &str,
    market_id: Option<&str>,
    alpha_id: Option<&str>,
) -> Result<AttributionQueryScope, AttributionContractError> {
    let as_of = parse_timestamp_or_error("as_of_utc", as_of_utc)?;
    let start = as_of - period.duration();
    let normalized_market = normalize_optional_identifier("market_id", market_id)?;
    let normalized_alpha = normalize_optional_identifier("alpha_id", alpha_id)?;

    Ok(AttributionQueryScope {
        market_id: normalized_market,
        alpha_id: normalized_alpha,
        period,
        as_of_utc: format_timestamp(as_of),
        start_inclusive_utc: format_timestamp(start),
        end_exclusive_utc: format_timestamp(as_of),
    })
}

pub fn build_cost_aware_attribution_rows(
    observations: &[AttributionObservation],
    scope: &AttributionQueryScope,
) -> Result<Vec<AttributionRow>, AttributionContractError> {
    let start = parse_timestamp_or_error("start_inclusive_utc", &scope.start_inclusive_utc)?;
    let end = parse_timestamp_or_error("end_exclusive_utc", &scope.end_exclusive_utc)?;
    if end <= start {
        return Err(AttributionContractError::invalid_payload(
            "end_exclusive_utc must be greater than start_inclusive_utc",
        ));
    }

    let mut groups: BTreeMap<(String, String), AttributionAccumulator> = BTreeMap::new();
    for observation in observations {
        validate_attribution_observation(observation)?;

        let market_id = normalize_attribution_identifier(&observation.market_id);
        let alpha_id = normalize_attribution_identifier(&observation.alpha_id);
        if scope.market_id.as_deref().is_some_and(|value| value != market_id) {
            continue;
        }
        if scope.alpha_id.as_deref().is_some_and(|value| value != alpha_id) {
            continue;
        }

        let period_end = parse_timestamp_or_error("period_end_utc", &observation.period_end_utc)?;
        if period_end < start || period_end >= end {
            continue;
        }

        let accumulator = groups
            .entry((market_id, alpha_id))
            .or_insert_with(|| AttributionAccumulator {
                realized_pnl_usd: 0.0,
                unrealized_pnl_usd: 0.0,
                fees_usd: 0.0,
                rebates_usd: 0.0,
                incentives_usd: 0.0,
                latest_period_end: period_end,
                source: observation.source.clone(),
                reason_code: observation.reason_code.clone(),
                correlation_id: observation.correlation_id.clone(),
                snapshot_id: observation.snapshot_id.clone(),
                run_id: observation.run_id.clone(),
            });

        accumulator.realized_pnl_usd += observation.realized_pnl_usd;
        accumulator.unrealized_pnl_usd += observation.unrealized_pnl_usd;
        accumulator.fees_usd += observation.fees_usd;
        accumulator.rebates_usd += observation.rebates_usd;
        accumulator.incentives_usd += observation.incentives_usd;

        let tie_break = period_end == accumulator.latest_period_end
            && observation.correlation_id < accumulator.correlation_id;
        if period_end > accumulator.latest_period_end || tie_break {
            accumulator.latest_period_end = period_end;
            accumulator.source = observation.source.clone();
            accumulator.reason_code = observation.reason_code.clone();
            accumulator.correlation_id = observation.correlation_id.clone();
            accumulator.snapshot_id = observation.snapshot_id.clone();
            accumulator.run_id = observation.run_id.clone();
        }
    }

    let mut rows: Vec<AttributionRow> = groups
        .into_iter()
        .map(|((market_id, alpha_id), item)| {
            let gross = item.realized_pnl_usd + item.unrealized_pnl_usd;
            let net_cost = item.fees_usd - item.rebates_usd - item.incentives_usd;
            let net = gross - net_cost;

            AttributionRow {
                market_id,
                alpha_id,
                period: scope.period.as_str().to_string(),
                period_start_utc: scope.start_inclusive_utc.clone(),
                period_end_utc: format_timestamp(item.latest_period_end),
                realized_pnl_usd: item.realized_pnl_usd,
                unrealized_pnl_usd: item.unrealized_pnl_usd,
                gross_pnl_usd: gross,
                net_pnl_usd: net,
                costs: AttributionCostBreakdown {
                    fees_usd: item.fees_usd,
                    rebates_usd: item.rebates_usd,
                    incentives_usd: item.incentives_usd,
                    net_cost_impact_usd: net_cost,
                },
                as_of_utc: scope.as_of_utc.clone(),
                source: item.source,
                reason_code: item.reason_code,
                correlation_id: item.correlation_id,
                snapshot_id: item.snapshot_id,
                run_id: item.run_id,
            }
        })
        .collect();

    rows.sort_by(|left, right| {
        right
            .period_end_utc
            .cmp(&left.period_end_utc)
            .then_with(|| left.market_id.cmp(&right.market_id))
            .then_with(|| left.alpha_id.cmp(&right.alpha_id))
    });
    Ok(rows)
}

pub fn validate_attribution_observation(
    observation: &AttributionObservation,
) -> Result<(), AttributionContractError> {
    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "market_id", &observation.market_id);
    validate_non_empty_field(&mut field_errors, "alpha_id", &observation.alpha_id);
    validate_non_empty_field(&mut field_errors, "source", &observation.source);
    validate_non_empty_field(&mut field_errors, "reason_code", &observation.reason_code);
    validate_non_empty_field(
        &mut field_errors,
        "correlation_id",
        &observation.correlation_id,
    );
    validate_utc_timestamp_field(&mut field_errors, "period_end_utc", &observation.period_end_utc);
    validate_utc_timestamp_field(&mut field_errors, "as_of_utc", &observation.as_of_utc);
    validate_canonical_identifier_field(&mut field_errors, "market_id", &observation.market_id);
    validate_canonical_identifier_field(&mut field_errors, "alpha_id", &observation.alpha_id);

    if let Some(value) = non_empty_optional(observation.snapshot_id.clone()) {
        validate_canonical_identifier_field(&mut field_errors, "snapshot_id", &value);
    }
    if let Some(value) = non_empty_optional(observation.run_id.clone()) {
        validate_canonical_identifier_field(&mut field_errors, "run_id", &value);
    }

    if AttributionReasonCode::parse(&observation.reason_code).is_err() {
        field_errors.push(AttributionValidationIssue {
            field: "reason_code",
            code: AttributionReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known attribution reason".to_string(),
        });
    }

    validate_finite_number(
        &mut field_errors,
        "realized_pnl_usd",
        observation.realized_pnl_usd,
    );
    validate_finite_number(
        &mut field_errors,
        "unrealized_pnl_usd",
        observation.unrealized_pnl_usd,
    );
    validate_finite_number(&mut field_errors, "fees_usd", observation.fees_usd);
    validate_finite_number(&mut field_errors, "rebates_usd", observation.rebates_usd);
    validate_finite_number(
        &mut field_errors,
        "incentives_usd",
        observation.incentives_usd,
    );

    if field_errors.is_empty() {
        return Ok(());
    }

    Err(AttributionContractError::invalid_payload_with_issues(
        "attribution observation payload failed validation",
        field_errors,
    ))
}

pub fn validate_attribution_row(row: &AttributionRow) -> Result<(), AttributionContractError> {
    const CONSISTENCY_EPSILON: f64 = 1e-9;

    let mut field_errors = Vec::new();
    validate_non_empty_field(&mut field_errors, "market_id", &row.market_id);
    validate_non_empty_field(&mut field_errors, "alpha_id", &row.alpha_id);
    validate_non_empty_field(&mut field_errors, "period", &row.period);
    validate_non_empty_field(&mut field_errors, "source", &row.source);
    validate_non_empty_field(&mut field_errors, "reason_code", &row.reason_code);
    validate_non_empty_field(&mut field_errors, "correlation_id", &row.correlation_id);
    validate_utc_timestamp_field(&mut field_errors, "period_start_utc", &row.period_start_utc);
    validate_utc_timestamp_field(&mut field_errors, "period_end_utc", &row.period_end_utc);
    validate_utc_timestamp_field(&mut field_errors, "as_of_utc", &row.as_of_utc);
    validate_canonical_identifier_field(&mut field_errors, "market_id", &row.market_id);
    validate_canonical_identifier_field(&mut field_errors, "alpha_id", &row.alpha_id);
    if let Some(value) = non_empty_optional(row.snapshot_id.clone()) {
        validate_canonical_identifier_field(&mut field_errors, "snapshot_id", &value);
    }
    if let Some(value) = non_empty_optional(row.run_id.clone()) {
        validate_canonical_identifier_field(&mut field_errors, "run_id", &value);
    }

    if AttributionPeriod::parse(&row.period).is_err() {
        field_errors.push(AttributionValidationIssue {
            field: "period",
            code: AttributionReasonCode::InvalidPayload.code(),
            message: "period must be one of 1h, 24h, or 30d".to_string(),
        });
    }
    if AttributionReasonCode::parse(&row.reason_code).is_err() {
        field_errors.push(AttributionValidationIssue {
            field: "reason_code",
            code: AttributionReasonCode::InvalidPayload.code(),
            message: "reason_code must be a known attribution reason".to_string(),
        });
    }

    validate_finite_number(&mut field_errors, "realized_pnl_usd", row.realized_pnl_usd);
    validate_finite_number(
        &mut field_errors,
        "unrealized_pnl_usd",
        row.unrealized_pnl_usd,
    );
    validate_finite_number(&mut field_errors, "gross_pnl_usd", row.gross_pnl_usd);
    validate_finite_number(&mut field_errors, "net_pnl_usd", row.net_pnl_usd);
    validate_finite_number(&mut field_errors, "fees_usd", row.costs.fees_usd);
    validate_finite_number(&mut field_errors, "rebates_usd", row.costs.rebates_usd);
    validate_finite_number(&mut field_errors, "incentives_usd", row.costs.incentives_usd);
    validate_finite_number(
        &mut field_errors,
        "net_cost_impact_usd",
        row.costs.net_cost_impact_usd,
    );

    let expected_gross = row.realized_pnl_usd + row.unrealized_pnl_usd;
    if (row.gross_pnl_usd - expected_gross).abs() > CONSISTENCY_EPSILON {
        field_errors.push(AttributionValidationIssue {
            field: "gross_pnl_usd",
            code: AttributionReasonCode::InvalidPayload.code(),
            message: "gross_pnl_usd must equal realized_pnl_usd + unrealized_pnl_usd".to_string(),
        });
    }
    let expected_net_cost = row.costs.fees_usd - row.costs.rebates_usd - row.costs.incentives_usd;
    if (row.costs.net_cost_impact_usd - expected_net_cost).abs() > CONSISTENCY_EPSILON {
        field_errors.push(AttributionValidationIssue {
            field: "net_cost_impact_usd",
            code: AttributionReasonCode::InvalidPayload.code(),
            message:
                "net_cost_impact_usd must equal fees_usd - rebates_usd - incentives_usd"
                    .to_string(),
        });
    }
    let expected_net = row.gross_pnl_usd - row.costs.net_cost_impact_usd;
    if (row.net_pnl_usd - expected_net).abs() > CONSISTENCY_EPSILON {
        field_errors.push(AttributionValidationIssue {
            field: "net_pnl_usd",
            code: AttributionReasonCode::InvalidPayload.code(),
            message: "net_pnl_usd must equal gross_pnl_usd - net_cost_impact_usd".to_string(),
        });
    }

    if field_errors.is_empty() {
        return Ok(());
    }

    Err(AttributionContractError::invalid_payload_with_issues(
        "attribution row payload failed validation",
        field_errors,
    ))
}

fn non_empty_optional(value: Option<String>) -> Option<String> {
    value.and_then(|entry| {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn validate_non_empty_field(
    field_errors: &mut Vec<AttributionValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if value.trim().is_empty() {
        field_errors.push(AttributionValidationIssue {
            field,
            code: AttributionReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_canonical_identifier_field(
    field_errors: &mut Vec<AttributionValidationIssue>,
    field: &'static str,
    value: &str,
) {
    validate_non_empty_field(field_errors, field, value);
    let normalized = normalize_attribution_identifier(value);
    if normalized.len() < 3 || normalized.len() > 120 {
        field_errors.push(AttributionValidationIssue {
            field,
            code: AttributionReasonCode::InvalidPayload.code(),
            message: format!("{field} must contain 3-120 canonical characters"),
        });
    }
    if !normalized
        .chars()
        .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || "._:-".contains(character))
    {
        field_errors.push(AttributionValidationIssue {
            field,
            code: AttributionReasonCode::InvalidPayload.code(),
            message: format!("{field} contains unsupported characters"),
        });
    }
}

fn validate_utc_timestamp_field(
    field_errors: &mut Vec<AttributionValidationIssue>,
    field: &'static str,
    value: &str,
) {
    if parse_timestamp(value).is_err() {
        field_errors.push(AttributionValidationIssue {
            field,
            code: AttributionReasonCode::InvalidPayload.code(),
            message: format!("{field} must be an RFC3339 UTC timestamp"),
        });
    }
}

fn validate_finite_number(
    field_errors: &mut Vec<AttributionValidationIssue>,
    field: &'static str,
    value: f64,
) {
    if !value.is_finite() {
        field_errors.push(AttributionValidationIssue {
            field,
            code: AttributionReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite"),
        });
    }
}

fn validate_canonical_identifier(
    field: &'static str,
    value: &str,
) -> Result<(), AttributionContractError> {
    let mut issues = Vec::new();
    validate_canonical_identifier_field(&mut issues, field, value);
    if issues.is_empty() {
        return Ok(());
    }
    Err(AttributionContractError::invalid_payload_with_issues(
        format!("{field} failed canonical validation"),
        issues,
    ))
}

fn normalize_optional_identifier(
    field: &'static str,
    value: Option<&str>,
) -> Result<Option<String>, AttributionContractError> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let normalized = normalize_attribution_identifier(raw);
    if normalized.is_empty() {
        return Err(AttributionContractError::invalid_payload_with_issues(
            format!("{field} cannot be blank"),
            vec![AttributionValidationIssue {
                field,
                code: AttributionReasonCode::InvalidPayload.code(),
                message: format!("{field} cannot be blank"),
            }],
        ));
    }

    validate_canonical_identifier(field, &normalized)?;
    Ok(Some(normalized))
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
) -> Result<OffsetDateTime, AttributionContractError> {
    parse_timestamp(value).map_err(|_| {
        AttributionContractError::invalid_payload_with_issues(
            format!("{field} must be an RFC3339 UTC timestamp"),
            vec![AttributionValidationIssue {
                field,
                code: AttributionReasonCode::InvalidPayload.code(),
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

    fn sample_observation(
        market_id: &str,
        alpha_id: &str,
        period_end_utc: &str,
        reason_code: AttributionReasonCode,
        correlation_id: &str,
    ) -> AttributionObservation {
        AttributionObservation {
            market_id: market_id.to_string(),
            alpha_id: alpha_id.to_string(),
            period_end_utc: period_end_utc.to_string(),
            realized_pnl_usd: 120.0,
            unrealized_pnl_usd: 30.0,
            fees_usd: 8.0,
            rebates_usd: 1.5,
            incentives_usd: 0.5,
            as_of_utc: "2026-04-06T15:00:00Z".to_string(),
            source: "reconciliation.exposure.v1".to_string(),
            reason_code: reason_code.code().to_string(),
            correlation_id: correlation_id.to_string(),
            snapshot_id: Some("snapshot::001".to_string()),
            run_id: Some("run::001".to_string()),
        }
    }

    fn sample_row() -> AttributionRow {
        AttributionRow {
            market_id: "market-btc-election".to_string(),
            alpha_id: "alpha-momentum".to_string(),
            period: AttributionPeriod::TwentyFourHours.as_str().to_string(),
            period_start_utc: "2026-04-05T15:00:00Z".to_string(),
            period_end_utc: "2026-04-06T15:00:00Z".to_string(),
            realized_pnl_usd: 120.0,
            unrealized_pnl_usd: 30.0,
            gross_pnl_usd: 150.0,
            net_pnl_usd: 144.0,
            costs: AttributionCostBreakdown {
                fees_usd: 8.0,
                rebates_usd: 2.0,
                incentives_usd: 0.0,
                net_cost_impact_usd: 6.0,
            },
            as_of_utc: "2026-04-06T15:00:00Z".to_string(),
            source: "portfolio-engine.attribution.v1".to_string(),
            reason_code: AttributionReasonCode::Ready.code().to_string(),
            correlation_id: "corr-attribution-001".to_string(),
            snapshot_id: Some("snapshot::attribution::001".to_string()),
            run_id: Some("run::reconciliation::001".to_string()),
        }
    }

    #[test]
    fn period_parsing_supports_canonical_windows() {
        assert_eq!(
            AttributionPeriod::parse("1h").expect("1h should parse"),
            AttributionPeriod::OneHour
        );
        assert_eq!(
            AttributionPeriod::parse("24h").expect("24h should parse"),
            AttributionPeriod::TwentyFourHours
        );
        assert_eq!(
            AttributionPeriod::parse("30d").expect("30d should parse"),
            AttributionPeriod::ThirtyDays
        );
        assert_eq!(
            AttributionPeriod::parse("7d")
                .expect_err("unsupported periods must fail")
                .code,
            AttributionReasonCode::InvalidPayload.code()
        );
    }

    #[test]
    fn query_scope_enforces_start_inclusive_end_exclusive_boundaries() {
        let scope = build_query_scope(
            AttributionPeriod::OneHour,
            "2026-04-06T15:00:00Z",
            None,
            None,
        )
        .expect("scope should be built");
        assert_eq!(scope.start_inclusive_utc, "2026-04-06T14:00:00Z");
        assert_eq!(scope.end_exclusive_utc, "2026-04-06T15:00:00Z");
    }

    #[test]
    fn query_scope_rejects_blank_optional_filters() {
        let error = build_query_scope(
            AttributionPeriod::TwentyFourHours,
            "2026-04-06T15:00:00Z",
            Some("   "),
            None,
        )
        .expect_err("blank market_id should fail");

        assert_eq!(error.code, AttributionReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "market_id")
        );
    }

    #[test]
    fn deterministic_sort_orders_rows_by_period_then_stable_keys() {
        let scope = build_query_scope(
            AttributionPeriod::TwentyFourHours,
            "2026-04-06T15:00:00Z",
            None,
            None,
        )
        .expect("scope should be built");

        let rows = build_cost_aware_attribution_rows(
            &[
                sample_observation(
                    "market-b",
                    "alpha-z",
                    "2026-04-06T14:30:00Z",
                    AttributionReasonCode::Ready,
                    "corr-b",
                ),
                sample_observation(
                    "market-a",
                    "alpha-x",
                    "2026-04-06T14:30:00Z",
                    AttributionReasonCode::Ready,
                    "corr-a",
                ),
            ],
            &scope,
        )
        .expect("aggregation should succeed");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].market_id, "market-a");
        assert_eq!(rows[1].market_id, "market-b");
        assert_eq!(rows[0].period_end_utc, "2026-04-06T14:30:00Z");
        assert_eq!(rows[1].period_end_utc, "2026-04-06T14:30:00Z");
    }

    #[test]
    fn zero_activity_window_returns_empty_set_without_implicit_zero_rows() {
        let scope = build_query_scope(
            AttributionPeriod::OneHour,
            "2026-04-06T15:00:00Z",
            Some("market-a"),
            None,
        )
        .expect("scope should be built");

        let rows = build_cost_aware_attribution_rows(
            &[sample_observation(
                "market-a",
                "alpha-x",
                "2026-04-06T12:00:00Z",
                AttributionReasonCode::Ready,
                "corr-outside-window",
            )],
            &scope,
        )
        .expect("aggregation should succeed");

        assert!(rows.is_empty());
    }

    #[test]
    fn observation_validation_rejects_unknown_reason_code() {
        let mut observation = sample_observation(
            "market-a",
            "alpha-x",
            "2026-04-06T14:30:00Z",
            AttributionReasonCode::Ready,
            "corr-unknown",
        );
        observation.reason_code = "unexpected_reason".to_string();
        let error = validate_attribution_observation(&observation)
            .expect_err("unknown reason_code should fail validation");
        assert_eq!(error.code, AttributionReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "reason_code")
        );
    }

    #[test]
    fn row_validation_rejects_inconsistent_financial_totals() {
        let mut row = sample_row();
        row.net_pnl_usd += 0.5;

        let error = validate_attribution_row(&row)
            .expect_err("inconsistent net pnl should fail validation");
        assert_eq!(error.code, AttributionReasonCode::InvalidPayload.code());
        assert!(
            error
                .field_errors
                .iter()
                .any(|issue| issue.field == "net_pnl_usd")
        );
    }
}
