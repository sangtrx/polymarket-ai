use crate::coverage::classifier::classify_traceability_row;
use crate::traceability::service::TraceabilityMappingRow;
use domain::coverage::{CoverageClass, CoverageMatrixRow, build_coverage_row};
use persistence::postgres::coverage::insert_coverage_snapshot;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone)]
pub struct RunCoverageClassificationInput {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub repo_root: String,
    pub canonical_requirement_ids: Vec<String>,
    pub traceability_rows: Vec<TraceabilityMappingRow>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CoverageServiceError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CoverageMatrixResult {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub rows: Vec<CoverageMatrixRow>,
}

pub trait CoveragePersistencePort: Send + Sync {
    fn persist_coverage<'a>(
        &'a self,
        result: &'a CoverageMatrixResult,
    ) -> Pin<Box<dyn Future<Output = Result<(), CoverageServiceError>> + Send + 'a>>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryCoveragePersistence;

impl CoveragePersistencePort for InMemoryCoveragePersistence {
    fn persist_coverage<'a>(
        &'a self,
        _result: &'a CoverageMatrixResult,
    ) -> Pin<Box<dyn Future<Output = Result<(), CoverageServiceError>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }
}

#[derive(Clone)]
pub struct PostgresCoveragePersistence {
    pool: PgPool,
}

impl PostgresCoveragePersistence {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl CoveragePersistencePort for PostgresCoveragePersistence {
    fn persist_coverage<'a>(
        &'a self,
        result: &'a CoverageMatrixResult,
    ) -> Pin<Box<dyn Future<Output = Result<(), CoverageServiceError>> + Send + 'a>> {
        Box::pin(async move {
            let mut connection =
                self.pool
                    .acquire()
                    .await
                    .map_err(|error| CoverageServiceError {
                        code: "coverage_query_failed".to_string(),
                        message: format!("unable to acquire postgres connection: {error}"),
                    })?;
            insert_coverage_snapshot(
                &mut connection,
                &result.snapshot_id,
                &result.commit_sha,
                &result.generated_at_utc,
                &result.rows,
            )
            .await
            .map_err(|error| CoverageServiceError {
                code: error.code.to_string(),
                message: error.message,
            })?;
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct CoverageService {
    persistence: Arc<dyn CoveragePersistencePort>,
}

impl CoverageService {
    pub fn new(persistence: Arc<dyn CoveragePersistencePort>) -> Self {
        Self { persistence }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(InMemoryCoveragePersistence))
    }

    pub async fn run_coverage_classification(
        &self,
        input: RunCoverageClassificationInput,
    ) -> Result<CoverageMatrixResult, CoverageServiceError> {
        validate_payload(&input)?;

        let mut traceability_by_requirement = BTreeMap::new();
        for row in input.traceability_rows {
            let requirement_id = row.canonical_requirement_id.trim().to_string();
            if traceability_by_requirement
                .insert(requirement_id.clone(), row)
                .is_some()
            {
                return Err(CoverageServiceError {
                    code: "coverage_invalid_payload".to_string(),
                    message: format!(
                        "traceability_rows must not contain duplicate canonical_requirement_id: {requirement_id}"
                    ),
                });
            }
        }

        let mut rows = input
            .canonical_requirement_ids
            .iter()
            .map(|requirement_id| {
                if let Some(traceability_row) = traceability_by_requirement.remove(requirement_id) {
                    return classify_traceability_row(&traceability_row).map_err(|error| {
                        CoverageServiceError {
                            code: error.code,
                            message: error.message,
                        }
                    });
                }

                build_coverage_row(
                    requirement_id.clone(),
                    CoverageClass::Missing,
                    "missing_evidence",
                    "No qualifying traceability row was found for this canonical requirement",
                    vec![],
                    vec![],
                    vec![],
                    "coverage_baseline_fill",
                )
                .map_err(|error| CoverageServiceError {
                    code: error.code,
                    message: error.message,
                })
            })
            .collect::<Result<Vec<CoverageMatrixRow>, CoverageServiceError>>()?;

        rows.sort_by(|left, right| {
            left.canonical_requirement_id
                .cmp(&right.canonical_requirement_id)
        });

        let result = CoverageMatrixResult {
            snapshot_id: input.snapshot_id,
            commit_sha: input.commit_sha,
            generated_at_utc: input.generated_at_utc,
            rows,
        };

        validate_result_for_persistence(&result)?;
        self.persistence.persist_coverage(&result).await?;
        Ok(result)
    }
}

pub async fn run_coverage_classification(
    input: RunCoverageClassificationInput,
) -> Result<CoverageMatrixResult, CoverageServiceError> {
    CoverageService::in_memory()
        .run_coverage_classification(input)
        .await
}

fn validate_payload(input: &RunCoverageClassificationInput) -> Result<(), CoverageServiceError> {
    if input.snapshot_id.trim().is_empty() {
        return Err(CoverageServiceError {
            code: "coverage_invalid_payload".to_string(),
            message: "snapshot_id is required".to_string(),
        });
    }
    let commit_sha = input.commit_sha.trim();
    if commit_sha.len() < 6
        || commit_sha.len() > 64
        || !commit_sha
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(CoverageServiceError {
            code: "coverage_invalid_payload".to_string(),
            message: "commit_sha must be 6-64 hex characters".to_string(),
        });
    }
    let generated_at =
        OffsetDateTime::parse(input.generated_at_utc.trim(), &Rfc3339).map_err(|_| {
            CoverageServiceError {
                code: "coverage_invalid_payload".to_string(),
                message: "generated_at_utc must be RFC3339 UTC".to_string(),
            }
        })?;
    if generated_at.offset() != UtcOffset::UTC {
        return Err(CoverageServiceError {
            code: "coverage_invalid_payload".to_string(),
            message: "generated_at_utc must use Z offset".to_string(),
        });
    }
    if !Path::new(input.repo_root.trim()).is_dir() {
        return Err(CoverageServiceError {
            code: "coverage_invalid_payload".to_string(),
            message: "repo_root must reference an existing directory".to_string(),
        });
    }
    if input.canonical_requirement_ids.is_empty() {
        return Err(CoverageServiceError {
            code: "coverage_invalid_payload".to_string(),
            message: "canonical_requirement_ids are required".to_string(),
        });
    }

    let mut seen_ids = BTreeSet::new();
    for requirement_id in &input.canonical_requirement_ids {
        let normalized = requirement_id.trim();
        if normalized.is_empty() {
            return Err(CoverageServiceError {
                code: "coverage_invalid_payload".to_string(),
                message: "canonical_requirement_id is required".to_string(),
            });
        }
        if !seen_ids.insert(normalized.to_string()) {
            return Err(CoverageServiceError {
                code: "coverage_invalid_payload".to_string(),
                message: format!(
                    "canonical_requirement_ids must not contain duplicates: {normalized}"
                ),
            });
        }
    }

    Ok(())
}

fn validate_result_for_persistence(
    result: &CoverageMatrixResult,
) -> Result<(), CoverageServiceError> {
    for row in &result.rows {
        build_coverage_row(
            row.canonical_requirement_id.clone(),
            row.coverage_class.clone(),
            row.reason_code.clone(),
            row.rationale.clone(),
            row.code_anchors.clone(),
            row.test_anchors.clone(),
            row.ambiguous_candidates.clone(),
            row.provenance.clone(),
        )
        .map_err(|error| CoverageServiceError {
            code: error.code,
            message: error.message,
        })?;
    }
    Ok(())
}
