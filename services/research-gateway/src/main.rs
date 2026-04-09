use research_gateway::ingestion::service::{
    IngestionMode, RunCanonicalIngestionInput, run_canonical_ingestion,
};
use research_gateway::traceability::matcher::EvidenceCandidate;
use research_gateway::traceability::service::{
    RunTraceabilityMappingInput, TraceabilityRequirementInput, run_traceability_mapping,
};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, PartialEq, Eq)]
struct CliErrorPayload {
    code: String,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IngestArtifactsCliArgs {
    commit_sha: String,
    ingested_at_utc: String,
    repo_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceEvidenceCliArgs {
    commit_sha: String,
    generated_at_utc: String,
    repo_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
    IngestArtifacts(IngestArtifactsCliArgs),
    TraceEvidence(TraceEvidenceCliArgs),
}

fn parse_cli_command(args: &[String]) -> Result<CliCommand, CliErrorPayload> {
    let command = args.get(1).map(String::as_str).ok_or_else(|| CliErrorPayload {
        code: "canonical_ingestion_invalid_payload".to_string(),
        message: "expected `ingest-artifacts` or `trace-evidence` command".to_string(),
    })?;
    let get_flag = |flag: &str| -> Option<String> {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
    match command {
        "ingest-artifacts" => {
            let commit_sha = get_flag("--commit-sha").ok_or_else(|| CliErrorPayload {
                code: "canonical_ingestion_invalid_payload".to_string(),
                message: "missing --commit-sha".to_string(),
            })?;
            let ingested_at_utc = get_flag("--ingested-at-utc").ok_or_else(|| CliErrorPayload {
                code: "canonical_ingestion_invalid_payload".to_string(),
                message: "missing --ingested-at-utc".to_string(),
            })?;
            let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
                code: "canonical_ingestion_invalid_payload".to_string(),
                message: "missing --repo-root".to_string(),
            })?;
            Ok(CliCommand::IngestArtifacts(IngestArtifactsCliArgs {
                commit_sha,
                ingested_at_utc,
                repo_root,
            }))
        }
        "trace-evidence" => {
            let commit_sha = get_flag("--commit-sha").ok_or_else(|| CliErrorPayload {
                code: "traceability_invalid_payload".to_string(),
                message: "missing --commit-sha".to_string(),
            })?;
            let generated_at_utc = get_flag("--generated-at-utc").ok_or_else(|| CliErrorPayload {
                code: "traceability_invalid_payload".to_string(),
                message: "missing --generated-at-utc".to_string(),
            })?;
            let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
                code: "traceability_invalid_payload".to_string(),
                message: "missing --repo-root".to_string(),
            })?;
            Ok(CliCommand::TraceEvidence(TraceEvidenceCliArgs {
                commit_sha,
                generated_at_utc,
                repo_root,
            }))
        }
        _ => Err(CliErrorPayload {
            code: "canonical_ingestion_invalid_payload".to_string(),
            message: "expected `ingest-artifacts` or `trace-evidence` command".to_string(),
        }),
    }
}

async fn run_cli(args: &[String]) -> Result<String, CliErrorPayload> {
    match parse_cli_command(args)? {
        CliCommand::IngestArtifacts(command) => {
            let output = run_canonical_ingestion(RunCanonicalIngestionInput {
                commit_sha: command.commit_sha,
                ingested_at_utc: command.ingested_at_utc,
                repo_root: command.repo_root,
                mode: IngestionMode::FullSnapshot,
            })
            .await
            .map_err(|error| CliErrorPayload {
                code: error.code,
                message: error.message,
            })?;
            serde_json::to_string(&output).map_err(|error| CliErrorPayload {
                code: "canonical_ingestion_invalid_payload".to_string(),
                message: format!("unable to serialize output: {error}"),
            })
        }
        CliCommand::TraceEvidence(command) => {
            let traceability_input = build_traceability_mapping_input(
                &command.commit_sha,
                &command.generated_at_utc,
                &command.repo_root,
            )
            .await?;
            let output = run_traceability_mapping(traceability_input)
                .await
                .map_err(|error| CliErrorPayload {
                    code: error.code,
                    message: error.message,
                })?;
            serde_json::to_string(&output).map_err(|error| CliErrorPayload {
                code: "traceability_invalid_payload".to_string(),
                message: format!("unable to serialize output: {error}"),
            })
        }
    }
}

