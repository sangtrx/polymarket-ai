pub mod classifier;
pub mod service;

#[cfg(test)]
mod tests {
    use super::classifier::classify_traceability_row;
    use super::service::{
        CoverageMatrixResult, CoveragePersistencePort, CoverageService, CoverageServiceError,
        RunCoverageClassificationInput, run_coverage_classification,
    };
    use crate::traceability::service::TraceabilityMappingRow;
    use domain::coverage::CoverageClass;
    use domain::traceability::{EvidenceAnchor, EvidenceType, LinkConfidence, LinkOutcome};
    use std::collections::BTreeMap;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingCoveragePersistence {
        persisted: Arc<Mutex<Vec<CoverageMatrixResult>>>,
        fail_with: Arc<Mutex<Option<CoverageServiceError>>>,
    }

    impl CoveragePersistencePort for RecordingCoveragePersistence {
        fn persist_coverage<'a>(
            &'a self,
            result: &'a CoverageMatrixResult,
        ) -> Pin<Box<dyn Future<Output = Result<(), CoverageServiceError>> + Send + 'a>> {
            Box::pin(async move {
                if let Some(error) = self
                    .fail_with
                    .lock()
                    .expect("mutex should not be poisoned")
                    .clone()
                {
                    return Err(error);
                }
                self.persisted
                    .lock()
                    .expect("mutex should not be poisoned")
                    .push(result.clone());
                Ok(())
            })
        }
    }

    fn code_anchor(symbol: &str) -> EvidenceAnchor {
        EvidenceAnchor {
            evidence_type: EvidenceType::Code,
            file_path: "services/research-gateway/src/coverage/service.rs".to_string(),
            symbol: Some(symbol.to_string()),
            section: None,
            line_start: Some(1),
            line_end: Some(20),
        }
    }

    fn test_anchor(symbol: &str) -> EvidenceAnchor {
        EvidenceAnchor {
            evidence_type: EvidenceType::Test,
            file_path: "tests/api/phase-3-coverage-classification.test.mjs".to_string(),
            symbol: Some(symbol.to_string()),
            section: None,
            line_start: Some(1),
            line_end: Some(20),
        }
    }

    fn traceability_row(
        requirement_id: &str,
        outcome: LinkOutcome,
        confidence: LinkConfidence,
        reason_code: &str,
        rationale: &str,
        ambiguous_candidates: Vec<EvidenceAnchor>,
    ) -> TraceabilityMappingRow {
        TraceabilityMappingRow {
            canonical_requirement_id: requirement_id.to_string(),
            outcome,
            confidence,
            rationale: rationale.to_string(),
            reason_code: reason_code.to_string(),
            code_anchors: vec![code_anchor("run_coverage_classification")],
            test_anchors: vec![test_anchor("coverage contract")],
            ambiguous_candidates,
            provenance: "traceability_service".to_string(),
        }
    }

    #[test]
    fn classifier_returns_exactly_one_class_per_row() {
        let covered_row = classify_traceability_row(&traceability_row(
            "project_md.requirements.1",
            LinkOutcome::Linked,
            LinkConfidence::High,
            "deterministic_anchor_match",
            "deterministic anchor match",
            vec![],
        ))
        .expect("covered row should classify");
        assert!(matches!(covered_row.coverage_class, CoverageClass::Covered));

        let partial_row = classify_traceability_row(&traceability_row(
            "project_md.requirements.2",
            LinkOutcome::Linked,
            LinkConfidence::Low,
            "semantic_fallback_used",
            "semantic fallback used",
            vec![],
        ))
        .expect("partial row should classify");
        assert!(matches!(partial_row.coverage_class, CoverageClass::Partial));

        let missing_row = classify_traceability_row(&traceability_row(
            "project_md.requirements.3",
            LinkOutcome::MissingEvidence,
            LinkConfidence::Low,
            "missing_evidence",
            "no qualifying evidence",
            vec![],
        ))
        .expect("missing row should classify");
        assert!(matches!(missing_row.coverage_class, CoverageClass::Missing));
    }

    #[test]
    fn classifier_marks_only_deterministic_high_or_medium_links_as_covered() {
        let covered = classify_traceability_row(&traceability_row(
            "project_md.requirements.1",
            LinkOutcome::Linked,
            LinkConfidence::Medium,
            "deterministic_anchor_match",
            "deterministic link",
            vec![],
        ))
        .expect("deterministic medium link should be covered");
        assert!(matches!(covered.coverage_class, CoverageClass::Covered));

        let semantic_partial = classify_traceability_row(&traceability_row(
            "project_md.requirements.1",
            LinkOutcome::Linked,
            LinkConfidence::Medium,
            "semantic_fallback_used",
            "semantic fallback",
            vec![],
        ))
        .expect("semantic fallback must stay partial");
        assert!(matches!(
            semantic_partial.coverage_class,
            CoverageClass::Partial
        ));

        let low_partial = classify_traceability_row(&traceability_row(
            "project_md.requirements.1",
            LinkOutcome::Linked,
            LinkConfidence::Low,
            "deterministic_anchor_match",
            "low-confidence deterministic candidate",
            vec![],
        ))
        .expect("low confidence must not be covered");
        assert!(matches!(low_partial.coverage_class, CoverageClass::Partial));
    }

    #[test]
    fn classifier_preserves_reason_lineage_for_partial_rows() {
        let ambiguous_partial = classify_traceability_row(&traceability_row(
            "project_md.requirements.2",
            LinkOutcome::Ambiguous,
            LinkConfidence::High,
            "ambiguous_multiple_candidates",
            "multiple deterministic candidates remained",
            vec![code_anchor("candidate_a"), code_anchor("candidate_b")],
        ))
        .expect("ambiguous row should classify partial");
        assert!(matches!(
            ambiguous_partial.coverage_class,
            CoverageClass::Partial
        ));
        assert_eq!(
            ambiguous_partial.reason_code,
            "ambiguous_multiple_candidates"
        );

        let stale_partial = classify_traceability_row(&traceability_row(
            "project_md.requirements.3",
            LinkOutcome::StaleEvidence,
            LinkConfidence::Low,
            "stale_link_invalidated",
            "previously linked anchor no longer matches snapshot",
            vec![],
        ))
        .expect("stale row should classify partial");
        assert!(matches!(
            stale_partial.coverage_class,
            CoverageClass::Partial
        ));
        assert_eq!(stale_partial.reason_code, "stale_link_invalidated");
    }

    #[test]
    fn classifier_marks_missing_outcomes_with_reason_and_rationale() {
        let missing = classify_traceability_row(&traceability_row(
            "project_md.requirements.4",
            LinkOutcome::MissingEvidence,
            LinkConfidence::Low,
            "missing_evidence",
            "no qualifying code evidence anchors found",
            vec![],
        ))
        .expect("missing rows should classify");
        assert!(matches!(missing.coverage_class, CoverageClass::Missing));
        assert_eq!(missing.reason_code, "missing_evidence");
        assert!(!missing.rationale.is_empty());
    }

    #[tokio::test]
    async fn classifies_all_requirements_and_sorts_rows_by_canonical_requirement_id() {
        let output = run_coverage_classification(RunCoverageClassificationInput {
            snapshot_id: "coverage_snapshot_1".to_string(),
            commit_sha: "abc123".to_string(),
            generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: ".".to_string(),
            canonical_requirement_ids: vec![
                "project_md.requirements.3".to_string(),
                "project_md.requirements.1".to_string(),
                "project_md.requirements.2".to_string(),
            ],
            traceability_rows: vec![
                traceability_row(
                    "project_md.requirements.2",
                    LinkOutcome::Linked,
                    LinkConfidence::Low,
                    "semantic_fallback_used",
                    "semantic fallback selected",
                    vec![],
                ),
                traceability_row(
                    "project_md.requirements.1",
                    LinkOutcome::Linked,
                    LinkConfidence::High,
                    "deterministic_anchor_match",
                    "deterministic anchor found",
                    vec![],
                ),
            ],
        })
        .await
        .expect("coverage classification should succeed");

        assert_eq!(output.snapshot_id, "coverage_snapshot_1");
        assert_eq!(output.commit_sha, "abc123");
        assert_eq!(output.generated_at_utc, "2026-04-09T00:00:00Z");
        assert_eq!(output.rows.len(), 3);
        assert_eq!(
            output
                .rows
                .iter()
                .map(|row| row.canonical_requirement_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "project_md.requirements.1",
                "project_md.requirements.2",
                "project_md.requirements.3"
            ]
        );

        let by_requirement = output
            .rows
            .into_iter()
            .map(|row| (row.canonical_requirement_id.clone(), row))
            .collect::<BTreeMap<_, _>>();
        assert!(matches!(
            by_requirement["project_md.requirements.1"].coverage_class,
            CoverageClass::Covered
        ));
        assert!(matches!(
            by_requirement["project_md.requirements.2"].coverage_class,
            CoverageClass::Partial
        ));
        assert!(matches!(
            by_requirement["project_md.requirements.3"].coverage_class,
            CoverageClass::Missing
        ));
        assert_eq!(
            by_requirement["project_md.requirements.3"].reason_code,
            "missing_evidence"
        );
    }

    #[tokio::test]
    async fn persists_coverage_snapshot_transactionally() {
        let persistence = RecordingCoveragePersistence::default();
        let service = CoverageService::new(Arc::new(persistence.clone()));
        let output = service
            .run_coverage_classification(RunCoverageClassificationInput {
                snapshot_id: "coverage_snapshot_2".to_string(),
                commit_sha: "abc123".to_string(),
                generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
                repo_root: ".".to_string(),
                canonical_requirement_ids: vec![
                    "project_md.requirements.1".to_string(),
                    "project_md.requirements.2".to_string(),
                ],
                traceability_rows: vec![traceability_row(
                    "project_md.requirements.1",
                    LinkOutcome::Linked,
                    LinkConfidence::High,
                    "deterministic_anchor_match",
                    "deterministic anchor found",
                    vec![],
                )],
            })
            .await
            .expect("classification and persistence should succeed");

        let persisted = persistence
            .persisted
            .lock()
            .expect("mutex should not be poisoned")
            .clone();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].snapshot_id, "coverage_snapshot_2");
        assert_eq!(persisted[0].rows, output.rows);
        assert_eq!(persisted[0].rows[1].reason_code, "missing_evidence");
    }

    #[tokio::test]
    async fn persistence_failures_return_machine_readable_errors() {
        let persistence = RecordingCoveragePersistence::default();
        *persistence
            .fail_with
            .lock()
            .expect("mutex should not be poisoned") = Some(CoverageServiceError {
            code: "coverage_query_failed".to_string(),
            message: "insert failed".to_string(),
        });
        let service = CoverageService::new(Arc::new(persistence));
        let error = service
            .run_coverage_classification(RunCoverageClassificationInput {
                snapshot_id: "coverage_snapshot_3".to_string(),
                commit_sha: "abc123".to_string(),
                generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
                repo_root: ".".to_string(),
                canonical_requirement_ids: vec!["project_md.requirements.1".to_string()],
                traceability_rows: vec![traceability_row(
                    "project_md.requirements.1",
                    LinkOutcome::Linked,
                    LinkConfidence::High,
                    "deterministic_anchor_match",
                    "deterministic anchor found",
                    vec![],
                )],
            })
            .await
            .expect_err("persistence failure should surface fail-closed error");
        assert_eq!(error.code, "coverage_query_failed");
    }
}
