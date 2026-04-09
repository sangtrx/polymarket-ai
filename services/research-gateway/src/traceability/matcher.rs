use domain::traceability::{EvidenceAnchor, LinkConfidence, LinkOutcome};

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
    let fallback = input
        .semantic_candidates
        .first()
        .or_else(|| input.deterministic_candidates.first())
        .map(|candidate| candidate.anchor.clone());
    MatchResult {
        canonical_requirement_id: input.canonical_requirement_id,
        outcome: if input.previous_links.is_empty() {
            LinkOutcome::Linked
        } else {
            LinkOutcome::MissingEvidence
        },
        confidence: LinkConfidence::High,
        rationale: "stub matcher".to_string(),
        reason_code: "semantic_fallback_used".to_string(),
        code_anchors: fallback.clone().into_iter().collect(),
        test_anchors: vec![],
        ambiguous_candidates: fallback.into_iter().collect(),
        provenance: "stub".to_string(),
    }
}
