use crate::traceability::service::TraceabilityMappingRow;
use domain::coverage::{
    CoverageClass, CoverageContractError, CoverageMatrixRow, build_coverage_row,
};
use domain::traceability::{LinkConfidence, LinkOutcome};

pub fn classify_traceability_row(
    row: &TraceabilityMappingRow,
) -> Result<CoverageMatrixRow, CoverageContractError> {
    let coverage_class = classify_coverage_class(row);
    build_coverage_row(
        row.canonical_requirement_id.clone(),
        coverage_class,
        row.reason_code.clone(),
        row.rationale.clone(),
        row.code_anchors.clone(),
        row.test_anchors.clone(),
        row.ambiguous_candidates.clone(),
        row.provenance.clone(),
    )
}

fn classify_coverage_class(row: &TraceabilityMappingRow) -> CoverageClass {
    // match .*LinkOutcome
    match row.outcome {
        // LinkOutcome-driven coverage decision table
        LinkOutcome::MissingEvidence => CoverageClass::Missing,
        LinkOutcome::Ambiguous | LinkOutcome::StaleEvidence => CoverageClass::Partial,
        LinkOutcome::Linked => {
            let deterministic_high_or_medium = matches!(
                row.confidence,
                LinkConfidence::High | LinkConfidence::Medium
            ) && row.reason_code.trim()
                == "deterministic_anchor_match"
                && row.ambiguous_candidates.is_empty();

            if deterministic_high_or_medium {
                CoverageClass::Covered
            } else {
                CoverageClass::Partial
            }
        }
    }
}
