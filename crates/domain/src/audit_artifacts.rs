use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalArtifactType {
    Prd,
    Architecture,
    Story,
    Roadmap,
}

impl CanonicalArtifactType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prd => "prd",
            Self::Architecture => "architecture",
            Self::Story => "story",
            Self::Roadmap => "roadmap",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalValidationIssue {
    pub field: String,
    pub code: String,
    pub message: String,
    pub severity: CanonicalValidationSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalArtifactContractError {
    pub code: String,
    pub message: String,
    pub field_errors: Vec<CanonicalValidationIssue>,
}

impl CanonicalArtifactContractError {
    fn invalid_payload(
        message: impl Into<String>,
        field_errors: Vec<CanonicalValidationIssue>,
    ) -> Self {
        Self {
            code: "canonical_artifact_invalid_payload".to_string(),
            message: message.into(),
            field_errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalArtifactInput {
    pub artifact_type: CanonicalArtifactType,
    pub artifact_path: String,
    pub heading_slug: String,
    pub item_index: u32,
    pub source_item_id: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalArtifactItem {
    pub canonical_requirement_id: String,
    pub artifact_type: CanonicalArtifactType,
    pub artifact_path: String,
    pub heading_slug: String,
    pub item_index: u32,
    pub source_item_id: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalSnapshotInput {
    pub commit_sha: String,
    pub ingested_at_utc: String,
    pub file_digests: BTreeMap<String, String>,
    pub aggregate_digest: String,
    pub artifacts: Vec<CanonicalArtifactInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalSnapshot {
    pub commit_sha: String,
    pub ingested_at_utc: String,
    pub file_digests: BTreeMap<String, String>,
    pub aggregate_digest: String,
    pub items: Vec<CanonicalArtifactItem>,
    pub warnings: Vec<CanonicalValidationIssue>,
}

// canonical_requirement_id uses artifact.section.item and is anchored on path + heading_slug + item_index.
pub fn canonical_requirement_id(
    artifact_path: &str,
    heading_slug: &str,
    item_index: u32,
) -> Result<String, CanonicalArtifactContractError> {
    if artifact_path.trim().is_empty() || heading_slug.trim().is_empty() || item_index == 0 {
        return Err(CanonicalArtifactContractError::invalid_payload(
            "path, heading_slug, and item_index are required",
            vec![CanonicalValidationIssue {
                field: "canonical_requirement_id".to_string(),
                code: "canonical_artifact_invalid_payload".to_string(),
                message: "path/heading_slug/item_index must be provided".to_string(),
                severity: CanonicalValidationSeverity::Error,
            }],
        ));
    }

    let artifact = normalize_path_token(artifact_path);
    let section = normalize_slug(heading_slug);
    Ok(format!("{artifact}.{section}.{item_index}"))
}

pub fn build_canonical_snapshot(
    input: &CanonicalSnapshotInput,
) -> Result<CanonicalSnapshot, CanonicalArtifactContractError> {
    if input.commit_sha.trim().is_empty() || input.aggregate_digest.trim().is_empty() {
        return Err(CanonicalArtifactContractError::invalid_payload(
            "commit_sha and aggregate_digest are required",
            vec![CanonicalValidationIssue {
                field: "snapshot".to_string(),
                code: "canonical_artifact_invalid_payload".to_string(),
                message: "snapshot metadata is incomplete".to_string(),
                severity: CanonicalValidationSeverity::Error,
            }],
        ));
    }
    validate_utc_timestamp(&input.ingested_at_utc)?;

    let mut items = Vec::with_capacity(input.artifacts.len());
    let mut warnings = Vec::new();
    for artifact in &input.artifacts {
        let canonical_requirement_id = canonical_requirement_id(
            &artifact.artifact_path,
            &artifact.heading_slug,
            artifact.item_index,
        )?;
        if artifact.source_item_id.trim().is_empty() {
            warnings.push(CanonicalValidationIssue {
                field: "source_item_id".to_string(),
                code: "canonical_artifact_minor_quality_warning".to_string(),
                message: "source_item_id missing; preserving item with warning".to_string(),
                severity: CanonicalValidationSeverity::Warning,
            });
        }
        items.push(CanonicalArtifactItem {
            canonical_requirement_id,
            artifact_type: artifact.artifact_type,
            artifact_path: artifact.artifact_path.trim().to_string(),
            heading_slug: normalize_slug(&artifact.heading_slug),
            item_index: artifact.item_index,
            source_item_id: if artifact.source_item_id.trim().is_empty() {
                None
            } else {
                Some(artifact.source_item_id.trim().to_string())
            },
            body: artifact.body.trim().to_string(),
        });
    }

    Ok(CanonicalSnapshot {
        commit_sha: input.commit_sha.trim().to_ascii_lowercase(),
        ingested_at_utc: input.ingested_at_utc.trim().to_string(),
        file_digests: input.file_digests.clone(),
        aggregate_digest: input.aggregate_digest.trim().to_ascii_lowercase(),
        items,
        warnings,
    })
}

fn normalize_path_token(path: &str) -> String {
    path.trim()
        .to_ascii_lowercase()
        .replace('/', "_")
        .replace('.', "_")
        .replace('-', "_")
}

fn normalize_slug(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(' ', "-")
        .replace('_', "-")
}

fn validate_utc_timestamp(value: &str) -> Result<(), CanonicalArtifactContractError> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        CanonicalArtifactContractError::invalid_payload(
            "ingested_at_utc must be RFC3339 UTC",
            vec![CanonicalValidationIssue {
                field: "ingested_at_utc".to_string(),
                code: "canonical_artifact_invalid_payload".to_string(),
                message: "ingested_at_utc must be RFC3339 UTC".to_string(),
                severity: CanonicalValidationSeverity::Error,
            }],
        )
    })?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(CanonicalArtifactContractError::invalid_payload(
            "ingested_at_utc must use Z offset",
            vec![CanonicalValidationIssue {
                field: "ingested_at_utc".to_string(),
                code: "canonical_artifact_invalid_payload".to_string(),
                message: "ingested_at_utc must use Z offset".to_string(),
                severity: CanonicalValidationSeverity::Error,
            }],
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_snapshot_input() -> CanonicalSnapshotInput {
        CanonicalSnapshotInput {
            commit_sha: "abc123".to_string(),
            ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
            file_digests: BTreeMap::from([(
                ".planning/PROJECT.md".to_string(),
                "digest-a".to_string(),
            )]),
            aggregate_digest: "digest-all".to_string(),
            artifacts: vec![CanonicalArtifactInput {
                artifact_type: CanonicalArtifactType::Prd,
                artifact_path: ".planning/PROJECT.md".to_string(),
                heading_slug: "requirements".to_string(),
                item_index: 1,
                source_item_id: "REQ-1".to_string(),
                body: "Traceability baseline".to_string(),
            }],
        }
    }

    #[test]
    fn deterministic_canonical_id_from_path_heading_and_index() {
        let first = canonical_requirement_id(".planning/PROJECT.md", "requirements", 1)
            .expect("canonical id should build");
        let second = canonical_requirement_id(".planning/PROJECT.md", "requirements", 1)
            .expect("canonical id should build");
        assert_eq!(first, second);
    }

    #[test]
    fn schema_invalid_payload_fails_closed() {
        let mut input = valid_snapshot_input();
        input.ingested_at_utc = "invalid-ts".to_string();
        let error = build_canonical_snapshot(&input).expect_err("invalid payload must fail");
        assert_eq!(error.code, "canonical_artifact_invalid_payload");
    }

    #[test]
    fn minor_quality_issue_is_warning_not_failure() {
        let mut input = valid_snapshot_input();
        input.artifacts[0].source_item_id = "".to_string();
        let snapshot =
            build_canonical_snapshot(&input).expect("minor quality issue should not fail snapshot");
        assert!(
            snapshot
                .warnings
                .iter()
                .any(|issue| issue.field == "source_item_id")
        );
    }
}
