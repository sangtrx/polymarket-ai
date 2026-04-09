use domain::coverage::{CoverageClass, CoverageMatrixRow};
use domain::risk_prioritization::{
    RemediationFocus, RiskSeverity, score_unresolved_row,
};
use domain::traceability::EvidenceAnchor;
use serde::Serialize;
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RiskScoredRow {
    pub canonical_requirement_id: String,
    pub coverage_class: CoverageClass,
    pub severity: RiskSeverity,
    pub risk_score: i32,
    pub reason_code: String,
    pub rationale: String,
    pub provenance: String,
    pub code_anchor_count: usize,
    pub test_anchor_count: usize,
    pub ambiguous_anchor_count: usize,
    pub severity_weight: i32,
    pub reason_weight: i32,
    pub evidence_penalty: i32,
    pub remediation_focus: RemediationFocus,
    pub top_evidence_anchors: Vec<EvidenceAnchor>,
}

pub fn classify_coverage_row(row: &CoverageMatrixRow) -> Option<RiskScoredRow> {
    if matches!(row.coverage_class, CoverageClass::Covered) {
        return None;
    }

    let code_anchor_count = row.code_anchors.len();
    let test_anchor_count = row.test_anchors.len();
    let ambiguous_anchor_count = row.ambiguous_candidates.len();
    let breakdown = score_unresolved_row(
        &row.reason_code,
        code_anchor_count,
        test_anchor_count,
        ambiguous_anchor_count,
    );

    Some(RiskScoredRow {
        canonical_requirement_id: row.canonical_requirement_id.clone(),
        coverage_class: row.coverage_class.clone(),
        severity: breakdown.severity,
        risk_score: breakdown.risk_score,
        reason_code: breakdown.reason_code,
        rationale: row.rationale.clone(),
        provenance: row.provenance.clone(),
        code_anchor_count,
        test_anchor_count,
        ambiguous_anchor_count,
        severity_weight: breakdown.severity_weight,
        reason_weight: breakdown.reason_weight,
        evidence_penalty: breakdown.evidence_penalty,
        remediation_focus: breakdown.remediation_focus,
        top_evidence_anchors: top_evidence_anchors(row),
    })
}

pub fn classify_unresolved_rows(rows: &[CoverageMatrixRow]) -> Vec<RiskScoredRow> {
    rows.iter().filter_map(classify_coverage_row).collect()
}

fn top_evidence_anchors(row: &CoverageMatrixRow) -> Vec<EvidenceAnchor> {
    let mut anchors = row
        .code_anchors
        .iter()
        .chain(row.test_anchors.iter())
        .chain(row.ambiguous_candidates.iter())
        .cloned()
        .collect::<Vec<_>>();
    anchors.sort_by(anchor_order);
    anchors.into_iter().take(3).collect()
}

fn anchor_order(left: &EvidenceAnchor, right: &EvidenceAnchor) -> Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.symbol.cmp(&right.symbol))
        .then_with(|| left.section.cmp(&right.section))
        .then_with(|| left.line_start.cmp(&right.line_start))
        .then_with(|| left.line_end.cmp(&right.line_end))
}