async fn build_traceability_mapping_input(
    commit_sha: &str,
    generated_at_utc: &str,
    repo_root: &str,
) -> Result<RunTraceabilityMappingInput, CliErrorPayload> {
    let ingestion = run_canonical_ingestion(RunCanonicalIngestionInput {
        commit_sha: commit_sha.to_string(),
        ingested_at_utc: generated_at_utc.to_string(),
        repo_root: repo_root.to_string(),
        mode: IngestionMode::FullSnapshot,
    })
    .await
    .map_err(|error| CliErrorPayload {
        code: "traceability_invalid_payload".to_string(),
        message: error.message,
    })?;

    let file_paths = collect_source_files(Path::new(repo_root)).map_err(|error| CliErrorPayload {
        code: "traceability_invalid_payload".to_string(),
        message: error,
    })?;

    let requirements = ingestion
        .canonical_requirement_ids
        .iter()
        .map(|requirement_id| {
            let (deterministic_candidates, semantic_candidates) =
                collect_requirement_candidates(Path::new(repo_root), requirement_id, &file_paths);
            TraceabilityRequirementInput {
                canonical_requirement_id: requirement_id.clone(),
                deterministic_candidates,
                semantic_candidates,
                previous_links: vec![],
            }
        })
        .collect::<Vec<_>>();

    Ok(RunTraceabilityMappingInput {
        snapshot_id: format!("traceability_{}_{}", commit_sha, generated_at_utc),
        commit_sha: commit_sha.to_string(),
        generated_at_utc: generated_at_utc.to_string(),
        repo_root: repo_root.to_string(),
        requirements,
    })
}

