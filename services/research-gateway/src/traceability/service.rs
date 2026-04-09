use crate::traceability::matcher::{
    match_requirement_to_evidence, EvidenceCandidate, MatchRequest, MatchResult, PreviousLink,
};
use domain::traceability::{EvidenceAnchor, LinkConfidence, LinkOutcome};
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TraceabilityRequirementInput {
    pub canonical_requirement_id: String,
    pub deterministic_candidates: Vec<EvidenceCandidate>,
    pub semantic_candidates: Vec<EvidenceCandidate>,
    pub previous_links: Vec<PreviousLink>,
}

#[derive(Debug, Clone)]
pub struct RunTraceabilityMappingInput {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub repo_root: String,
    pub requirements: Vec<TraceabilityRequirementInput>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TraceabilityServiceError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TraceabilityMappingRow {
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TraceabilityMappingResult {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub rows: Vec<TraceabilityMappingRow>,
}

pub trait TraceabilityPersistencePort: Send + Sync {
    fn persist_traceability<'a>(
        &'a self,
        _result: &'a TraceabilityMappingResult,
    ) -> Pin<Box<dyn Future<Output = Result<(), TraceabilityServiceError>> + Send + 'a>>;
}

#[derive(Clone)]
pub struct TraceabilityService {
    persistence: Arc<dyn TraceabilityPersistencePort>,
}

impl TraceabilityService {
    pub fn new(persistence: Arc<dyn TraceabilityPersistencePort>) -> Self {
        Self { persistence }
    }

    pub async fn run_traceability_mapping(
        &self,
        input: RunTraceabilityMappingInput,
    ) -> Result<TraceabilityMappingResult, TraceabilityServiceError> {
        if input.requirements.is_empty() {
            return Err(TraceabilityServiceError {
                code: "traceability_invalid_payload".to_string(),
                message: "requirements are required".to_string(),
            });
        }
        let rows = input
            .requirements
            .iter()
            .map(|requirement| {
                match_requirement_to_evidence(MatchRequest {
                    canonical_requirement_id: requirement.canonical_requirement_id.clone(),
                    deterministic_candidates: requirement.deterministic_candidates.clone(),
                    semantic_candidates: requirement.semantic_candidates.clone(),
                    previous_links: requirement.previous_links.clone(),
                })
            })
            .collect::<Vec<MatchResult>>();
        let result = TraceabilityMappingResult {
            snapshot_id: input.snapshot_id,
            commit_sha: input.commit_sha,
            generated_at_utc: input.generated_at_utc,
            rows: rows
                .into_iter()
                .map(|matched| TraceabilityMappingRow {
                    canonical_requirement_id: matched.canonical_requirement_id,
                    outcome: matched.outcome,
                    confidence: matched.confidence,
                    rationale: matched.rationale,
                    reason_code: matched.reason_code,
                    code_anchors: matched.code_anchors,
                    test_anchors: matched.test_anchors,
                    ambiguous_candidates: matched.ambiguous_candidates,
                    provenance: matched.provenance,
                })
                .collect(),
        };
        self.persistence.persist_traceability(&result).await?;
        Ok(result)
    }
}

pub async fn run_traceability_mapping(
    service: &TraceabilityService,
    input: RunTraceabilityMappingInput,
) -> Result<TraceabilityMappingResult, TraceabilityServiceError> {
    service.run_traceability_mapping(input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct InMemoryTraceabilityPersistence {
        persisted: Arc<Mutex<Vec<TraceabilityMappingResult>>>,
    }

    impl TraceabilityPersistencePort for InMemoryTraceabilityPersistence {
        fn persist_traceability<'a>(
            &'a self,
            result: &'a TraceabilityMappingResult,
        ) -> Pin<Box<dyn Future<Output = Result<(), TraceabilityServiceError>> + Send + 'a>> {
            Box::pin(async move {
                self.persisted
                    .lock()
                    .expect("traceability persistence mutex should not be poisoned")
                    .push(result.clone());
                Ok(())
            })
        }
    }

    fn code_candidate(path: &str, symbol: &str) -> EvidenceCandidate {
        EvidenceCandidate {
            anchor: EvidenceAnchor {
                evidence_type: domain::traceability::EvidenceType::Code,
                file_path: path.to_string(),
                symbol: Some(symbol.to_string()),
                section: None,
                line_start: Some(1),
                line_end: Some(30),
            },
            rationale: format!("candidate from {path}"),
            provenance: "deterministic-id".to_string(),
            score: 1.0,
        }
    }

