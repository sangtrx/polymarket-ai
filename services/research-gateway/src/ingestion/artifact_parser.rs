use crate::ingestion::artifact_discovery::ArtifactSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationFinding {
    pub severity: ValidationSeverity,
    pub code: &'static str,
    pub message: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedArtifactItem {
    pub heading_slug: String,
    pub source_item_id: Option<String>,
    pub body: String,
    pub line_number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedArtifact {
    pub items: Vec<ParsedArtifactItem>,
    pub findings: Vec<ValidationFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactParseError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ValidationFinding>,
}

pub fn parse_markdown_artifact(_source: &ArtifactSource, _markdown: &str) -> Result<ParsedArtifact, ArtifactParseError> {
    Ok(ParsedArtifact::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingestion::artifact_discovery::{ArtifactSource, ArtifactSourceKind};

    fn source(path: &str) -> ArtifactSource {
        ArtifactSource {
            relative_path: path.to_string(),
            kind: ArtifactSourceKind::Prd,
        }
    }

    #[test]
    fn extracts_headings_checklists_and_acceptance_tables() {
        let markdown = r#"
# PRD
## Coverage Requirements
- [ ] REQ-1 Requirement from checklist

| ID | Acceptance | Notes |
| -- | ---------- | ----- |
| ACC-1 | Emits canonical IDs | keeps provenance |
"#;

        let parsed = parse_markdown_artifact(&source(".planning/PRD.md"), markdown)
            .expect("parser should succeed for valid structures");

        assert!(parsed
            .items
            .iter()
            .any(|item| item.body.contains("Coverage Requirements")));
        assert!(parsed
            .items
            .iter()
            .any(|item| item.source_item_id.as_deref() == Some("REQ-1")));
        assert!(parsed
            .items
            .iter()
            .any(|item| item.source_item_id.as_deref() == Some("ACC-1")));
    }

    #[test]
    fn schema_violations_fail_with_machine_readable_error_code() {
        let markdown = r#"
# PRD
| ID | Acceptance |
| --- |
| ACC-1 | malformed row |
"#;

        let error = parse_markdown_artifact(&source(".planning/PRD.md"), markdown)
            .expect_err("malformed acceptance schema must fail");
        assert_eq!(error.code, "canonical_artifact_invalid_payload");
        assert!(!error.field_errors.is_empty());
    }

    #[test]
    fn minor_quality_issues_emit_warning_without_failure() {
        let markdown = r#"
## Stories
- [ ] Missing identifier item
"#;
        let parsed = parse_markdown_artifact(&source(".planning/stories/story-1.md"), markdown)
            .expect("minor quality issue should not fail parse");
        assert!(parsed
            .findings
            .iter()
            .any(|finding| finding.severity == ValidationSeverity::Warning));
        assert!(!parsed.items.is_empty());
    }
}
