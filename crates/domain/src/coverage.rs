use crate::traceability::EvidenceAnchor;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoverageContractError {
    pub code: String,
    pub message: String,
}

impl CoverageContractError {
    fn invalid_payload(message: impl Into<String>) -> Self {
        Self {
            code: "coverage_invalid_payload".to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverageClass {
    Covered,
    Partial,
    Missing,
}

impl TryFrom<&str> for CoverageClass {
    type Error = CoverageContractError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_ascii_lowercase().as_str() {
            "covered" => Ok(Self::Covered),
            "partial" => Ok(Self::Partial),
            "missing" => Ok(Self::Missing),
            _ => Err(CoverageContractError::invalid_payload(
                "class must be one of: covered|partial|missing",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoverageMatrixRow {
    pub canonical_requirement_id: String,
    #[serde(rename = "class")]
    pub coverage_class: CoverageClass,
    pub reason_code: String,
    pub rationale: String,
    pub code_anchors: Vec<EvidenceAnchor>,
    pub test_anchors: Vec<EvidenceAnchor>,
    pub ambiguous_candidates: Vec<EvidenceAnchor>,
    pub provenance: String,
}

#[allow(clippy::too_many_arguments)]
pub fn build_coverage_row(
    canonical_requirement_id: impl Into<String>,
    coverage_class: CoverageClass,
    reason_code: impl Into<String>,
    rationale: impl Into<String>,
    code_anchors: Vec<EvidenceAnchor>,
    test_anchors: Vec<EvidenceAnchor>,
    ambiguous_candidates: Vec<EvidenceAnchor>,
    provenance: impl Into<String>,
) -> Result<CoverageMatrixRow, CoverageContractError> {
    let canonical_requirement_id = canonical_requirement_id.into().trim().to_string();
    if canonical_requirement_id.is_empty() {
        return Err(CoverageContractError::invalid_payload(
            "canonical_requirement_id is required",
        ));
    }

    let reason_code = reason_code.into().trim().to_string();
    let rationale = rationale.into().trim().to_string();
    if matches!(
        coverage_class,
        CoverageClass::Partial | CoverageClass::Missing
    ) && (reason_code.is_empty() || rationale.is_empty())
    {
        return Err(CoverageContractError::invalid_payload(
            "partial/missing rows require non-empty reason_code and rationale",
        ));
    }

    if reason_code.is_empty() {
        return Err(CoverageContractError::invalid_payload(
            "reason_code is required",
        ));
    }
    if rationale.is_empty() {
        return Err(CoverageContractError::invalid_payload(
            "rationale is required",
        ));
    }

    let provenance = provenance.into().trim().to_string();
    if provenance.is_empty() {
        return Err(CoverageContractError::invalid_payload(
            "provenance is required",
        ));
    }

    let code_anchors: Result<Vec<EvidenceAnchor>, CoverageContractError> =
        code_anchors.into_iter().map(validate_anchor).collect();
    let test_anchors: Result<Vec<EvidenceAnchor>, CoverageContractError> =
        test_anchors.into_iter().map(validate_anchor).collect();
    let ambiguous_candidates: Result<Vec<EvidenceAnchor>, CoverageContractError> =
        ambiguous_candidates
            .into_iter()
            .map(validate_anchor)
            .collect();

    Ok(CoverageMatrixRow {
        canonical_requirement_id,
        coverage_class,
        reason_code,
        rationale,
        code_anchors: code_anchors?,
        test_anchors: test_anchors?,
        ambiguous_candidates: ambiguous_candidates?,
        provenance,
    })
}

fn validate_anchor(anchor: EvidenceAnchor) -> Result<EvidenceAnchor, CoverageContractError> {
    let file_path = anchor.file_path.trim().to_string();
    let symbol = anchor
        .symbol
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let section = anchor
        .section
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if file_path.is_empty() {
        return Err(CoverageContractError::invalid_payload(
            "anchor file_path is required",
        ));
    }
    if symbol.is_none() && section.is_none() && anchor.line_start.is_none() {
        return Err(CoverageContractError::invalid_payload(
            "anchor must include symbol, section, or line_start",
        ));
    }
    if anchor.line_end.is_some() && anchor.line_start.is_none() {
        return Err(CoverageContractError::invalid_payload(
            "line_end requires line_start",
        ));
    }
    if let (Some(start), Some(end)) = (anchor.line_start, anchor.line_end)
        && end < start
    {
        return Err(CoverageContractError::invalid_payload(
            "line_end must be greater than or equal to line_start",
        ));
    }

    Ok(EvidenceAnchor {
        evidence_type: anchor.evidence_type,
        file_path,
        symbol,
        section,
        line_start: anchor.line_start,
        line_end: anchor.line_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traceability::EvidenceType;

    fn code_anchor() -> EvidenceAnchor {
        EvidenceAnchor {
            evidence_type: EvidenceType::Code,
            file_path: "services/research-gateway/src/coverage/service.rs".to_string(),
            symbol: Some("run_coverage_classification".to_string()),
            section: None,
            line_start: Some(1),
            line_end: Some(40),
        }
    }

    #[test]
    fn parses_supported_coverage_classes() {
        assert_eq!(
            CoverageClass::try_from("covered"),
            Ok(CoverageClass::Covered)
        );
        assert_eq!(
            CoverageClass::try_from("partial"),
            Ok(CoverageClass::Partial)
        );
        assert_eq!(
            CoverageClass::try_from("missing"),
            Ok(CoverageClass::Missing)
        );
    }

    #[test]
    fn rejects_partial_or_missing_rows_without_reason_payload() {
        let error = build_coverage_row(
            "project_md.requirements.1",
            CoverageClass::Missing,
            "",
            "",
            vec![],
            vec![],
            vec![],
            "coverage_service",
        )
        .expect_err("missing reason payload must fail closed");
        assert_eq!(error.code, "coverage_invalid_payload");
    }

    #[test]
    fn preserves_separate_anchor_buckets() {
        let row = build_coverage_row(
            "project_md.requirements.1",
            CoverageClass::Partial,
            "semantic_fallback_used",
            "semantic fallback lowered confidence",
            vec![code_anchor()],
            vec![EvidenceAnchor {
                evidence_type: EvidenceType::Test,
                file_path: "tests/api/phase-3-coverage-classification.test.mjs".to_string(),
                symbol: Some("coverage contract".to_string()),
                section: None,
                line_start: Some(1),
                line_end: Some(30),
            }],
            vec![code_anchor()],
            "coverage_service",
        )
        .expect("valid row should be constructed");
        assert_eq!(row.code_anchors.len(), 1);
        assert_eq!(row.test_anchors.len(), 1);
        assert_eq!(row.ambiguous_candidates.len(), 1);
    }
}