fn collect_source_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("unable to read {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("unable to read directory entry: {error}"))?;
            let path = entry.path();
            if path.file_name().map(|name| name == ".git").unwrap_or(false) {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if is_supported_source_file(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn collect_requirement_candidates(
    repo_root: &Path,
    requirement_id: &str,
    files: &[PathBuf],
) -> (Vec<EvidenceCandidate>, Vec<EvidenceCandidate>) {
    let mut deterministic = Vec::new();
    let mut semantic = Vec::new();
    let semantic_tokens = requirement_id
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| token.len() >= 4)
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();

    for file in files {
        let Ok(contents) = fs::read_to_string(file) else {
            continue;
        };
        let relative_path = file
            .strip_prefix(repo_root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        if contents.contains(requirement_id) {
            deterministic.push(candidate_from_file(
                &relative_path,
                "deterministic canonical requirement ID reference",
                1.0,
            ));
            continue;
        }

        let lowered = contents.to_ascii_lowercase();
        let token_hits = semantic_tokens
            .iter()
            .filter(|token| lowered.contains(token.as_str()) || relative_path.contains(token.as_str()))
            .count();
        if token_hits > 0 {
            semantic.push(candidate_from_file(
                &relative_path,
                "semantic token overlap fallback candidate",
                token_hits as f32 / semantic_tokens.len().max(1) as f32,
            ));
        }
    }

    deterministic.sort_by(|left, right| right.score.total_cmp(&left.score));
    semantic.sort_by(|left, right| right.score.total_cmp(&left.score));
    (deterministic, semantic.into_iter().take(3).collect())
}

fn candidate_from_file(relative_path: &str, rationale: &str, score: f32) -> EvidenceCandidate {
    let evidence_type = if relative_path.contains("/tests/")
        || relative_path.contains(".test.")
        || relative_path.contains(".e2e.")
    {
        domain::traceability::EvidenceType::Test
    } else {
        domain::traceability::EvidenceType::Code
    };
    EvidenceCandidate {
        anchor: domain::traceability::EvidenceAnchor {
            evidence_type,
            file_path: relative_path.to_string(),
            symbol: Some("line_anchor".to_string()),
            section: None,
            line_start: Some(1),
            line_end: Some(1),
        },
        rationale: rationale.to_string(),
        provenance: "cli_repository_scan".to_string(),
        score,
    }
}

fn is_supported_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("rs" | "ts" | "tsx" | "js" | "mjs" | "md")
    )
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    match run_cli(&args).await {
        Ok(payload) => println!("{payload}"),
        Err(error) => {
            let payload =
                serde_json::to_string(&error).unwrap_or_else(|_| "{\"code\":\"canonical_ingestion_invalid_payload\",\"message\":\"unable to serialize error\"}".to_string());
            eprintln!("{payload}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_traceability_fixture_repo() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("traceability-cli-{nanos}"));
        fs::create_dir_all(root.join(".planning")).expect("fixture planning dir should be created");
        fs::create_dir_all(root.join("docs")).expect("fixture docs dir should be created");
        fs::create_dir_all(root.join("services/research-gateway/src"))
            .expect("fixture service dir should be created");
        fs::create_dir_all(root.join("tests/api")).expect("fixture tests dir should be created");
        fs::write(
            root.join(".planning/PRD.md"),
            "# PRD\n- [ ] TRAC-1 deterministic mapping output",
        )
        .expect("fixture PRD should be written");
        fs::write(
            root.join("docs/architecture.md"),
            "# Architecture\n- [ ] TRAC-1 evidence linkage",
        )
        .expect("fixture architecture should be written");
        fs::write(
            root.join(".planning/ROADMAP.md"),
            "# Roadmap\n- [ ] TRAC-1 mapping delivery",
        )
        .expect("fixture roadmap should be written");
        fs::create_dir_all(root.join(".planning/stories")).expect("fixture stories dir should be created");
        fs::write(
            root.join(".planning/stories/story-1.md"),
            "# Story\n- [ ] TRAC-1 operator traceability link",
        )
        .expect("fixture story should be written");
        fs::write(
            root.join("services/research-gateway/src/feature.rs"),
            "// project_md.requirements.1 deterministic implementation",
        )
        .expect("fixture code file should be written");
        fs::write(
            root.join("tests/api/phase-2-traceability.test.mjs"),
            "import assert from 'node:assert/strict';\nassert.equal(1,1); // project_md.requirements.1 assertion",
        )
        .expect("fixture test file should be written");
        root
    }

    #[test]
    fn ingest_artifacts_command_requires_commit_sha() {
        let args = vec![
            "research-gateway".to_string(),
            "ingest-artifacts".to_string(),
            "--ingested-at-utc".to_string(),
            "2026-04-09T00:00:00Z".to_string(),
            "--repo-root".to_string(),
            ".".to_string(),
        ];
        let error = parse_cli_command(&args).expect_err("missing commit sha should fail");
        assert_eq!(error.code, "canonical_ingestion_invalid_payload");
    }

    #[test]
    fn ingest_artifacts_command_requires_ingested_at_utc() {
        let args = vec![
            "research-gateway".to_string(),
            "ingest-artifacts".to_string(),
            "--commit-sha".to_string(),
            "abc123".to_string(),
            "--repo-root".to_string(),
            ".".to_string(),
        ];
        let error = parse_cli_command(&args).expect_err("missing timestamp should fail");
        assert_eq!(error.code, "canonical_ingestion_invalid_payload");
    }

    #[test]
    fn ingest_artifacts_command_parses_required_flags() {
        let args = vec![
            "research-gateway".to_string(),
            "ingest-artifacts".to_string(),
            "--commit-sha".to_string(),
            "abc123".to_string(),
            "--ingested-at-utc".to_string(),
            "2026-04-09T00:00:00Z".to_string(),
            "--repo-root".to_string(),
            ".".to_string(),
        ];
        let command = parse_cli_command(&args).expect("all required flags should parse");
        assert_eq!(
            command,
            CliCommand::IngestArtifacts(IngestArtifactsCliArgs {
                commit_sha: "abc123".to_string(),
                ingested_at_utc: "2026-04-09T00:00:00Z".to_string(),
                repo_root: ".".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn ingest_artifacts_command_surfaces_machine_readable_error_payload_for_invalid_inputs() {
        let args = vec![
            "research-gateway".to_string(),
            "ingest-artifacts".to_string(),
            "--commit-sha".to_string(),
            "abc123".to_string(),
            "--ingested-at-utc".to_string(),
            "not-a-timestamp".to_string(),
            "--repo-root".to_string(),
            ".".to_string(),
        ];
        let error = run_cli(&args)
            .await
            .expect_err("invalid timestamp should fail closed");
        assert_eq!(error.code, "canonical_ingestion_invalid_payload");
        assert!(error.message.contains("ingested_at_utc"));
    }

    #[test]
    fn trace_evidence_command_requires_commit_sha() {
        let args = vec![
            "research-gateway".to_string(),
            "trace-evidence".to_string(),
            "--generated-at-utc".to_string(),
            "2026-04-09T00:00:00Z".to_string(),
            "--repo-root".to_string(),
            ".".to_string(),
        ];
        let error = parse_cli_command(&args).expect_err("missing commit sha should fail");
        assert_eq!(error.code, "traceability_invalid_payload");
    }

    #[tokio::test]
    async fn trace_evidence_command_returns_json_with_rationale_confidence_and_reason_code() {
        let fixture = create_traceability_fixture_repo();
        let args = vec![
            "research-gateway".to_string(),
            "trace-evidence".to_string(),
            "--commit-sha".to_string(),
            "abc123".to_string(),
            "--generated-at-utc".to_string(),
            "2026-04-09T00:00:00Z".to_string(),
            "--repo-root".to_string(),
            fixture.to_string_lossy().to_string(),
        ];
        let payload = run_cli(&args).await.expect("traceability mapping should succeed");
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("json payload expected");
        assert!(parsed.get("rows").is_some());
        let first_row = parsed["rows"][0].clone();
        assert!(first_row.get("rationale").is_some());
        assert!(first_row.get("confidence").is_some());
        assert!(first_row.get("reason_code").is_some());
        fs::remove_dir_all(fixture).expect("fixture repo should be removed");
    }

    #[tokio::test]
    async fn trace_evidence_command_fails_closed_with_machine_readable_code() {
        let args = vec![
            "research-gateway".to_string(),
            "trace-evidence".to_string(),
            "--commit-sha".to_string(),
            "abc123".to_string(),
            "--generated-at-utc".to_string(),
            "invalid".to_string(),
            "--repo-root".to_string(),
            ".".to_string(),
        ];
        let error = run_cli(&args)
            .await
            .expect_err("invalid traceability input should fail closed");
        assert_eq!(error.code, "traceability_invalid_payload");
    }
}

#[cfg(test)]
mod main {
    mod tests {
        use super::super::*;
        use std::fs;
        use std::path::PathBuf;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn create_traceability_fixture_repo() -> PathBuf {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be monotonic")
                .as_nanos();
            let root = std::env::temp_dir().join(format!("traceability-cli-filter-{nanos}"));
            fs::create_dir_all(root.join(".planning")).expect("fixture planning dir should be created");
            fs::create_dir_all(root.join("docs")).expect("fixture docs dir should be created");
            fs::create_dir_all(root.join(".planning/stories")).expect("fixture stories dir should be created");
            fs::create_dir_all(root.join("services/research-gateway/src"))
                .expect("fixture service dir should be created");
            fs::create_dir_all(root.join("tests/api")).expect("fixture tests dir should be created");
            fs::write(
                root.join(".planning/PRD.md"),
                "# PRD\n- [ ] TRAC-1 deterministic mapping output",
            )
            .expect("fixture PRD should be written");
            fs::write(
                root.join("docs/architecture.md"),
                "# Architecture\n- [ ] TRAC-1 evidence linkage",
            )
            .expect("fixture architecture should be written");
            fs::write(
                root.join(".planning/ROADMAP.md"),
                "# Roadmap\n- [ ] TRAC-1 mapping delivery",
            )
            .expect("fixture roadmap should be written");
            fs::write(
                root.join(".planning/stories/story-1.md"),
                "# Story\n- [ ] TRAC-1 operator traceability link",
            )
            .expect("fixture story should be written");
            fs::write(
                root.join("services/research-gateway/src/feature.rs"),
                "// project_md.requirements.1 deterministic implementation",
            )
            .expect("fixture code file should be written");
            fs::write(
                root.join("tests/api/phase-2-traceability.test.mjs"),
                "import assert from 'node:assert/strict';\nassert.equal(1,1); // project_md.requirements.1 assertion",
            )
            .expect("fixture test file should be written");
            root
        }

        #[test]
        fn trace_evidence_command_requires_commit_sha() {
            let args = vec![
                "research-gateway".to_string(),
                "trace-evidence".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                ".".to_string(),
            ];
            let error = parse_cli_command(&args).expect_err("missing commit sha should fail");
            assert_eq!(error.code, "traceability_invalid_payload");
        }

        #[tokio::test]
        async fn trace_evidence_command_returns_json_payload() {
            let fixture = create_traceability_fixture_repo();
            let args = vec![
                "research-gateway".to_string(),
                "trace-evidence".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];
            let payload = run_cli(&args).await.expect("traceability mapping should succeed");
            let parsed: serde_json::Value = serde_json::from_str(&payload).expect("json payload expected");
            assert!(parsed.get("rows").is_some());
            fs::remove_dir_all(fixture).expect("fixture repo should be removed");
        }

        #[tokio::test]
        async fn trace_evidence_command_fails_closed_with_machine_readable_code() {
            let args = vec![
                "research-gateway".to_string(),
                "trace-evidence".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "invalid".to_string(),
                "--repo-root".to_string(),
                ".".to_string(),
            ];
            let error = run_cli(&args)
                .await
                .expect_err("invalid traceability input should fail closed");
            assert_eq!(error.code, "traceability_invalid_payload");
        }
    }
}
