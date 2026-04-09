use crate::ingestion::artifact_discovery::ArtifactSource;
use domain::audit_artifacts::canonical_requirement_id;

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
    pub artifact_path: String,
    pub heading_slug: String,
    pub item_index: u32,
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

pub fn parse_markdown_artifact(
    source: &ArtifactSource,
    markdown: &str,
) -> Result<ParsedArtifact, ArtifactParseError> {
    let mut parsed = ParsedArtifact::default();
    let mut current_heading_slug: Option<String> = None;
    let mut item_index: u32 = 0;
    let lines: Vec<&str> = markdown.lines().collect();
    let mut line_index = 0usize;

    while line_index < lines.len() {
        let line_number = line_index + 1;
        let line = lines[line_index].trim();

        if line.starts_with('#') {
            let heading = line.trim_start_matches('#').trim();
            if heading.is_empty() {
                return Err(invalid_payload(
                    "heading",
                    "heading title cannot be empty",
                    line_number,
                ));
            }
            let slug = slugify(heading);
            current_heading_slug = Some(slug.clone());
            item_index += 1;
            push_item(&mut parsed, source, slug, item_index, None, heading, line_number)?;
            line_index += 1;
            continue;
        }

        if line.starts_with("|") {
            let (consumed, rows) = collect_table_rows(&lines, line_index);
            if let Some(first) = rows.first() {
                let headers = split_table_row(first);
                if headers.iter().any(|header| header.eq_ignore_ascii_case("acceptance")) {
                    parse_acceptance_rows(
                        source,
                        &mut parsed,
                        &current_heading_slug,
                        &rows,
                        &mut item_index,
                        line_number,
                    )?;
                }
            }
            line_index += consumed;
            continue;
        }

        if line.starts_with("- [") || line.starts_with("* [") {
            let Some(heading_slug) = current_heading_slug.clone() else {
                return Err(invalid_payload(
                    "checklist",
                    "checklist item must exist under a heading",
                    line_number,
                ));
            };
            let body = line
                .split_once(']')
                .map(|(_, text)| text.trim())
                .unwrap_or_default();
            if body.is_empty() {
                return Err(invalid_payload(
                    "checklist",
                    "checklist body cannot be empty",
                    line_number,
                ));
            }
            item_index += 1;
            let source_item_id = extract_source_item_id(body);
            if source_item_id.is_none() {
                parsed.findings.push(ValidationFinding {
                    severity: ValidationSeverity::Warning,
                    code: "canonical_artifact_minor_quality_warning",
                    message: "checklist item missing source identifier".to_string(),
                    field: "source_item_id".to_string(),
                });
            }
            push_item(
                &mut parsed,
                source,
                heading_slug,
                item_index,
                source_item_id,
                body,
                line_number,
            )?;
        }

        line_index += 1;
    }

    Ok(parsed)
}

fn parse_acceptance_rows(
    source: &ArtifactSource,
    parsed: &mut ParsedArtifact,
    heading_slug: &Option<String>,
    rows: &[String],
    item_index: &mut u32,
    line_number: usize,
) -> Result<(), ArtifactParseError> {
    if rows.len() < 3 {
        return Err(invalid_payload(
            "acceptance_table",
            "acceptance table requires header, divider, and at least one row",
            line_number,
        ));
    }

    let headers = split_table_row(&rows[0]);
    let divider = split_table_row(&rows[1]);
    if divider.len() != headers.len() || divider.iter().any(|cell| !cell.contains('-')) {
        return Err(invalid_payload(
            "acceptance_table",
            "acceptance table divider is malformed",
            line_number + 1,
        ));
    }

    let id_index = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case("id"))
        .ok_or_else(|| invalid_payload("acceptance_table", "acceptance table missing ID column", line_number))?;
    let acceptance_index = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case("acceptance"))
        .ok_or_else(|| {
            invalid_payload(
                "acceptance_table",
                "acceptance table missing Acceptance column",
                line_number,
            )
        })?;

    let base_slug = heading_slug
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| "acceptance-criteria".to_string());
    for (offset, row) in rows.iter().enumerate().skip(2) {
        let columns = split_table_row(row);
        if columns.len() != headers.len() {
            return Err(invalid_payload(
                "acceptance_table",
                "acceptance row does not match header width",
                line_number + offset,
            ));
        }
        let body = columns[acceptance_index].trim().to_string();
        if body.is_empty() {
            return Err(invalid_payload(
                "acceptance_table",
                "acceptance row body cannot be empty",
                line_number + offset,
            ));
        }
        *item_index += 1;
        let source_item_id = normalize_id_cell(&columns[id_index]);
        if source_item_id.is_none() {
            parsed.findings.push(ValidationFinding {
                severity: ValidationSeverity::Warning,
                code: "canonical_artifact_minor_quality_warning",
                message: "acceptance row missing source identifier".to_string(),
                field: "source_item_id".to_string(),
            });
        }
        push_item(
            parsed,
            source,
            base_slug.clone(),
            *item_index,
            source_item_id,
            &body,
            line_number + offset,
        )?;
    }
    Ok(())
}

fn collect_table_rows(lines: &[&str], start: usize) -> (usize, Vec<String>) {
    let mut rows = Vec::new();
    let mut cursor = start;
    while cursor < lines.len() {
        let trimmed = lines[cursor].trim();
        if !trimmed.starts_with('|') {
            break;
        }
        rows.push(trimmed.to_string());
        cursor += 1;
    }
    (cursor - start, rows)
}

fn split_table_row(row: &str) -> Vec<String> {
    row.trim_matches('|')
        .split('|')
        .map(|part| part.trim().to_string())
        .collect()
}

fn push_item(
    parsed: &mut ParsedArtifact,
    source: &ArtifactSource,
    heading_slug: String,
    item_index: u32,
    source_item_id: Option<String>,
    body: &str,
    line_number: usize,
) -> Result<(), ArtifactParseError> {
    canonical_requirement_id(&source.relative_path, &heading_slug, item_index).map_err(|_| {
        invalid_payload(
            "canonical_requirement_id",
            "failed to validate canonical requirement anchor",
            line_number,
        )
    })?;
    parsed.items.push(ParsedArtifactItem {
        artifact_path: source.relative_path.clone(),
        heading_slug,
        item_index,
        source_item_id,
        body: body.trim().to_string(),
        line_number,
    });
    Ok(())
}

fn normalize_id_cell(cell: &str) -> Option<String> {
    let value = cell.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

fn extract_source_item_id(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    let cleaned = token.trim_end_matches([':', ',', '.']);
    if cleaned.contains('-') || cleaned.chars().any(|ch| ch.is_ascii_digit()) {
        return Some(cleaned.to_string());
    }
    None
}

fn slugify(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(' ', "-")
        .replace('_', "-")
}

fn invalid_payload(field: &str, message: &str, line_number: usize) -> ArtifactParseError {
    ArtifactParseError {
        code: "canonical_artifact_invalid_payload",
        message: message.to_string(),
        field_errors: vec![ValidationFinding {
            severity: ValidationSeverity::Error,
            code: "canonical_artifact_invalid_payload",
            message: format!("{message} at line {line_number}"),
            field: field.to_string(),
        }],
    }
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
