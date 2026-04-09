use crate::coverage::service::CoverageMatrixResult;
use crate::risk::classifier::{RiskScoredRow, classify_unresolved_rows};
use domain::coverage::CoverageClass;
use domain::risk_prioritization::{RemediationFocus, RiskSeverity};
use domain::traceability::EvidenceAnchor;
use persistence::postgres::risk_prioritization::{PersistedRiskRow, insert_risk_snapshot};
use serde::Serialize;
use sqlx::PgPool;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone)]
pub struct RunRiskPrioritizationInput {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub repo_root: String,
    pub coverage_matrix: CoverageMatrixResult,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RiskServiceError {
    pub code: String,
    pub message: String,
}

impl RiskServiceError {
    pub fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "risk_invalid_payload".to_string(),
            message: message.into(),
        }
    }

    pub fn unmapped_reason(message: impl Into<String>) -> Self {
        Self {
            code: "risk_weight_unmapped_reason".to_string(),
            message: message.into(),
        }
    }

    pub fn query_failed(message: impl Into<String>) -> Self {
        Self {
            code: "risk_query_failed".to_string(),
            message: message.into(),
        }
    }

    pub fn constraint_violation(message: impl Into<String>) -> Self {
        Self {
            code: "risk_constraint_violation".to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RiskFixItem {
    pub canonical_requirement_id: String,
    pub coverage_class: CoverageClass,
    pub severity: RiskSeverity,
    pub risk_score: i32,
    pub priority_rank: usize,
    pub reason_code: String,
    pub rationale: String,
    pub provenance: String,
    pub code_anchor_count: usize,
    pub test_anchor_count: usize,
    pub ambiguous_anchor_count: usize,
    pub top_evidence_anchors: Vec<EvidenceAnchor>,
    pub remediation_focus: RemediationFocus,
    pub severity_weight: i32,
    pub reason_weight: i32,
    pub evidence_penalty: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RiskPrioritizationResult {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub rows: Vec<RiskFixItem>,
}

pub trait RiskPersistencePort: Send + Sync {
    fn persist_risk_prioritization<'a>(
        &'a self,
        result: &'a RiskPrioritizationResult,
    ) -> Pin<Box<dyn Future<Output = Result<(), RiskServiceError>> + Send + 'a>>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryRiskPersistence;

impl RiskPersistencePort for InMemoryRiskPersistence {
    fn persist_risk_prioritization<'a>(
        &'a self,
        _result: &'a RiskPrioritizationResult,
    ) -> Pin<Box<dyn Future<Output = Result<(), RiskServiceError>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }
}

#[derive(Clone)]
pub struct PostgresRiskPersistence {
    pool: PgPool,
}

impl PostgresRiskPersistence {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl RiskPersistencePort for PostgresRiskPersistence {
    fn persist_risk_prioritization<'a>(
        &'a self,
        result: &'a RiskPrioritizationResult,
    ) -> Pin<Box<dyn Future<Output = Result<(), RiskServiceError>> + Send + 'a>> {
        Box::pin(async move {
            let mut connection =
                self.pool
                    .acquire()
                    .await
                    .map_err(|error| RiskServiceError::query_failed(format!(
                        "unable to acquire postgres connection: {error}"
                    )))?;
            let rows = result
                .rows
                .iter()
                .map(build_persisted_row)
                .collect::<Result<Vec<_>, _>>()?;
            insert_risk_snapshot(
                &mut connection,
                &result.snapshot_id,
                &result.commit_sha,
                &result.generated_at_utc,
                &rows,
            )
            .await
            .map_err(|error| RiskServiceError {
                code: error.code.to_string(),
                message: error.message,
            })?;
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct RiskPrioritizationService {
    persistence: Arc<dyn RiskPersistencePort>,
}

impl RiskPrioritizationService {
    pub fn new(persistence: Arc<dyn RiskPersistencePort>) -> Self {
        Self { persistence }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryRiskPersistence))
    }

    pub async fn run_risk_prioritization(
        &self,
        input: RunRiskPrioritizationInput,
    ) -> Result<RiskPrioritizationResult, RiskServiceError> {
        validate_payload(&input)?;
        let mut scored_rows = classify_unresolved_rows(&input.coverage_matrix.rows);
        sort_scored_rows(&mut scored_rows);

        let rows = scored_rows
            .into_iter()
            .enumerate()
            .map(|(index, scored_row)| to_fix_item(scored_row, index + 1))
            .collect::<Vec<_>>();
        let result = RiskPrioritizationResult {
            snapshot_id: input.snapshot_id,
            commit_sha: input.commit_sha,
            generated_at_utc: input.generated_at_utc,
            rows,
        };

        self.persistence.persist_risk_prioritization(&result).await?;
        Ok(result)
    }
}

pub async fn run_risk_prioritization(
    input: RunRiskPrioritizationInput,
) -> Result<RiskPrioritizationResult, RiskServiceError> {
    RiskPrioritizationService::in_memory()
        .run_risk_prioritization(input)
        .await
}

fn sort_scored_rows(rows: &mut [RiskScoredRow]) {
    rows.sort_by(|left, right| {
        severity_rank(right.severity)
            .cmp(&severity_rank(left.severity))
            .then_with(|| right.risk_score.cmp(&left.risk_score))
            .then_with(|| left.canonical_requirement_id.cmp(&right.canonical_requirement_id))
    });
}

fn severity_rank(severity: RiskSeverity) -> u8 {
    match severity {
        RiskSeverity::Critical => 4,
        RiskSeverity::High => 3,
        RiskSeverity::Medium => 2,
        RiskSeverity::Low => 1,
    }
}

fn to_fix_item(scored_row: RiskScoredRow, priority_rank: usize) -> RiskFixItem {
    RiskFixItem {
        canonical_requirement_id: scored_row.canonical_requirement_id,
        coverage_class: scored_row.coverage_class,
        severity: scored_row.severity,
        risk_score: scored_row.risk_score,
        priority_rank,
        reason_code: scored_row.reason_code,
        rationale: scored_row.rationale,
        provenance: scored_row.provenance,
        code_anchor_count: scored_row.code_anchor_count,
        test_anchor_count: scored_row.test_anchor_count,
        ambiguous_anchor_count: scored_row.ambiguous_anchor_count,
        top_evidence_anchors: scored_row.top_evidence_anchors,
        remediation_focus: scored_row.remediation_focus,
        severity_weight: scored_row.severity_weight,
        reason_weight: scored_row.reason_weight,
        evidence_penalty: scored_row.evidence_penalty,
    }
}

fn validate_payload(input: &RunRiskPrioritizationInput) -> Result<(), RiskServiceError> {
    if input.snapshot_id.trim().is_empty() {
        return Err(RiskServiceError::invalid_payload(
            "snapshot_id is required",
        ));
    }
    let commit_sha = input.commit_sha.trim();
    if commit_sha.len() < 6
        || commit_sha.len() > 64
        || !commit_sha
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(RiskServiceError::invalid_payload(
            "commit_sha must be 6-64 hex characters",
        ));
    }
    let generated_at = OffsetDateTime::parse(input.generated_at_utc.trim(), &Rfc3339)
        .map_err(|_| RiskServiceError::invalid_payload("generated_at_utc must be RFC3339 UTC"))?;
    if generated_at.offset() != UtcOffset::UTC {
        return Err(RiskServiceError::invalid_payload(
            "generated_at_utc must use Z offset",
        ));
    }
    if !Path::new(input.repo_root.trim()).is_dir() {
        return Err(RiskServiceError::invalid_payload(
            "repo_root must reference an existing directory",
        ));
    }
    Ok(())
}

fn build_persisted_row(item: &RiskFixItem) -> Result<PersistedRiskRow, RiskServiceError> {
    if item.canonical_requirement_id.trim().is_empty() {
        return Err(RiskServiceError::invalid_payload(
            "canonical_requirement_id is required",
        ));
    }
    Ok(PersistedRiskRow {
        canonical_requirement_id: item.canonical_requirement_id.clone(),
        coverage_class: item.coverage_class.clone(),
        severity: item.severity,
        risk_score: item.risk_score,
        priority_rank: item.priority_rank,
        reason_code: item.reason_code.clone(),
        rationale: item.rationale.clone(),
        provenance: item.provenance.clone(),
        code_anchor_count: item.code_anchor_count,
        test_anchor_count: item.test_anchor_count,
        ambiguous_anchor_count: item.ambiguous_anchor_count,
        top_evidence_anchors: item.top_evidence_anchors.clone(),
        remediation_focus: item.remediation_focus,
        severity_weight: item.severity_weight,
        reason_weight: item.reason_weight,
        evidence_penalty: item.evidence_penalty,
    })
}
