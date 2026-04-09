use crate::ingestion::artifact_parser::ParsedArtifact;
use domain::audit_artifacts::{
    CanonicalArtifactInput, CanonicalArtifactType, CanonicalSnapshotInput, build_canonical_snapshot,
    canonical_requirement_id,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquivalenceRecord {
    pub equivalence_id: String,
    pub left_canonical_requirement_id: String,
    pub right_canonical_requirement_id: String,
    pub left_source_item_id: String,
    pub right_source_item_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRecord {
    pub conflict_id: String,
    pub status: String,
    pub left_canonical_requirement_id: String,
    pub right_canonical_requirement_id: String,
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
    pub snapshot_input: Option<CanonicalSnapshotInput>,
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

pub fn build_snapshot_assembly(input: SnapshotBuildInput) -> SnapshotAssembly {
    let mut findings = Vec::new();
    let mut canonical_entries = Vec::new();
    let mut artifacts = Vec::new();

    for artifact in input.parsed_artifacts {
        for item in artifact.items {
            let Ok(canonical_id) =
                canonical_requirement_id(&item.artifact_path, &item.heading_slug, item.item_index)
            else {
                findings.push(SnapshotFinding {
                    code: "canonical_artifact_invalid_payload".to_string(),
                    message: format!(
                        "invalid canonical anchor for {}:{}",
                        item.artifact_path, item.item_index
                    ),
                });
                continue;
            };

            let Some(artifact_type) = classify_artifact_type(&item.artifact_path) else {
                findings.push(SnapshotFinding {
                    code: "canonical_artifact_invalid_payload".to_string(),
                    message: format!("unable to classify artifact type for {}", item.artifact_path),
                });
                continue;
            };

            canonical_entries.push(CanonicalEntry {
                canonical_requirement_id: canonical_id,
                source_item_id: item.source_item_id.clone().unwrap_or_default(),
                body: item.body.clone(),
            });

            artifacts.push(CanonicalArtifactInput {
                artifact_type,
                artifact_path: item.artifact_path.clone(),
                heading_slug: item.heading_slug.clone(),
                item_index: item.item_index,
                source_item_id: item.source_item_id.clone().unwrap_or_default(),
                body: item.body.clone(),
            });

            if item.source_item_id.is_none() {
                findings.push(SnapshotFinding {
                    code: "canonical_artifact_minor_quality_warning".to_string(),
                    message: format!(
                        "source_item_id missing for {}:{}",
                        item.artifact_path, item.item_index
                    ),
                });
            }
        }
    }
    let snapshot_input = CanonicalSnapshotInput {
        commit_sha: input.commit_sha,
        ingested_at_utc: input.ingested_at_utc,
        file_digests: input.file_digests,
        aggregate_digest: input.aggregate_digest,
        artifacts,
    };

    if let Err(error) = build_canonical_snapshot(&snapshot_input) {
        findings.push(SnapshotFinding {
            code: error.code,
            message: error.message,
        });
        return SnapshotAssembly {
            snapshot_input: Some(snapshot_input),
            equivalence_records: Vec::new(),
            unresolved_conflicts: Vec::new(),
            findings,
            successful: false,
        };
    }

    let equivalence_records = build_equivalence_records(&canonical_entries);
    let unresolved_conflicts = build_unresolved_conflicts(&canonical_entries);

    for conflict in &unresolved_conflicts {
        findings.push(SnapshotFinding {
            code: "canonical_item_conflict_unresolved".to_string(),
            message: format!(
                "unresolved conflict between {} and {}",
                conflict.left_source_item_id, conflict.right_source_item_id
            ),
        });
    }

    SnapshotAssembly {
        snapshot_input: Some(snapshot_input),
        equivalence_records,
        unresolved_conflicts,
        findings,
        successful: true,
    }
}

#[derive(Debug, Clone)]
struct CanonicalEntry {
    canonical_requirement_id: String,
    source_item_id: String,
    body: String,
}

fn classify_artifact_type(path: &str) -> Option<CanonicalArtifactType> {
    let lowered = path.to_ascii_lowercase();
    if lowered.contains("prd") {
        return Some(CanonicalArtifactType::Prd);
    }
    if lowered.contains("architecture") || lowered.contains("arch") {
        return Some(CanonicalArtifactType::Architecture);
    }
    if lowered.contains("roadmap") {
        return Some(CanonicalArtifactType::Roadmap);
    }
    if lowered.contains("story") || lowered.contains("stories") {
        return Some(CanonicalArtifactType::Story);
    }
    None
}

fn build_equivalence_records(entries: &[CanonicalEntry]) -> Vec<EquivalenceRecord> {
    let mut records = Vec::new();
    for left in 0..entries.len() {
        for right in (left + 1)..entries.len() {
            if normalized_semantic_key(&entries[left].body) == normalized_semantic_key(&entries[right].body)
                && !entries[left].source_item_id.is_empty()
                && !entries[right].source_item_id.is_empty()
            {
                records.push(EquivalenceRecord {
                    equivalence_id: format!("equivalence-{left}-{right}"),
                    left_canonical_requirement_id: entries[left].canonical_requirement_id.clone(),
                    right_canonical_requirement_id: entries[right].canonical_requirement_id.clone(),
                    left_source_item_id: entries[left].source_item_id.clone(),
                    right_source_item_id: entries[right].source_item_id.clone(),
                });
            }
        }
    }
    records
}

fn build_unresolved_conflicts(entries: &[CanonicalEntry]) -> Vec<ConflictRecord> {
    let mut records = Vec::new();
    for left in 0..entries.len() {
        for right in (left + 1)..entries.len() {
            if is_semantic_conflict(&entries[left].body, &entries[right].body) {
                records.push(ConflictRecord {
                    conflict_id: format!("conflict-{left}-{right}"),
                    status: "unresolved".to_string(),
                    left_canonical_requirement_id: entries[left].canonical_requirement_id.clone(),
                    right_canonical_requirement_id: entries[right].canonical_requirement_id.clone(),
                    left_source_item_id: entries[left].source_item_id.clone(),
                    right_source_item_id: entries[right].source_item_id.clone(),
                    rationale: "semantic polarity mismatch".to_string(),
                });
            }
        }
    }
    records
}

fn is_semantic_conflict(left: &str, right: &str) -> bool {
    let left_polarity = sentence_polarity(left);
    let right_polarity = sentence_polarity(right);
    left_polarity.is_some()
        && right_polarity.is_some()
        && left_polarity != right_polarity
        && normalized_semantic_key(left) == normalized_semantic_key(right)
}

fn sentence_polarity(sentence: &str) -> Option<bool> {
    let lowered = sentence.to_ascii_lowercase();
    if lowered.contains("must not") || lowered.contains("should not") || lowered.contains("never") {
        return Some(false);
    }
    if lowered.contains("must") || lowered.contains("should") {
        return Some(true);
    }
    None
}

fn normalized_semantic_key(text: &str) -> String {
    text.to_ascii_lowercase()
        .replace("must not", "")
        .replace("should not", "")
        .replace("must", "")
        .replace("should", "")
        .replace("never", "")
        .replace("  ", " ")
        .trim()
        .to_string()
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
