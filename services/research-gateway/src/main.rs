use research_gateway::ingestion::service::{
    IngestionMode, RunCanonicalIngestionInput, run_canonical_ingestion,
};
use serde::Serialize;
use std::env;

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
enum CliCommand {
    IngestArtifacts(IngestArtifactsCliArgs),
}

fn parse_cli_command(args: &[String]) -> Result<CliCommand, CliErrorPayload> {
    if args.get(1).map(String::as_str) != Some("ingest-artifacts") {
        return Err(CliErrorPayload {
            code: "canonical_ingestion_invalid_payload".to_string(),
            message: "expected `ingest-artifacts` command".to_string(),
        });
    }
    let get_flag = |flag: &str| -> Option<String> {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
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
    }
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