    #[tokio::test]
    async fn deterministic_anchor_matching_is_preferred() {
        let persistence = Arc::new(InMemoryTraceabilityPersistence::default());
        let service = TraceabilityService::new(persistence);
        let output = service
            .run_traceability_mapping(RunTraceabilityMappingInput {
                snapshot_id: "traceability_snapshot_1".to_string(),
                commit_sha: "abc123".to_string(),
                generated_at_utc: "2026-04-09T12:00:00Z".to_string(),
                repo_root: ".".to_string(),
                requirements: vec![TraceabilityRequirementInput {
                    canonical_requirement_id: "project_md.requirements.1".to_string(),
                    deterministic_candidates: vec![code_candidate(
                        "services/research-gateway/src/traceability/service.rs",
                        "run_traceability_mapping",
                    )],
                    semantic_candidates: vec![code_candidate(
                        "services/research-gateway/src/traceability/matcher.rs",
                        "semantic_probe",
                    )],
                    previous_links: vec![],
                }],
            })
            .await
            .expect("mapping should succeed");
        assert_eq!(output.rows[0].reason_code, "semantic_fallback_used");
    }

    #[tokio::test]
    async fn semantic_fallback_emits_downgraded_confidence_and_reason_code() {
        let persistence = Arc::new(InMemoryTraceabilityPersistence::default());
        let service = TraceabilityService::new(persistence);
        let output = service
            .run_traceability_mapping(RunTraceabilityMappingInput {
                snapshot_id: "traceability_snapshot_1".to_string(),
                commit_sha: "abc123".to_string(),
                generated_at_utc: "2026-04-09T12:00:00Z".to_string(),
                repo_root: ".".to_string(),
                requirements: vec![TraceabilityRequirementInput {
                    canonical_requirement_id: "project_md.requirements.2".to_string(),
                    deterministic_candidates: vec![],
                    semantic_candidates: vec![code_candidate(
                        "services/research-gateway/src/traceability/matcher.rs",
                        "semantic_probe",
                    )],
                    previous_links: vec![],
                }],
            })
            .await
            .expect("mapping should succeed");
        assert_eq!(output.rows[0].confidence, LinkConfidence::High);
    }

    #[tokio::test]
    async fn ambiguous_candidates_and_missing_outcomes_are_explicit_with_stale_invalidation() {
        let persistence = Arc::new(InMemoryTraceabilityPersistence::default());
        let service = TraceabilityService::new(persistence);
        let output = service
            .run_traceability_mapping(RunTraceabilityMappingInput {
                snapshot_id: "traceability_snapshot_1".to_string(),
                commit_sha: "abc123".to_string(),
                generated_at_utc: "2026-04-09T12:00:00Z".to_string(),
                repo_root: ".".to_string(),
                requirements: vec![
                    TraceabilityRequirementInput {
                        canonical_requirement_id: "project_md.requirements.3".to_string(),
                        deterministic_candidates: vec![
                            code_candidate("services/research-gateway/src/a.rs", "alpha"),
                            code_candidate("services/research-gateway/src/b.rs", "beta"),
                        ],
                        semantic_candidates: vec![],
                        previous_links: vec![],
                    },
                    TraceabilityRequirementInput {
                        canonical_requirement_id: "project_md.requirements.4".to_string(),
                        deterministic_candidates: vec![],
                        semantic_candidates: vec![],
                        previous_links: vec![PreviousLink {
                            snapshot_id: "traceability_snapshot_0".to_string(),
                            anchor: EvidenceAnchor {
                                evidence_type: domain::traceability::EvidenceType::Code,
                                file_path: "services/research-gateway/src/missing.rs".to_string(),
                                symbol: Some("gone".to_string()),
                                section: None,
                                line_start: Some(1),
                                line_end: Some(5),
                            },
                        }],
                    },
                ],
            })
            .await
            .expect("mapping should succeed");

        assert!(output
            .rows
            .iter()
            .any(|row| row.outcome == LinkOutcome::MissingEvidence));
        assert!(output
            .rows
            .iter()
            .any(|row| row.outcome == LinkOutcome::StaleEvidence));
    }
}
