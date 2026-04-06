use crate::governance::{GovernancePermission, GovernanceRole, RolePermissionMatrix};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub const RECONCILIATION_CRITICAL_MISMATCH_THRESHOLD: f64 = 0.001;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationRunStatus {
    Succeeded,
    CriticalHalt,
    Failed,
}

impl ReconciliationRunStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::CriticalHalt => "critical_halt",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReconciliationContractError> {
        match value {
            "succeeded" => Ok(Self::Succeeded),
            "critical_halt" => Ok(Self::CriticalHalt),
            "failed" => Ok(Self::Failed),
            _ => Err(ReconciliationContractError::invalid_payload(format!(
                "unknown reconciliation run status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationDiffClass {
    MissingInternalRecord,
    MissingVenueRecord,
    MarketMismatch,
    LifecycleStateMismatch,
    QuantityMismatch,
    PriceMismatch,
}

impl ReconciliationDiffClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingInternalRecord => "missing_internal_record",
            Self::MissingVenueRecord => "missing_venue_record",
            Self::MarketMismatch => "market_mismatch",
            Self::LifecycleStateMismatch => "lifecycle_state_mismatch",
            Self::QuantityMismatch => "quantity_mismatch",
            Self::PriceMismatch => "price_mismatch",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReconciliationContractError> {
        match value {
            "missing_internal_record" => Ok(Self::MissingInternalRecord),
            "missing_venue_record" => Ok(Self::MissingVenueRecord),
            "market_mismatch" => Ok(Self::MarketMismatch),
            "lifecycle_state_mismatch" => Ok(Self::LifecycleStateMismatch),
            "quantity_mismatch" => Ok(Self::QuantityMismatch),
            "price_mismatch" => Ok(Self::PriceMismatch),
            _ => Err(ReconciliationContractError::invalid_payload(format!(
                "unknown reconciliation diff class `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationReasonCode {
    Matched,
    NonCriticalMismatch,
    CriticalMismatch,
    WindowUnavailable,
    VenueUnavailable,
    PersistenceUnavailable,
    Unauthorized,
    StateHydrationFailed,
    InvalidPayload,
}

impl ReconciliationReasonCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Matched => "reconciliation_matched",
            Self::NonCriticalMismatch => "reconciliation_non_critical_mismatch",
            Self::CriticalMismatch => "reconciliation_critical_mismatch",
            Self::WindowUnavailable => "reconciliation_window_unavailable",
            Self::VenueUnavailable => "reconciliation_venue_unavailable",
            Self::PersistenceUnavailable => "reconciliation_persistence_unavailable",
            Self::Unauthorized => "reconciliation_unauthorized",
            Self::StateHydrationFailed => "reconciliation_state_hydration_failed",
            Self::InvalidPayload => "reconciliation_invalid_payload",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ReconciliationContractError> {
        match value {
            "reconciliation_matched" => Ok(Self::Matched),
            "reconciliation_non_critical_mismatch" => Ok(Self::NonCriticalMismatch),
            "reconciliation_critical_mismatch" => Ok(Self::CriticalMismatch),
            "reconciliation_window_unavailable" => Ok(Self::WindowUnavailable),
            "reconciliation_venue_unavailable" => Ok(Self::VenueUnavailable),
            "reconciliation_persistence_unavailable" => Ok(Self::PersistenceUnavailable),
            "reconciliation_unauthorized" => Ok(Self::Unauthorized),
            "reconciliation_state_hydration_failed" => Ok(Self::StateHydrationFailed),
            "reconciliation_invalid_payload" => Ok(Self::InvalidPayload),
            _ => Err(ReconciliationContractError::invalid_payload(format!(
                "unknown reconciliation reason code `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconciliationValidationIssue {
    pub field: &'static str,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconciliationContractError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ReconciliationValidationIssue>,
}

impl ReconciliationContractError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn window_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ReconciliationReasonCode::WindowUnavailable.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: ReconciliationReasonCode::Unauthorized.code(),
            message: message.into(),
            field_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconciliationOrderRecord {
    pub order_id: String,
    pub market_id: String,
    pub lifecycle_state: String,
    pub quantity: Option<f64>,
    pub price: Option<f64>,
    pub observed_at_utc: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconciliationDiffRecord {
    pub diff_id: String,
    pub run_id: String,
    pub order_id: String,
    pub market_id: String,
    pub diff_class: ReconciliationDiffClass,
    pub reason_code: String,
    pub internal_value: Option<String>,
    pub venue_value: Option<String>,
    pub observed_at_utc: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconciliationRunSummary {
    pub run_id: String,
    pub window_started_at_utc: String,
    pub window_ended_at_utc: String,
    pub compared_records: i64,
    pub mismatch_count: i64,
    pub mismatch_rate: f64,
    pub critical_halt: bool,
    pub status: ReconciliationRunStatus,
    pub reason_code: String,
    pub evaluated_at_utc: String,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconciliationRunResult {
    pub summary: ReconciliationRunSummary,
    pub diffs: Vec<ReconciliationDiffRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExposureSnapshot {
    pub snapshot_id: String,
    pub run_id: String,
    pub market_id: Option<String>,
    pub net_exposure: f64,
    pub gross_exposure: f64,
    pub open_order_count: i64,
    pub reason_code: String,
    pub captured_at_utc: String,
    pub correlation_id: String,
}

pub fn validate_reconciliation_order_record(
    record: &ReconciliationOrderRecord,
) -> Result<(), ReconciliationContractError> {
    let mut issues = Vec::new();
    validate_non_empty("order_id", &record.order_id, &mut issues);
    validate_non_empty("market_id", &record.market_id, &mut issues);
    validate_non_empty("lifecycle_state", &record.lifecycle_state, &mut issues);
    validate_non_empty("observed_at_utc", &record.observed_at_utc, &mut issues);
    validate_timestamp_utc("observed_at_utc", &record.observed_at_utc, &mut issues);
    validate_non_empty("correlation_id", &record.correlation_id, &mut issues);
    validate_optional_non_negative("quantity", record.quantity, &mut issues);
    validate_optional_non_negative("price", record.price, &mut issues);
    if issues.is_empty() {
        return Ok(());
    }
    Err(ReconciliationContractError {
        code: ReconciliationReasonCode::InvalidPayload.code(),
        message: "reconciliation order record payload failed validation".to_string(),
        field_errors: issues,
    })
}

pub fn validate_reconciliation_diff_record(
    diff: &ReconciliationDiffRecord,
) -> Result<(), ReconciliationContractError> {
    let mut issues = Vec::new();
    validate_non_empty("diff_id", &diff.diff_id, &mut issues);
    validate_non_empty("run_id", &diff.run_id, &mut issues);
    validate_non_empty("order_id", &diff.order_id, &mut issues);
    validate_non_empty("market_id", &diff.market_id, &mut issues);
    if let Err(error) = ReconciliationReasonCode::parse(&diff.reason_code) {
        issues.push(ReconciliationValidationIssue {
            field: "reason_code",
            code: error.code,
            message: error.message,
        });
    }
    validate_non_empty("observed_at_utc", &diff.observed_at_utc, &mut issues);
    validate_timestamp_utc("observed_at_utc", &diff.observed_at_utc, &mut issues);
    validate_non_empty("correlation_id", &diff.correlation_id, &mut issues);
    if issues.is_empty() {
        return Ok(());
    }
    Err(ReconciliationContractError {
        code: ReconciliationReasonCode::InvalidPayload.code(),
        message: "reconciliation diff payload failed validation".to_string(),
        field_errors: issues,
    })
}

pub fn validate_reconciliation_run_summary(
    summary: &ReconciliationRunSummary,
) -> Result<(), ReconciliationContractError> {
    let mut issues = Vec::new();
    validate_non_empty("run_id", &summary.run_id, &mut issues);
    validate_non_empty(
        "window_started_at_utc",
        &summary.window_started_at_utc,
        &mut issues,
    );
    validate_non_empty(
        "window_ended_at_utc",
        &summary.window_ended_at_utc,
        &mut issues,
    );
    validate_non_empty("evaluated_at_utc", &summary.evaluated_at_utc, &mut issues);
    validate_non_empty("correlation_id", &summary.correlation_id, &mut issues);
    validate_timestamp_utc(
        "window_started_at_utc",
        &summary.window_started_at_utc,
        &mut issues,
    );
    validate_timestamp_utc(
        "window_ended_at_utc",
        &summary.window_ended_at_utc,
        &mut issues,
    );
    validate_timestamp_utc("evaluated_at_utc", &summary.evaluated_at_utc, &mut issues);

    if summary.compared_records <= 0 {
        issues.push(ReconciliationValidationIssue {
            field: "compared_records",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: "compared_records must be greater than zero".to_string(),
        });
    }
    if summary.mismatch_count < 0 {
        issues.push(ReconciliationValidationIssue {
            field: "mismatch_count",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: "mismatch_count cannot be negative".to_string(),
        });
    }
    if summary.mismatch_count > summary.compared_records {
        issues.push(ReconciliationValidationIssue {
            field: "mismatch_count",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: "mismatch_count cannot exceed compared_records".to_string(),
        });
    }
    if !summary.mismatch_rate.is_finite() || !(0.0..=1.0).contains(&summary.mismatch_rate) {
        issues.push(ReconciliationValidationIssue {
            field: "mismatch_rate",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: "mismatch_rate must be a finite ratio between 0 and 1".to_string(),
        });
    } else if summary.compared_records > 0 {
        let expected = summary.mismatch_count as f64 / summary.compared_records as f64;
        if (summary.mismatch_rate - expected).abs() > 1e-12 {
            issues.push(ReconciliationValidationIssue {
                field: "mismatch_rate",
                code: ReconciliationReasonCode::InvalidPayload.code(),
                message: "mismatch_rate must equal mismatch_count/compared_records".to_string(),
            });
        }
    }
    if let Err(error) = ReconciliationReasonCode::parse(&summary.reason_code) {
        issues.push(ReconciliationValidationIssue {
            field: "reason_code",
            code: error.code,
            message: error.message,
        });
    }

    let inferred_critical = summary.mismatch_rate > RECONCILIATION_CRITICAL_MISMATCH_THRESHOLD;
    if summary.critical_halt != inferred_critical {
        issues.push(ReconciliationValidationIssue {
            field: "critical_halt",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: format!(
                "critical_halt must reflect mismatch threshold semantics (> {}%)",
                RECONCILIATION_CRITICAL_MISMATCH_THRESHOLD * 100.0
            ),
        });
    }
    if summary.critical_halt && summary.status != ReconciliationRunStatus::CriticalHalt {
        issues.push(ReconciliationValidationIssue {
            field: "status",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: "critical reconciliation runs must use `critical_halt` status".to_string(),
        });
    }
    if !summary.critical_halt
        && summary.status == ReconciliationRunStatus::CriticalHalt
        && summary.mismatch_count <= summary.compared_records
    {
        issues.push(ReconciliationValidationIssue {
            field: "status",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: "non-critical reconciliation runs cannot use `critical_halt` status"
                .to_string(),
        });
    }

    if issues.is_empty() {
        return Ok(());
    }
    Err(ReconciliationContractError {
        code: ReconciliationReasonCode::InvalidPayload.code(),
        message: "reconciliation run summary failed validation".to_string(),
        field_errors: issues,
    })
}

pub fn validate_exposure_snapshot(
    snapshot: &ExposureSnapshot,
) -> Result<(), ReconciliationContractError> {
    let mut issues = Vec::new();
    validate_non_empty("snapshot_id", &snapshot.snapshot_id, &mut issues);
    validate_non_empty("run_id", &snapshot.run_id, &mut issues);
    if let Some(market_id) = snapshot.market_id.as_deref() {
        validate_non_empty("market_id", market_id, &mut issues);
    }
    validate_non_empty("captured_at_utc", &snapshot.captured_at_utc, &mut issues);
    validate_timestamp_utc("captured_at_utc", &snapshot.captured_at_utc, &mut issues);
    validate_non_empty("correlation_id", &snapshot.correlation_id, &mut issues);
    if let Err(error) = ReconciliationReasonCode::parse(&snapshot.reason_code) {
        issues.push(ReconciliationValidationIssue {
            field: "reason_code",
            code: error.code,
            message: error.message,
        });
    }
    validate_non_negative("gross_exposure", snapshot.gross_exposure, &mut issues);
    if !snapshot.net_exposure.is_finite() {
        issues.push(ReconciliationValidationIssue {
            field: "net_exposure",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: "net_exposure must be finite".to_string(),
        });
    }
    if snapshot.open_order_count < 0 {
        issues.push(ReconciliationValidationIssue {
            field: "open_order_count",
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: "open_order_count cannot be negative".to_string(),
        });
    }
    if issues.is_empty() {
        return Ok(());
    }
    Err(ReconciliationContractError {
        code: ReconciliationReasonCode::InvalidPayload.code(),
        message: "exposure snapshot payload failed validation".to_string(),
        field_errors: issues,
    })
}

pub fn calculate_mismatch_rate(
    mismatch_count: i64,
    compared_records: i64,
) -> Result<f64, ReconciliationContractError> {
    if compared_records <= 0 {
        return Err(ReconciliationContractError::window_unavailable(
            "cannot reconcile an empty internal/venue window",
        ));
    }
    if mismatch_count < 0 || mismatch_count > compared_records {
        return Err(ReconciliationContractError::invalid_payload(
            "mismatch_count must be between 0 and compared_records",
        ));
    }
    Ok(mismatch_count as f64 / compared_records as f64)
}

pub fn mismatch_rate_triggers_halt(
    mismatch_rate: f64,
) -> Result<bool, ReconciliationContractError> {
    if !mismatch_rate.is_finite() || !(0.0..=1.0).contains(&mismatch_rate) {
        return Err(ReconciliationContractError::invalid_payload(
            "mismatch_rate must be a finite ratio between 0 and 1",
        ));
    }
    Ok(mismatch_rate > RECONCILIATION_CRITICAL_MISMATCH_THRESHOLD)
}

pub fn reconcile_window(
    run_id: &str,
    window_started_at_utc: &str,
    window_ended_at_utc: &str,
    evaluated_at_utc: &str,
    correlation_id: &str,
    internal_window: &[ReconciliationOrderRecord],
    venue_window: &[ReconciliationOrderRecord],
) -> Result<ReconciliationRunResult, ReconciliationContractError> {
    if internal_window.is_empty() && venue_window.is_empty() {
        return Err(ReconciliationContractError::window_unavailable(
            "internal and venue reconciliation windows are both empty",
        ));
    }
    validate_non_empty_or_error("run_id", run_id)?;
    validate_non_empty_or_error("window_started_at_utc", window_started_at_utc)?;
    validate_non_empty_or_error("window_ended_at_utc", window_ended_at_utc)?;
    validate_non_empty_or_error("evaluated_at_utc", evaluated_at_utc)?;
    validate_non_empty_or_error("correlation_id", correlation_id)?;
    validate_timestamp_or_error("window_started_at_utc", window_started_at_utc)?;
    validate_timestamp_or_error("window_ended_at_utc", window_ended_at_utc)?;
    validate_timestamp_or_error("evaluated_at_utc", evaluated_at_utc)?;

    let start = parse_timestamp_or_error("window_started_at_utc", window_started_at_utc)?;
    let end = parse_timestamp_or_error("window_ended_at_utc", window_ended_at_utc)?;
    if end < start {
        return Err(ReconciliationContractError::invalid_payload(
            "window_ended_at_utc must be greater than or equal to window_started_at_utc",
        ));
    }

    let normalized_run_id = run_id.trim().to_ascii_lowercase();
    let internal_records = normalize_window_records("internal_window", internal_window)?;
    let venue_records = normalize_window_records("venue_window", venue_window)?;

    let compared_ids: BTreeSet<String> = internal_records
        .keys()
        .chain(venue_records.keys())
        .cloned()
        .collect();
    let compared_records = i64::try_from(compared_ids.len()).map_err(|_| {
        ReconciliationContractError::invalid_payload(
            "compared record count exceeds supported reconciliation limits",
        )
    })?;

    let mut diffs = Vec::new();
    for order_id in compared_ids {
        let internal = internal_records.get(&order_id);
        let venue = venue_records.get(&order_id);
        if let Some(diff) = build_diff(&normalized_run_id, &order_id, internal, venue)? {
            diffs.push(diff);
        }
    }
    diffs.sort_by(|left, right| {
        left.order_id
            .cmp(&right.order_id)
            .then_with(|| left.diff_class.cmp(&right.diff_class))
            .then_with(|| left.diff_id.cmp(&right.diff_id))
    });

    let mismatch_count = i64::try_from(diffs.len()).map_err(|_| {
        ReconciliationContractError::invalid_payload("mismatch count exceeds supported limits")
    })?;
    let mismatch_rate = calculate_mismatch_rate(mismatch_count, compared_records)?;
    let critical_halt = mismatch_rate_triggers_halt(mismatch_rate)?;
    let status = if critical_halt {
        ReconciliationRunStatus::CriticalHalt
    } else {
        ReconciliationRunStatus::Succeeded
    };
    let reason_code = if mismatch_count == 0 {
        ReconciliationReasonCode::Matched.code().to_string()
    } else if critical_halt {
        ReconciliationReasonCode::CriticalMismatch
            .code()
            .to_string()
    } else {
        ReconciliationReasonCode::NonCriticalMismatch
            .code()
            .to_string()
    };

    let summary = ReconciliationRunSummary {
        run_id: normalized_run_id,
        window_started_at_utc: window_started_at_utc.trim().to_string(),
        window_ended_at_utc: window_ended_at_utc.trim().to_string(),
        compared_records,
        mismatch_count,
        mismatch_rate,
        critical_halt,
        status,
        reason_code,
        evaluated_at_utc: evaluated_at_utc.trim().to_string(),
        correlation_id: correlation_id.trim().to_string(),
    };
    validate_reconciliation_run_summary(&summary)?;

    Ok(ReconciliationRunResult { summary, diffs })
}

pub fn build_exposure_snapshot(
    snapshot_id: &str,
    run_id: &str,
    market_id: Option<&str>,
    records: &[ReconciliationOrderRecord],
    reason_code: ReconciliationReasonCode,
    captured_at_utc: &str,
    correlation_id: &str,
) -> Result<ExposureSnapshot, ReconciliationContractError> {
    validate_non_empty_or_error("snapshot_id", snapshot_id)?;
    validate_non_empty_or_error("run_id", run_id)?;
    validate_non_empty_or_error("captured_at_utc", captured_at_utc)?;
    validate_non_empty_or_error("correlation_id", correlation_id)?;
    validate_timestamp_or_error("captured_at_utc", captured_at_utc)?;

    let normalized_market_id = market_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let filtered = records.iter().filter(|record| {
        normalized_market_id
            .as_deref()
            .map(|market| record.market_id == market)
            .unwrap_or(true)
    });

    let mut net_exposure = 0.0;
    let mut gross_exposure = 0.0;
    let mut open_order_count = 0_i64;
    for record in filtered {
        validate_reconciliation_order_record(record)?;
        let quantity = record.quantity.unwrap_or(0.0);
        net_exposure += quantity;
        gross_exposure += quantity.abs();
        if !is_terminal_lifecycle_state(&record.lifecycle_state) {
            open_order_count += 1;
        }
    }

    let snapshot = ExposureSnapshot {
        snapshot_id: snapshot_id.trim().to_string(),
        run_id: run_id.trim().to_ascii_lowercase(),
        market_id: normalized_market_id,
        net_exposure,
        gross_exposure,
        open_order_count,
        reason_code: reason_code.code().to_string(),
        captured_at_utc: captured_at_utc.trim().to_string(),
        correlation_id: correlation_id.trim().to_string(),
    };
    validate_exposure_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn authorize_reconciliation_read(role: &str) -> Result<(), ReconciliationContractError> {
    let role = GovernanceRole::parse(role)
        .map_err(|error| ReconciliationContractError::unauthorized(error.message))?;
    let matrix = RolePermissionMatrix::canonical();
    let allowed = matrix
        .has_permission(role, GovernancePermission::ReadAnalytics)
        .map_err(|error| ReconciliationContractError::unauthorized(error.message))?;
    if !allowed {
        return Err(ReconciliationContractError::unauthorized(
            "role does not permit reconciliation/exposure read access",
        ));
    }
    Ok(())
}

fn normalize_window_records(
    field: &'static str,
    records: &[ReconciliationOrderRecord],
) -> Result<BTreeMap<String, ReconciliationOrderRecord>, ReconciliationContractError> {
    let mut normalized = BTreeMap::new();
    for record in records {
        validate_reconciliation_order_record(record)?;
        if normalized
            .insert(record.order_id.clone(), record.clone())
            .is_some()
        {
            return Err(ReconciliationContractError::invalid_payload(format!(
                "{field} contains duplicate order_id `{}`",
                record.order_id
            )));
        }
    }
    Ok(normalized)
}

fn build_diff(
    run_id: &str,
    order_id: &str,
    internal: Option<&ReconciliationOrderRecord>,
    venue: Option<&ReconciliationOrderRecord>,
) -> Result<Option<ReconciliationDiffRecord>, ReconciliationContractError> {
    let Some(diff_details) = classify_diff(order_id, internal, venue) else {
        return Ok(None);
    };

    let diff = ReconciliationDiffRecord {
        diff_id: format!(
            "diff::{run_id}::{}::{}",
            order_id.trim().to_ascii_lowercase(),
            diff_details.diff_class.as_str()
        ),
        run_id: run_id.to_string(),
        order_id: order_id.to_string(),
        market_id: diff_details.market_id,
        diff_class: diff_details.diff_class,
        reason_code: ReconciliationReasonCode::NonCriticalMismatch
            .code()
            .to_string(),
        internal_value: diff_details.internal_value,
        venue_value: diff_details.venue_value,
        observed_at_utc: diff_details.observed_at_utc,
        correlation_id: diff_details.correlation_id,
    };
    validate_reconciliation_diff_record(&diff)?;
    Ok(Some(diff))
}

struct DiffDetails {
    diff_class: ReconciliationDiffClass,
    market_id: String,
    internal_value: Option<String>,
    venue_value: Option<String>,
    observed_at_utc: String,
    correlation_id: String,
}

fn classify_diff(
    order_id: &str,
    internal: Option<&ReconciliationOrderRecord>,
    venue: Option<&ReconciliationOrderRecord>,
) -> Option<DiffDetails> {
    match (internal, venue) {
        (None, None) => None,
        (None, Some(venue_record)) => Some(DiffDetails {
            diff_class: ReconciliationDiffClass::MissingInternalRecord,
            market_id: venue_record.market_id.clone(),
            internal_value: None,
            venue_value: Some(compact_order_record(venue_record)),
            observed_at_utc: venue_record.observed_at_utc.clone(),
            correlation_id: venue_record.correlation_id.clone(),
        }),
        (Some(internal_record), None) => Some(DiffDetails {
            diff_class: ReconciliationDiffClass::MissingVenueRecord,
            market_id: internal_record.market_id.clone(),
            internal_value: Some(compact_order_record(internal_record)),
            venue_value: None,
            observed_at_utc: internal_record.observed_at_utc.clone(),
            correlation_id: internal_record.correlation_id.clone(),
        }),
        (Some(internal_record), Some(venue_record)) => {
            if internal_record.market_id != venue_record.market_id {
                return Some(DiffDetails {
                    diff_class: ReconciliationDiffClass::MarketMismatch,
                    market_id: internal_record.market_id.clone(),
                    internal_value: Some(internal_record.market_id.clone()),
                    venue_value: Some(venue_record.market_id.clone()),
                    observed_at_utc: internal_record.observed_at_utc.clone(),
                    correlation_id: internal_record.correlation_id.clone(),
                });
            }
            if internal_record.lifecycle_state != venue_record.lifecycle_state {
                return Some(DiffDetails {
                    diff_class: ReconciliationDiffClass::LifecycleStateMismatch,
                    market_id: internal_record.market_id.clone(),
                    internal_value: Some(internal_record.lifecycle_state.clone()),
                    venue_value: Some(venue_record.lifecycle_state.clone()),
                    observed_at_utc: internal_record.observed_at_utc.clone(),
                    correlation_id: internal_record.correlation_id.clone(),
                });
            }
            if internal_record.quantity != venue_record.quantity {
                return Some(DiffDetails {
                    diff_class: ReconciliationDiffClass::QuantityMismatch,
                    market_id: internal_record.market_id.clone(),
                    internal_value: Some(format_optional_quantity(internal_record.quantity)),
                    venue_value: Some(format_optional_quantity(venue_record.quantity)),
                    observed_at_utc: internal_record.observed_at_utc.clone(),
                    correlation_id: internal_record.correlation_id.clone(),
                });
            }
            if internal_record.price != venue_record.price {
                return Some(DiffDetails {
                    diff_class: ReconciliationDiffClass::PriceMismatch,
                    market_id: internal_record.market_id.clone(),
                    internal_value: Some(format_optional_quantity(internal_record.price)),
                    venue_value: Some(format_optional_quantity(venue_record.price)),
                    observed_at_utc: internal_record.observed_at_utc.clone(),
                    correlation_id: internal_record.correlation_id.clone(),
                });
            }

            let _ = order_id;
            None
        }
    }
}

fn compact_order_record(record: &ReconciliationOrderRecord) -> String {
    format!(
        "order_id={},market_id={},state={},quantity={},price={}",
        record.order_id,
        record.market_id,
        record.lifecycle_state,
        format_optional_quantity(record.quantity),
        format_optional_quantity(record.price)
    )
}

fn format_optional_quantity(value: Option<f64>) -> String {
    value
        .map(|number| format!("{number:.8}"))
        .unwrap_or_else(|| "none".to_string())
}

fn is_terminal_lifecycle_state(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "filled" | "canceled" | "expired"
    )
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
    issues: &mut Vec<ReconciliationValidationIssue>,
) {
    if value.trim().is_empty() {
        issues.push(ReconciliationValidationIssue {
            field,
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: format!("{field} cannot be blank"),
        });
    }
}

fn validate_non_negative(
    field: &'static str,
    value: f64,
    issues: &mut Vec<ReconciliationValidationIssue>,
) {
    if !value.is_finite() || value < 0.0 {
        issues.push(ReconciliationValidationIssue {
            field,
            code: ReconciliationReasonCode::InvalidPayload.code(),
            message: format!("{field} must be finite and >= 0"),
        });
    }
}

fn validate_optional_non_negative(
    field: &'static str,
    value: Option<f64>,
    issues: &mut Vec<ReconciliationValidationIssue>,
) {
    if let Some(number) = value {
        validate_non_negative(field, number, issues);
    }
}

fn validate_timestamp_utc(
    field: &'static str,
    value: &str,
    issues: &mut Vec<ReconciliationValidationIssue>,
) {
    match OffsetDateTime::parse(value, &Rfc3339) {
        Ok(parsed) => {
            if parsed.offset() != UtcOffset::UTC {
                issues.push(ReconciliationValidationIssue {
                    field,
                    code: ReconciliationReasonCode::InvalidPayload.code(),
                    message: format!("{field} must use UTC `Z` offset"),
                });
            }
        }
        Err(error) => {
            issues.push(ReconciliationValidationIssue {
                field,
                code: ReconciliationReasonCode::InvalidPayload.code(),
                message: format!("{field} must be an RFC3339 UTC timestamp: {error}"),
            });
        }
    }
}

fn validate_non_empty_or_error(
    field: &'static str,
    value: &str,
) -> Result<(), ReconciliationContractError> {
    if value.trim().is_empty() {
        return Err(ReconciliationContractError::invalid_payload(format!(
            "{field} cannot be blank"
        )));
    }
    Ok(())
}

fn validate_timestamp_or_error(
    field: &'static str,
    value: &str,
) -> Result<(), ReconciliationContractError> {
    parse_timestamp_or_error(field, value).map(|_| ())
}

fn parse_timestamp_or_error(
    field: &'static str,
    value: &str,
) -> Result<OffsetDateTime, ReconciliationContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        ReconciliationContractError::invalid_payload(format!(
            "{field} must be an RFC3339 UTC timestamp: {error}"
        ))
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(ReconciliationContractError::invalid_payload(format!(
            "{field} must use UTC `Z` offset"
        )));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_order(
        order_id: &str,
        state: &str,
        quantity: Option<f64>,
        price: Option<f64>,
    ) -> ReconciliationOrderRecord {
        ReconciliationOrderRecord {
            order_id: order_id.to_string(),
            market_id: "market-1".to_string(),
            lifecycle_state: state.to_string(),
            quantity,
            price,
            observed_at_utc: "2026-04-06T00:00:00Z".to_string(),
            correlation_id: "corr-1".to_string(),
        }
    }

    #[test]
    fn reconciliation_run_is_deterministic_for_identical_windows() {
        let internal = vec![
            sample_order("order-a", "live", Some(1.0), Some(0.42)),
            sample_order("order-b", "live", Some(2.0), Some(0.55)),
            sample_order("order-c", "pending", Some(3.0), Some(0.77)),
        ];
        let mut venue = internal.clone();
        venue[1].lifecycle_state = "filled".to_string();
        let _ = venue.pop();
        venue.push(sample_order("order-d", "live", Some(4.0), Some(0.11)));

        let first = reconcile_window(
            "run-1",
            "2026-04-06T00:00:00Z",
            "2026-04-06T00:01:00Z",
            "2026-04-06T00:01:01Z",
            "corr-1",
            &internal,
            &venue,
        )
        .expect("reconciliation should succeed");
        let second = reconcile_window(
            "run-1",
            "2026-04-06T00:00:00Z",
            "2026-04-06T00:01:00Z",
            "2026-04-06T00:01:01Z",
            "corr-1",
            &internal,
            &venue,
        )
        .expect("reconciliation should be repeatable");

        assert_eq!(first.summary, second.summary);
        assert_eq!(first.diffs, second.diffs);
        assert_eq!(first.diffs.len(), 3);
        assert_eq!(first.diffs[0].order_id, "order-b");
        assert_eq!(
            first.diffs[0].diff_class,
            ReconciliationDiffClass::LifecycleStateMismatch
        );
        assert_eq!(first.diffs[1].order_id, "order-c");
        assert_eq!(
            first.diffs[1].diff_class,
            ReconciliationDiffClass::MissingVenueRecord
        );
        assert_eq!(first.diffs[2].order_id, "order-d");
        assert_eq!(
            first.diffs[2].diff_class,
            ReconciliationDiffClass::MissingInternalRecord
        );
    }

    #[test]
    fn threshold_boundary_is_deterministic_for_point_one_percent() {
        let mut internal = Vec::new();
        let mut venue = Vec::new();
        for index in 0..1000 {
            internal.push(sample_order(
                &format!("order-{index}"),
                "live",
                Some(1.0),
                Some(0.5),
            ));
            venue.push(sample_order(
                &format!("order-{index}"),
                "live",
                Some(1.0),
                Some(0.5),
            ));
        }
        venue[0].lifecycle_state = "filled".to_string();

        let boundary = reconcile_window(
            "run-boundary",
            "2026-04-06T00:00:00Z",
            "2026-04-06T00:01:00Z",
            "2026-04-06T00:01:01Z",
            "corr-boundary",
            &internal,
            &venue,
        )
        .expect("boundary run should succeed");
        assert_eq!(boundary.summary.mismatch_count, 1);
        assert!(!boundary.summary.critical_halt);
        assert_eq!(boundary.summary.status, ReconciliationRunStatus::Succeeded);
        assert_eq!(
            boundary.summary.reason_code,
            ReconciliationReasonCode::NonCriticalMismatch.code()
        );

        venue[1].lifecycle_state = "filled".to_string();
        let breached = reconcile_window(
            "run-breached",
            "2026-04-06T00:00:00Z",
            "2026-04-06T00:01:00Z",
            "2026-04-06T00:01:01Z",
            "corr-breached",
            &internal,
            &venue,
        )
        .expect("breached run should succeed");
        assert_eq!(breached.summary.mismatch_count, 2);
        assert!(breached.summary.critical_halt);
        assert_eq!(
            breached.summary.status,
            ReconciliationRunStatus::CriticalHalt
        );
        assert_eq!(
            breached.summary.reason_code,
            ReconciliationReasonCode::CriticalMismatch.code()
        );
    }

    #[test]
    fn empty_windows_are_rejected_with_machine_reason_code() {
        let error = reconcile_window(
            "run-empty",
            "2026-04-06T00:00:00Z",
            "2026-04-06T00:01:00Z",
            "2026-04-06T00:01:01Z",
            "corr-empty",
            &[],
            &[],
        )
        .expect_err("empty windows should fail closed");
        assert_eq!(
            error.code,
            ReconciliationReasonCode::WindowUnavailable.code()
        );
    }

    #[test]
    fn exposure_snapshot_builder_keeps_halt_visibility_data() {
        let records = vec![
            sample_order("order-open", "live", Some(1.5), Some(0.4)),
            sample_order("order-terminal", "filled", Some(2.0), Some(0.6)),
        ];
        let snapshot = build_exposure_snapshot(
            "snapshot-1",
            "run-1",
            None,
            &records,
            ReconciliationReasonCode::CriticalMismatch,
            "2026-04-06T00:01:01Z",
            "corr-1",
        )
        .expect("snapshot should build");

        assert_eq!(snapshot.open_order_count, 1);
        assert_eq!(
            snapshot.reason_code,
            ReconciliationReasonCode::CriticalMismatch.code()
        );
        assert_eq!(snapshot.captured_at_utc, "2026-04-06T00:01:01Z");
    }

    #[test]
    fn unauthorized_read_access_fails_closed_with_machine_reason_code() {
        let error = authorize_reconciliation_read("guest")
            .expect_err("unknown role should be rejected fail-closed");
        assert_eq!(error.code, ReconciliationReasonCode::Unauthorized.code());
        assert!(authorize_reconciliation_read("read_only_analytics").is_ok());
    }

    #[test]
    fn run_summary_validation_rejects_inconsistent_ratio() {
        let summary = ReconciliationRunSummary {
            run_id: "run-1".to_string(),
            window_started_at_utc: "2026-04-06T00:00:00Z".to_string(),
            window_ended_at_utc: "2026-04-06T00:01:00Z".to_string(),
            compared_records: 10,
            mismatch_count: 1,
            mismatch_rate: 0.2,
            critical_halt: false,
            status: ReconciliationRunStatus::Succeeded,
            reason_code: ReconciliationReasonCode::NonCriticalMismatch
                .code()
                .to_string(),
            evaluated_at_utc: "2026-04-06T00:01:01Z".to_string(),
            correlation_id: "corr-1".to_string(),
        };
        let error = validate_reconciliation_run_summary(&summary)
            .expect_err("inconsistent mismatch ratio should be rejected");
        assert_eq!(error.code, ReconciliationReasonCode::InvalidPayload.code());
    }
}
