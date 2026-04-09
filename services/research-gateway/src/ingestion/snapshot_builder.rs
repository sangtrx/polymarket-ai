use crate::ingestion::artifact_parser::ParsedArtifact;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquivalenceRecord {
    pub equivalence_id: String,
    pub left_source_item_id: String,
    pub right_source_item_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRecord {
    pub conflict_id: String,
    pub status: String,
    pub left_source_item_id: String,
    pub right_source_item_id: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotFinding {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SnapshotAssembly {
    pub equivalence_records: Vec<EquivalenceRecord>,
    pub unresolved_conflicts: Vec<ConflictRecord>,
    pub findings: Vec<SnapshotFinding>,
    pub successful: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotBuildInput {
    pub commit_sha: String,
    pub ingested_at_utc: String,
    pub file_digests: BTreeMap<String, String>,
    pub aggregate_digest: String,
    pub parsed_artifacts: Vec<ParsedArtifact>,
}

pub fn build_snapshot_assembly(_input: SnapshotBuildInput) -> SnapshotAssembly {
    SnapshotAssembly::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingestion::artifact_parser::{ParsedArtifact, ParsedArtifactItem};

    fn parsed(items: Vec<ParsedArtifactItem>) -> ParsedArtifact {
        ParsedArtifact {
            items,
            findings: Vec::new(),
        }
    }

    fn item(path: &str, heading_slug: &str, index: u32, source_item_id: &str, body: &str) -> ParsedArtifactItem {
        ParsedArtifactItem {
            artifact_path: path.to_string(),
            heading_slug: heading_slug.to_string(),
            item_index: index,
            source_item_id: Some(source_item_id.to_string()),
            body: body.to_string(),
            line_number: 1,
        }
    }

    #[test]
    fn preserves_source_ids_through_explicit_equivalence_records() {
        let input = SnapshotBuildInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            file_digests: BTreeMap::new(),
            aggregate_digest: "digest-all".to_string(),
            parsed_artifacts: vec![
                parsed(vec![item(
                    ".planning/PRD.md",
                    "coverage",
                    1,
                    "REQ-1",
                    "System captures canonical audit evidence",
                )]),
                parsed(vec![item(
                    "docs/architecture.md",
                    "coverage",
                    1,
                    "ARCH-REQ-1",
                    "System captures canonical audit evidence",
                )]),
            ],
        };

        let assembly = build_snapshot_assembly(input);
        assert!(assembly.equivalence_records.iter().any(|record| {
            (record.left_source_item_id == "REQ-1" && record.right_source_item_id == "ARCH-REQ-1")
                || (record.left_source_item_id == "ARCH-REQ-1"
                    && record.right_source_item_id == "REQ-1")
        }));
    }

    #[test]
    fn persists_semantic_conflicts_as_unresolved_records() {
        let input = SnapshotBuildInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            file_digests: BTreeMap::new(),
            aggregate_digest: "digest-all".to_string(),
            parsed_artifacts: vec![
                parsed(vec![item(
                    ".planning/PRD.md",
                    "gating",
                    1,
                    "REQ-2",
                    "Execution must require manual approval",
                )]),
                parsed(vec![item(
                    "docs/architecture.md",
                    "gating",
                    1,
                    "ARCH-2",
                    "Execution must not require manual approval",
                )]),
            ],
        };

        let assembly = build_snapshot_assembly(input);
        assert!(assembly
            .unresolved_conflicts
            .iter()
            .any(|conflict| conflict.status == "unresolved"));
    }

    #[test]
    fn remains_successful_when_schema_valid_even_with_conflicts() {
        let input = SnapshotBuildInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            file_digests: BTreeMap::new(),
            aggregate_digest: "digest-all".to_string(),
            parsed_artifacts: vec![
                parsed(vec![item(
                    ".planning/PRD.md",
                    "gating",
                    1,
                    "REQ-2",
                    "Execution must require manual approval",
                )]),
                parsed(vec![item(
                    "docs/architecture.md",
                    "gating",
                    1,
                    "ARCH-2",
                    "Execution must not require manual approval",
                )]),
            ],
        };

        let assembly = build_snapshot_assembly(input);
        assert!(assembly.successful);
        assert!(assembly
            .findings
            .iter()
            .any(|finding| finding.code == "canonical_item_conflict_unresolved"));
    }
}
