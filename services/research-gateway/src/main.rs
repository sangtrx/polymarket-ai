use research_gateway::ingestion::service::{
    IngestionMode, RunCanonicalIngestionInput, run_canonical_ingestion,
};
use serde::Serialize;
use std::env;

#[derive(Debug, Serialize, PartialEq, Eq)]
struct CliErrorPayload {
    code: &'static str,
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
            code: "canonical_ingestion_invalid_payload",
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
        code: "canonical_ingestion_invalid_payload",
        message: "missing --commit-sha".to_string(),
    })?;
    let ingested_at_utc = get_flag("--ingested-at-utc").ok_or_else(|| CliErrorPayload {
        code: "canonical_ingestion_invalid_payload",
        message: "missing --ingested-at-utc".to_string(),
    })?;
    let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
        code: "canonical_ingestion_invalid_payload",
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
                code: "canonical_ingestion_invalid_payload",
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
}
