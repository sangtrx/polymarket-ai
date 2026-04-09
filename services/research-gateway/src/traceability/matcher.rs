use domain::traceability::{EvidenceAnchor, LinkConfidence, LinkOutcome};
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct EvidenceCandidate {
    pub anchor: EvidenceAnchor,
    pub rationale: String,
    pub provenance: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct PreviousLink {
    pub snapshot_id: String,
    pub anchor: EvidenceAnchor,
}

#[derive(Debug, Clone)]
pub struct MatchRequest {
    pub canonical_requirement_id: String,
    pub deterministic_candidates: Vec<EvidenceCandidate>,
    pub semantic_candidates: Vec<EvidenceCandidate>,
    pub previous_links: Vec<PreviousLink>,
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub canonical_requirement_id: String,
    pub outcome: LinkOutcome,
    pub confidence: LinkConfidence,
    pub rationale: String,
    pub reason_code: String,
    pub code_anchors: Vec<EvidenceAnchor>,
    pub test_anchors: Vec<EvidenceAnchor>,
    pub ambiguous_candidates: Vec<EvidenceAnchor>,
    pub provenance: String,
}

pub fn match_requirement_to_evidence(input: MatchRequest) -> MatchResult {
    let deterministic = normalize_candidates(input.deterministic_candidates);
    let semantic = normalize_candidates(input.semantic_candidates);

    let deterministic_code = filter_code(&deterministic);
    let deterministic_test = filter_test(&deterministic);

    if !deterministic_code.is_empty() {
        return build_match_result(
            input.canonical_requirement_id,
            deterministic_code,
            deterministic_test,
            LinkConfidence::High,
            "deterministic_anchor_match".to_string(),
            "Deterministic canonical requirement anchors were found in implementation artifacts"
                .to_string(),
            "deterministic_first".to_string(),
        );
    }

    let semantic_code = filter_code(&semantic);
    let semantic_test = filter_test(&semantic);
    if !semantic_code.is_empty() {
        return build_match_result(
            input.canonical_requirement_id,
            semantic_code,
            semantic_test,
            LinkConfidence::Low,
            "semantic_fallback_used".to_string(),
            "Semantic fallback used because deterministic anchors were unavailable".to_string(),
            "semantic_fallback".to_string(),
        );
    }

    if !input.previous_links.is_empty() {
        let mut anchors = input
            .previous_links
            .iter()
            .map(|previous| previous.anchor.clone())
            .collect::<Vec<_>>();
        anchors.sort_by(anchor_order);
        let provenance = input
            .previous_links
            .iter()
            .map(|previous| previous.snapshot_id.clone())
            .collect::<Vec<_>>()
            .join(",");
        return MatchResult {
            canonical_requirement_id: input.canonical_requirement_id,
            outcome: LinkOutcome::StaleEvidence,
            confidence: LinkConfidence::Low,
            rationale: "Previously linked evidence anchors are stale at the current snapshot".to_string(),
            reason_code: "stale_link_invalidated".to_string(),
            code_anchors: anchors,
            test_anchors: vec![],
            ambiguous_candidates: vec![],
            provenance: format!("historical_snapshot:{provenance}"),
        };
    }

    MatchResult {
        canonical_requirement_id: input.canonical_requirement_id,
        outcome: LinkOutcome::MissingEvidence,
        confidence: LinkConfidence::Low,
        rationale: "No qualifying code evidence anchors found for the canonical requirement".to_string(),
        reason_code: "missing_evidence".to_string(),
        code_anchors: vec![],
        test_anchors: vec![],
        ambiguous_candidates: vec![],
        provenance: "deterministic_first".to_string(),
    }
}

fn build_match_result(
    canonical_requirement_id: String,
    code_anchors: Vec<EvidenceAnchor>,
    test_anchors: Vec<EvidenceAnchor>,
    confidence: LinkConfidence,
    reason_code: String,
    rationale: String,
    provenance: String,
) -> MatchResult {
    let ambiguous_candidates = if code_anchors.len() > 1 {
        code_anchors.clone()
    } else {
        vec![]
    };
    let outcome = if ambiguous_candidates.is_empty() {
        LinkOutcome::Linked
    } else {
        LinkOutcome::Ambiguous
    };
    MatchResult {
        canonical_requirement_id,
        outcome,
        confidence,
        rationale,
        reason_code,
        code_anchors,
        test_anchors,
        ambiguous_candidates,
        provenance,
    }
}

fn normalize_candidates(mut candidates: Vec<EvidenceCandidate>) -> Vec<EvidenceCandidate> {
    candidates.sort_by(candidate_order);
    candidates
}

fn filter_code(candidates: &[EvidenceCandidate]) -> Vec<EvidenceAnchor> {
    candidates
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.anchor.evidence_type,
                domain::traceability::EvidenceType::Code
            )
        })
        .map(|candidate| candidate.anchor.clone())
        .collect()
}

fn filter_test(candidates: &[EvidenceCandidate]) -> Vec<EvidenceAnchor> {
    candidates
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.anchor.evidence_type,
                domain::traceability::EvidenceType::Test
            )
        })
        .map(|candidate| candidate.anchor.clone())
        .collect()
}

fn candidate_order(left: &EvidenceCandidate, right: &EvidenceCandidate) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.anchor.file_path.cmp(&right.anchor.file_path))
        .then_with(|| left.anchor.symbol.cmp(&right.anchor.symbol))
}

fn anchor_order(left: &EvidenceAnchor, right: &EvidenceAnchor) -> Ordering {
    left.file_path
        .cmp(&right.file_path)
        .then_with(|| left.symbol.cmp(&right.symbol))
}
