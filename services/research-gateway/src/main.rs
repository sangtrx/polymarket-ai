use research_gateway::coverage::service::{
    RunCoverageClassificationInput, run_coverage_classification,
};
use research_gateway::ingestion::service::{
    IngestionMode, RunCanonicalIngestionInput, run_canonical_ingestion,
};
use research_gateway::readiness::{ReadinessEvaluationResult, RunReadinessInput, run_readiness};
use research_gateway::risk::service::{RunRiskPrioritizationInput, run_risk_prioritization};
use research_gateway::traceability::matcher::{EvidenceCandidate, PreviousLink};
use research_gateway::traceability::service::{
    RunTraceabilityMappingInput, TraceabilityRequirementInput, run_traceability_mapping,
};
use reporting_service::exports::workflows::{
    ReportExportOrchestrator, ReportExportWorkflowService, TriggerOnDemandExportInput,
};
use domain::readiness::{
    WaiverRecord, WaiverState, normalize_readiness_identifier,
    parse_utc_timestamp as parse_readiness_utc_timestamp, resolve_waiver_state, validate_waiver,
};
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
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
struct ClassifyCoverageCliArgs {
    commit_sha: String,
    generated_at_utc: String,
    repo_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrioritizeRiskCliArgs {
    commit_sha: String,
    generated_at_utc: String,
    repo_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Phase5ChainCliArgs {
    commit_sha: String,
    generated_at_utc: String,
    repo_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CreateReadinessWaiverCliArgs {
    waiver_id: Option<String>,
    canonical_requirement_id: String,
    owner: String,
    reason_code: String,
    justification: String,
    approved_by: String,
    created_at_utc: String,
    expires_at_utc: String,
    repo_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RevokeReadinessWaiverCliArgs {
    waiver_id: String,
    revoked_by: String,
    revoked_reason_code: String,
    revoked_at_utc: String,
    repo_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListReadinessWaiversCliArgs {
    as_of_utc: String,
    repo_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
    IngestArtifacts(IngestArtifactsCliArgs),
    TraceEvidence(TraceEvidenceCliArgs),
    ClassifyCoverage(ClassifyCoverageCliArgs),
    PrioritizeRisk(PrioritizeRiskCliArgs),
    RunPhase5Chain(Phase5ChainCliArgs),
    CreateReadinessWaiver(CreateReadinessWaiverCliArgs),
    RevokeReadinessWaiver(RevokeReadinessWaiverCliArgs),
    ListReadinessWaivers(ListReadinessWaiversCliArgs),
}

fn parse_cli_command(args: &[String]) -> Result<CliCommand, CliErrorPayload> {
    let command = args
        .get(1)
        .map(String::as_str)
        .ok_or_else(|| CliErrorPayload {
            code: "canonical_ingestion_invalid_payload".to_string(),
            message:
                "expected `ingest-artifacts`, `trace-evidence`, `classify-coverage`, `prioritize-risk`, `run-phase5-chain`, `create-readiness-waiver`, `revoke-readiness-waiver`, or `list-readiness-waivers` command".to_string(),
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
            let generated_at_utc =
                get_flag("--generated-at-utc").ok_or_else(|| CliErrorPayload {
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
        "classify-coverage" => {
            let commit_sha = get_flag("--commit-sha").ok_or_else(|| CliErrorPayload {
                code: "coverage_invalid_payload".to_string(),
                message: "missing --commit-sha".to_string(),
            })?;
            let generated_at_utc =
                get_flag("--generated-at-utc").ok_or_else(|| CliErrorPayload {
                    code: "coverage_invalid_payload".to_string(),
                    message: "missing --generated-at-utc".to_string(),
                })?;
            let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
                code: "coverage_invalid_payload".to_string(),
                message: "missing --repo-root".to_string(),
            })?;
            Ok(CliCommand::ClassifyCoverage(ClassifyCoverageCliArgs {
                commit_sha,
                generated_at_utc,
                repo_root,
            }))
        }
        "prioritize-risk" => {
            let commit_sha = get_flag("--commit-sha").ok_or_else(|| CliErrorPayload {
                code: "risk_invalid_payload".to_string(),
                message: "missing --commit-sha".to_string(),
            })?;
            let generated_at_utc =
                get_flag("--generated-at-utc").ok_or_else(|| CliErrorPayload {
                    code: "risk_invalid_payload".to_string(),
                    message: "missing --generated-at-utc".to_string(),
                })?;
            let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
                code: "risk_invalid_payload".to_string(),
                message: "missing --repo-root".to_string(),
            })?;
            Ok(CliCommand::PrioritizeRisk(PrioritizeRiskCliArgs {
                commit_sha,
                generated_at_utc,
                repo_root,
            }))
        }
        "run-phase5-chain" => {
            let commit_sha = get_flag("--commit-sha").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --commit-sha".to_string(),
            })?;
            let generated_at_utc =
                get_flag("--generated-at-utc").ok_or_else(|| CliErrorPayload {
                    code: "readiness_invalid_payload".to_string(),
                    message: "missing --generated-at-utc".to_string(),
                })?;
            let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --repo-root".to_string(),
            })?;
            Ok(CliCommand::RunPhase5Chain(Phase5ChainCliArgs {
                commit_sha,
                generated_at_utc,
                repo_root,
            }))
        }
        "create-readiness-waiver" => {
            let canonical_requirement_id =
                get_flag("--canonical-requirement-id").ok_or_else(|| CliErrorPayload {
                    code: "readiness_invalid_payload".to_string(),
                    message: "missing --canonical-requirement-id".to_string(),
                })?;
            let owner = get_flag("--owner").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --owner".to_string(),
            })?;
            let reason_code = get_flag("--reason-code").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --reason-code".to_string(),
            })?;
            let justification = get_flag("--justification").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --justification".to_string(),
            })?;
            let approved_by = get_flag("--approved-by").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --approved-by".to_string(),
            })?;
            let created_at_utc = get_flag("--created-at-utc").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --created-at-utc".to_string(),
            })?;
            let expires_at_utc = get_flag("--expires-at-utc").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --expires-at-utc".to_string(),
            })?;
            let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --repo-root".to_string(),
            })?;
            Ok(CliCommand::CreateReadinessWaiver(
                CreateReadinessWaiverCliArgs {
                    waiver_id: get_flag("--waiver-id"),
                    canonical_requirement_id,
                    owner,
                    reason_code,
                    justification,
                    approved_by,
                    created_at_utc,
                    expires_at_utc,
                    repo_root,
                },
            ))
        }
        "revoke-readiness-waiver" => {
            let waiver_id = get_flag("--waiver-id").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --waiver-id".to_string(),
            })?;
            let revoked_by = get_flag("--revoked-by").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --revoked-by".to_string(),
            })?;
            let revoked_reason_code =
                get_flag("--revoked-reason-code").ok_or_else(|| CliErrorPayload {
                    code: "readiness_invalid_payload".to_string(),
                    message: "missing --revoked-reason-code".to_string(),
                })?;
            let revoked_at_utc = get_flag("--revoked-at-utc").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --revoked-at-utc".to_string(),
            })?;
            let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --repo-root".to_string(),
            })?;
            Ok(CliCommand::RevokeReadinessWaiver(
                RevokeReadinessWaiverCliArgs {
                    waiver_id,
                    revoked_by,
                    revoked_reason_code,
                    revoked_at_utc,
                    repo_root,
                },
            ))
        }
        "list-readiness-waivers" => {
            let as_of_utc = get_flag("--as-of-utc").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --as-of-utc".to_string(),
            })?;
            let repo_root = get_flag("--repo-root").ok_or_else(|| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: "missing --repo-root".to_string(),
            })?;
            Ok(CliCommand::ListReadinessWaivers(
                ListReadinessWaiversCliArgs {
                    as_of_utc,
                    repo_root,
                },
            ))
        }
        _ => Err(CliErrorPayload {
            code: "canonical_ingestion_invalid_payload".to_string(),
            message:
                "expected `ingest-artifacts`, `trace-evidence`, `classify-coverage`, `prioritize-risk`, `run-phase5-chain`, `create-readiness-waiver`, `revoke-readiness-waiver`, or `list-readiness-waivers` command".to_string(),
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
        CliCommand::ClassifyCoverage(command) => {
            let commit_sha = command.commit_sha;
            let generated_at_utc = command.generated_at_utc;
            let repo_root = command.repo_root;

            let traceability_input =
                build_traceability_mapping_input(&commit_sha, &generated_at_utc, &repo_root)
                    .await
                    .map_err(|error| CliErrorPayload {
                        code: "coverage_invalid_payload".to_string(),
                        message: error.message,
                    })?;
            let canonical_requirement_ids = traceability_input
                .requirements
                .iter()
                .map(|requirement| requirement.canonical_requirement_id.clone())
                .collect::<Vec<_>>();

            let traceability_output =
                run_traceability_mapping(traceability_input)
                    .await
                    .map_err(|error| CliErrorPayload {
                        code: "coverage_invalid_payload".to_string(),
                        message: error.message,
                    })?;
            let output = run_coverage_classification(RunCoverageClassificationInput {
                snapshot_id: format!("coverage_{}_{}", commit_sha, generated_at_utc),
                commit_sha,
                generated_at_utc,
                repo_root,
                canonical_requirement_ids,
                traceability_rows: traceability_output.rows,
            })
            .await
            .map_err(|error| CliErrorPayload {
                code: error.code,
                message: error.message,
            })?;
            serde_json::to_string(&output).map_err(|error| CliErrorPayload {
                code: "coverage_invalid_payload".to_string(),
                message: format!("unable to serialize output: {error}"),
            })
        }
        CliCommand::PrioritizeRisk(command) => {
            let commit_sha = command.commit_sha;
            let generated_at_utc = command.generated_at_utc;
            let repo_root = command.repo_root;

            let traceability_input =
                build_traceability_mapping_input(&commit_sha, &generated_at_utc, &repo_root)
                    .await
                    .map_err(|error| CliErrorPayload {
                        code: "risk_invalid_payload".to_string(),
                        message: error.message,
                    })?;
            let canonical_requirement_ids = traceability_input
                .requirements
                .iter()
                .map(|requirement| requirement.canonical_requirement_id.clone())
                .collect::<Vec<_>>();
            let traceability_output =
                run_traceability_mapping(traceability_input)
                    .await
                    .map_err(|error| CliErrorPayload {
                        code: "risk_invalid_payload".to_string(),
                        message: error.message,
                    })?;
            let coverage_output = run_coverage_classification(RunCoverageClassificationInput {
                snapshot_id: format!("coverage_{}_{}", commit_sha, generated_at_utc),
                commit_sha: commit_sha.clone(),
                generated_at_utc: generated_at_utc.clone(),
                repo_root: repo_root.clone(),
                canonical_requirement_ids,
                traceability_rows: traceability_output.rows,
            })
            .await
            .map_err(|error| CliErrorPayload {
                code: "risk_invalid_payload".to_string(),
                message: error.message,
            })?;
            let output = run_risk_prioritization(RunRiskPrioritizationInput {
                snapshot_id: format!("risk_{}_{}", commit_sha, generated_at_utc),
                commit_sha,
                generated_at_utc,
                repo_root,
                coverage_matrix: coverage_output,
            })
            .await
            .map_err(|error| CliErrorPayload {
                code: error.code,
                message: error.message,
            })?;
            serde_json::to_string(&output).map_err(|error| CliErrorPayload {
                code: "risk_invalid_payload".to_string(),
                message: format!("unable to serialize output: {error}"),
            })
        }
        CliCommand::RunPhase5Chain(command) => {
            let output = run_phase5_chain(&command)
                .await
                .map_err(|error| CliErrorPayload {
                    code: error.code,
                    message: error.message,
                })?;
            serde_json::to_string(&output).map_err(|error| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: format!("unable to serialize output: {error}"),
            })
        }
        CliCommand::CreateReadinessWaiver(command) => {
            let output = create_readiness_waiver(command)?;
            serde_json::to_string(&output).map_err(|error| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: format!("unable to serialize output: {error}"),
            })
        }
        CliCommand::RevokeReadinessWaiver(command) => {
            let output = revoke_readiness_waiver(command)?;
            serde_json::to_string(&output).map_err(|error| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: format!("unable to serialize output: {error}"),
            })
        }
        CliCommand::ListReadinessWaivers(command) => {
            let output = list_readiness_waivers(command)?;
            serde_json::to_string(&output).map_err(|error| CliErrorPayload {
                code: "readiness_invalid_payload".to_string(),
                message: format!("unable to serialize output: {error}"),
            })
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ReadinessArtifactMetadata {
    artifact_id: String,
    artifact_type: String,
    checksum: String,
    path: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct Phase5ReportExportEvidence {
    job_id: String,
    status: String,
    reason_code: String,
    artifact_count: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct Phase5ChainOutput {
    traceability_snapshot_id: String,
    coverage_snapshot_id: String,
    risk_snapshot_id: String,
    readiness: ReadinessEvaluationResult,
    artifacts: Vec<ReadinessArtifactMetadata>,
    report_export: Phase5ReportExportEvidence,
}

#[derive(Debug, Clone)]
struct Phase5ChainError {
    code: String,
    message: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ReadinessWaiverMutationOutput {
    status: String,
    waiver: WaiverRecord,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ReadinessWaiverListOutput {
    as_of_utc: String,
    active: Vec<WaiverRecord>,
    expired: Vec<WaiverRecord>,
    revoked: Vec<WaiverRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
struct ReadinessWaiverStore {
    #[serde(default)]
    waivers: Vec<WaiverRecord>,
}

fn create_readiness_waiver(
    command: CreateReadinessWaiverCliArgs,
) -> Result<ReadinessWaiverMutationOutput, CliErrorPayload> {
    parse_readiness_utc_timestamp("created_at_utc", &command.created_at_utc)
        .map_err(|error| waiver_invalid_payload(error.message))?;
    parse_readiness_utc_timestamp("expires_at_utc", &command.expires_at_utc)
        .map_err(|error| waiver_invalid_payload(error.message))?;

    let waiver_id = command.waiver_id.unwrap_or_else(|| {
        compose_waiver_id(
            &command.canonical_requirement_id,
            &command.owner,
            &command.created_at_utc,
            &command.expires_at_utc,
        )
    });
    let waiver = WaiverRecord {
        waiver_id: normalize_readiness_identifier(&waiver_id),
        canonical_requirement_id: normalize_readiness_identifier(&command.canonical_requirement_id),
        owner: normalize_readiness_identifier(&command.owner),
        reason_code: normalize_readiness_identifier(&command.reason_code),
        justification: command.justification.trim().to_string(),
        approved_by: normalize_readiness_identifier(&command.approved_by),
        created_at_utc: command.created_at_utc.trim().to_string(),
        expires_at_utc: command.expires_at_utc.trim().to_string(),
        revoked_at_utc: None,
    };
    validate_waiver(&waiver).map_err(|error| waiver_invalid_payload(error.message))?;

    let mut store = load_readiness_waiver_store(&command.repo_root)?;
    if store
        .waivers
        .iter()
        .any(|existing| existing.waiver_id == waiver.waiver_id)
    {
        return Err(waiver_invalid_payload(format!(
            "waiver_id `{}` already exists",
            waiver.waiver_id
        )));
    }
    store.waivers.push(waiver.clone());
    sort_waivers(&mut store.waivers);
    persist_readiness_waiver_store(&command.repo_root, &store)?;

    Ok(ReadinessWaiverMutationOutput {
        status: "created".to_string(),
        waiver,
    })
}

fn revoke_readiness_waiver(
    command: RevokeReadinessWaiverCliArgs,
) -> Result<ReadinessWaiverMutationOutput, CliErrorPayload> {
    parse_readiness_utc_timestamp("revoked_at_utc", &command.revoked_at_utc)
        .map_err(|error| waiver_invalid_payload(error.message))?;

    let mut store = load_readiness_waiver_store(&command.repo_root)?;
    let normalized_waiver_id = normalize_readiness_identifier(&command.waiver_id);
    let waiver = store
        .waivers
        .iter_mut()
        .find(|candidate| candidate.waiver_id == normalized_waiver_id)
        .ok_or_else(|| {
            waiver_invalid_payload(format!("waiver_id `{normalized_waiver_id}` not found"))
        })?;
    if waiver.revoked_at_utc.is_some() {
        return Err(waiver_invalid_payload(format!(
            "waiver_id `{normalized_waiver_id}` is already revoked"
        )));
    }
    if command.revoked_by.trim().is_empty() || command.revoked_reason_code.trim().is_empty() {
        return Err(waiver_invalid_payload(
            "revoked_by and revoked_reason_code are required",
        ));
    }

    waiver.revoked_at_utc = Some(command.revoked_at_utc.trim().to_string());
    let waived = waiver.clone();
    sort_waivers(&mut store.waivers);
    persist_readiness_waiver_store(&command.repo_root, &store)?;

    Ok(ReadinessWaiverMutationOutput {
        status: "revoked".to_string(),
        waiver: waived,
    })
}

fn list_readiness_waivers(
    command: ListReadinessWaiversCliArgs,
) -> Result<ReadinessWaiverListOutput, CliErrorPayload> {
    parse_readiness_utc_timestamp("as_of_utc", &command.as_of_utc)
        .map_err(|error| waiver_invalid_payload(error.message))?;

    let store = load_readiness_waiver_store(&command.repo_root)?;
    let mut active = Vec::new();
    let mut expired = Vec::new();
    let mut revoked = Vec::new();
    for waiver in store.waivers {
        validate_waiver(&waiver).map_err(|error| waiver_invalid_payload(error.message))?;
        let state = resolve_waiver_state(&waiver, &command.as_of_utc)
            .map_err(|error| waiver_invalid_payload(error.message))?;
        match state {
            WaiverState::Active => active.push(waiver),
            WaiverState::Expired => expired.push(waiver),
            WaiverState::Revoked => revoked.push(waiver),
        }
    }
    sort_waivers(&mut active);
    sort_waivers(&mut expired);
    sort_waivers(&mut revoked);

    Ok(ReadinessWaiverListOutput {
        as_of_utc: command.as_of_utc,
        active,
        expired,
        revoked,
    })
}

fn load_phase5_active_waivers(
    repo_root: &str,
    generated_at_utc: &str,
    unresolved_requirement_ids: &BTreeSet<String>,
) -> Result<Vec<WaiverRecord>, Phase5ChainError> {
    let listing = list_readiness_waivers(ListReadinessWaiversCliArgs {
        as_of_utc: generated_at_utc.to_string(),
        repo_root: repo_root.to_string(),
    })
    .map_err(|error| Phase5ChainError {
        code: error.code,
        message: error.message,
    })?;
    for waiver in &listing.active {
        let requirement_id = normalize_readiness_identifier(&waiver.canonical_requirement_id);
        if !unresolved_requirement_ids.contains(&requirement_id) {
            return Err(Phase5ChainError {
                code: "readiness_invalid_payload".to_string(),
                message: format!(
                    "waiver `{}` references unknown or resolved canonical requirement `{}`",
                    waiver.waiver_id, waiver.canonical_requirement_id
                ),
            });
        }
    }
    Ok(listing.active)
}

fn readiness_waiver_store_path(repo_root: &str) -> PathBuf {
    Path::new(repo_root)
        .join(".planning")
        .join("artifacts")
        .join("readiness-waivers.json")
}

fn load_readiness_waiver_store(repo_root: &str) -> Result<ReadinessWaiverStore, CliErrorPayload> {
    let path = readiness_waiver_store_path(repo_root);
    if !path.exists() {
        return Ok(ReadinessWaiverStore::default());
    }
    let raw = fs::read_to_string(&path).map_err(|error| {
        waiver_invalid_payload(format!(
            "unable to read readiness waiver store `{}`: {error}",
            path.display()
        ))
    })?;

    let mut store = serde_json::from_str::<ReadinessWaiverStore>(&raw)
        .or_else(|_| serde_json::from_str::<Vec<WaiverRecord>>(&raw).map(|waivers| ReadinessWaiverStore { waivers }))
        .map_err(|error| {
            waiver_invalid_payload(format!(
                "unable to parse readiness waiver store `{}`: {error}",
                path.display()
            ))
        })?;
    sort_waivers(&mut store.waivers);
    Ok(store)
}

fn persist_readiness_waiver_store(
    repo_root: &str,
    store: &ReadinessWaiverStore,
) -> Result<(), CliErrorPayload> {
    let path = readiness_waiver_store_path(repo_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            waiver_invalid_payload(format!(
                "unable to create readiness waiver directory `{}`: {error}",
                parent.display()
            ))
        })?;
    }
    let payload = serde_json::to_string_pretty(store).map_err(|error| {
        waiver_invalid_payload(format!("unable to serialize readiness waiver store: {error}"))
    })?;
    fs::write(&path, payload).map_err(|error| {
        waiver_invalid_payload(format!(
            "unable to write readiness waiver store `{}`: {error}",
            path.display()
        ))
    })
}

fn sort_waivers(waivers: &mut [WaiverRecord]) {
    waivers.sort_by(|left, right| {
        left.canonical_requirement_id
            .cmp(&right.canonical_requirement_id)
            .then_with(|| left.expires_at_utc.cmp(&right.expires_at_utc))
            .then_with(|| left.waiver_id.cmp(&right.waiver_id))
    });
}

fn compose_waiver_id(
    canonical_requirement_id: &str,
    owner: &str,
    created_at_utc: &str,
    expires_at_utc: &str,
) -> String {
    let fingerprint = sha256_hex(
        format!(
            "{}|{}|{}|{}",
            normalize_readiness_identifier(canonical_requirement_id),
            normalize_readiness_identifier(owner),
            created_at_utc.trim(),
            expires_at_utc.trim()
        )
        .as_bytes(),
    );
    format!("waiver_{}", &fingerprint[..16])
}

fn waiver_invalid_payload(message: impl Into<String>) -> CliErrorPayload {
    CliErrorPayload {
        code: "readiness_invalid_payload".to_string(),
        message: message.into(),
    }
}

async fn run_phase5_chain(command: &Phase5ChainCliArgs) -> Result<Phase5ChainOutput, Phase5ChainError> {
    let traceability_input = build_traceability_mapping_input(
        &command.commit_sha,
        &command.generated_at_utc,
        &command.repo_root,
    )
    .await
    .map_err(|error| Phase5ChainError {
        code: "readiness_invalid_payload".to_string(),
        message: error.message,
    })?;

    let traceability_snapshot_id = traceability_input.snapshot_id.clone();
    let canonical_requirement_ids = traceability_input
        .requirements
        .iter()
        .map(|requirement| requirement.canonical_requirement_id.clone())
        .collect::<Vec<_>>();

    let traceability_output = run_traceability_mapping(traceability_input)
        .await
        .map_err(|error| Phase5ChainError {
            code: "readiness_dependency_unavailable".to_string(),
            message: error.message,
        })?;

    let coverage_snapshot_id = format!(
        "coverage_{}_{}",
        command.commit_sha, command.generated_at_utc
    );
    let coverage_output = run_coverage_classification(RunCoverageClassificationInput {
        snapshot_id: coverage_snapshot_id.clone(),
        commit_sha: command.commit_sha.clone(),
        generated_at_utc: command.generated_at_utc.clone(),
        repo_root: command.repo_root.clone(),
        canonical_requirement_ids,
        traceability_rows: traceability_output.rows,
    })
    .await
    .map_err(|error| Phase5ChainError {
        code: "readiness_dependency_unavailable".to_string(),
        message: error.message,
    })?;

    let risk_snapshot_id = format!("risk_{}_{}", command.commit_sha, command.generated_at_utc);
    let risk_output = run_risk_prioritization(RunRiskPrioritizationInput {
        snapshot_id: risk_snapshot_id.clone(),
        commit_sha: command.commit_sha.clone(),
        generated_at_utc: command.generated_at_utc.clone(),
        repo_root: command.repo_root.clone(),
        coverage_matrix: coverage_output,
    })
    .await
    .map_err(|error| Phase5ChainError {
        code: "readiness_dependency_unavailable".to_string(),
        message: error.message,
    })?;

    let unresolved_requirement_ids = risk_output
        .rows
        .iter()
        .map(|row| normalize_readiness_identifier(&row.canonical_requirement_id))
        .collect::<BTreeSet<_>>();
    let waivers = load_phase5_active_waivers(
        &command.repo_root,
        &command.generated_at_utc,
        &unresolved_requirement_ids,
    )?;

    let readiness_output = run_readiness(RunReadinessInput {
        snapshot_id: format!("readiness_{}_{}", command.commit_sha, command.generated_at_utc),
        commit_sha: command.commit_sha.clone(),
        generated_at_utc: command.generated_at_utc.clone(),
        repo_root: command.repo_root.clone(),
        risk_prioritization: Some(risk_output),
        waivers,
    })
    .await
    .map_err(|error| Phase5ChainError {
        code: error.code,
        message: error.message,
    })?;

    let artifacts = export_readiness_report(
        &command.repo_root,
        &command.commit_sha,
        &command.generated_at_utc,
        &readiness_output,
    )
    .map_err(|error| Phase5ChainError {
        code: "readiness_dependency_unavailable".to_string(),
        message: error,
    })?;

    let report_export_service = ReportExportWorkflowService::in_memory();
    let export_evidence = report_export_service
        .trigger_on_demand_export(TriggerOnDemandExportInput {
            actor_id: "research-gateway".to_string(),
            actor_role: "operational_control".to_string(),
            correlation_id: format!(
                "phase5_chain_{}_{}",
                command.commit_sha, command.generated_at_utc
            ),
            requested_at_utc: command.generated_at_utc.clone(),
            as_of_utc: command.generated_at_utc.clone(),
            commit_sha: Some(command.commit_sha.clone()),
            reason_code: None,
            unavailable_artifact_types: Vec::new(),
        })
        .map_err(|error| Phase5ChainError {
            code: "readiness_dependency_unavailable".to_string(),
            message: error.message,
        })?;

    Ok(Phase5ChainOutput {
        traceability_snapshot_id,
        coverage_snapshot_id,
        risk_snapshot_id,
        readiness: readiness_output,
        artifacts,
        report_export: Phase5ReportExportEvidence {
            job_id: export_evidence.job_id,
            status: export_evidence.status,
            reason_code: export_evidence.reason_code,
            artifact_count: export_evidence.artifact_count,
        },
    })
}

fn export_readiness_report(
    repo_root: &str,
    commit_sha: &str,
    generated_at_utc: &str,
    readiness: &ReadinessEvaluationResult,
) -> Result<Vec<ReadinessArtifactMetadata>, String> {
    let artifact_dir = Path::new(repo_root).join(".planning").join("artifacts");
    fs::create_dir_all(&artifact_dir)
        .map_err(|error| format!("unable to create readiness artifact dir: {error}"))?;

    let json_payload = serde_json::to_string_pretty(readiness)
        .map_err(|error| format!("unable to serialize readiness report json: {error}"))?;
    let markdown_payload = render_readiness_markdown(readiness);

    let json_path = artifact_dir.join("readiness-report.json");
    let md_path = artifact_dir.join("readiness-report.md");
    fs::write(&json_path, &json_payload)
        .map_err(|error| format!("unable to write readiness-report.json: {error}"))?;
    fs::write(&md_path, &markdown_payload)
        .map_err(|error| format!("unable to write readiness-report.md: {error}"))?;

    let normalized_timestamp = generated_at_utc.replace(':', "-");
    Ok(vec![
        ReadinessArtifactMetadata {
            artifact_id: format!("readiness_artifact_{}_{}_json", commit_sha, normalized_timestamp),
            artifact_type: "readiness-report.json".to_string(),
            checksum: sha256_hex(json_payload.as_bytes()),
            path: json_path.to_string_lossy().to_string(),
        },
        ReadinessArtifactMetadata {
            artifact_id: format!("readiness_artifact_{}_{}_md", commit_sha, normalized_timestamp),
            artifact_type: "readiness-report.md".to_string(),
            checksum: sha256_hex(markdown_payload.as_bytes()),
            path: md_path.to_string_lossy().to_string(),
        },
    ])
}

fn render_readiness_markdown(readiness: &ReadinessEvaluationResult) -> String {
    let mut markdown = String::new();
    markdown.push_str("# CI Readiness Report\n\n");
    markdown.push_str(&format!("- Snapshot: `{}`\n", readiness.snapshot_id));
    markdown.push_str(&format!(
        "- Advisory State: `{}`\n",
        match readiness.readiness_state {
            domain::readiness::ReadinessState::Ready => "ready",
            domain::readiness::ReadinessState::Caution => "caution",
            domain::readiness::ReadinessState::NotReady => "not_ready",
        }
    ));
    markdown.push_str(&format!(
        "- Recommendation Code: `{}`\n\n",
        readiness.reason_code
    ));
    markdown.push_str("## Coverage Posture\n");
    markdown.push_str(&format!(
        "- Unwaived unresolved rows: {}\n",
        readiness.explainability.unwaived_count
    ));
    markdown.push_str(&format!(
        "- Waived unresolved rows: {}\n\n",
        readiness.explainability.waived_count
    ));
    markdown.push_str("## Top Unresolved Risks\n");
    if readiness.explainability.top_unresolved_risks.is_empty() {
        markdown.push_str("- None\n");
    } else {
        for risk in &readiness.explainability.top_unresolved_risks {
            markdown.push_str(&format!(
                "- `{}` (`{:?}`, score {}, rank {})\n",
                risk.canonical_requirement_id, risk.severity, risk.risk_score, risk.priority_rank
            ));
        }
    }
    markdown.push_str("\n## Waiver Ledger\n");
    markdown.push_str("- Active: included in waived count\n");
    markdown.push_str("- Expired: excluded from waiver reduction\n");
    markdown.push_str("- Revoked: excluded from waiver reduction\n");
    markdown
}

fn sha256_hex(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    format!("{:x}", hasher.finalize())
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

    let file_paths =
        collect_source_files(Path::new(repo_root)).map_err(|error| CliErrorPayload {
            code: "traceability_invalid_payload".to_string(),
            message: error,
        })?;

    let history = load_traceability_history(Path::new(repo_root));
    let requirements = ingestion
        .canonical_requirement_ids
        .iter()
        .map(|requirement_id| {
            let previous_links = history.get(requirement_id).cloned().unwrap_or_default();
            let (deterministic_candidates, semantic_candidates) =
                collect_requirement_candidates(Path::new(repo_root), requirement_id, &file_paths);
            TraceabilityRequirementInput {
                canonical_requirement_id: requirement_id.clone(),
                deterministic_candidates,
                semantic_candidates,
                previous_links,
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
            let entry =
                entry.map_err(|error| format!("unable to read directory entry: {error}"))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| format!("unable to inspect {}: {error}", path.display()))?;
            if path.file_name().map(|name| name == ".git").unwrap_or(false) {
                continue;
            }
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if file_type.is_file() && is_supported_source_file(&path) {
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
        .filter(|token| {
            !matches!(
                token.to_ascii_lowercase().as_str(),
                "project" | "planning" | "requirements" | "story" | "roadmap" | "architecture"
            )
        })
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
            .filter(|token| {
                lowered.contains(token.as_str()) || relative_path.contains(token.as_str())
            })
            .count();
        if token_hits >= 2 {
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

#[derive(Debug, Deserialize)]
struct HistoryAnchor {
    snapshot_id: String,
    file_path: String,
    symbol: Option<String>,
    line_start: Option<u32>,
    line_end: Option<u32>,
}

fn load_traceability_history(
    repo_root: &Path,
) -> std::collections::BTreeMap<String, Vec<PreviousLink>> {
    let history_path = repo_root.join(".traceability-history.json");
    let Ok(raw) = fs::read_to_string(history_path) else {
        return std::collections::BTreeMap::new();
    };
    let Ok(parsed) =
        serde_json::from_str::<std::collections::BTreeMap<String, Vec<HistoryAnchor>>>(&raw)
    else {
        return std::collections::BTreeMap::new();
    };
    parsed
        .into_iter()
        .map(|(requirement_id, anchors)| {
            let links = anchors
                .into_iter()
                .map(|anchor| PreviousLink {
                    snapshot_id: anchor.snapshot_id,
                    anchor: domain::traceability::EvidenceAnchor {
                        evidence_type: domain::traceability::EvidenceType::Code,
                        file_path: anchor.file_path,
                        symbol: anchor.symbol,
                        section: None,
                        line_start: anchor.line_start,
                        line_end: anchor.line_end,
                    },
                })
                .collect::<Vec<_>>();
            (requirement_id, links)
        })
        .collect()
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
        Some("rs" | "ts" | "tsx" | "js" | "mjs")
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
        fs::create_dir_all(root.join(".planning/stories"))
            .expect("fixture stories dir should be created");
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
        let payload = run_cli(&args)
            .await
            .expect("traceability mapping should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&payload).expect("json payload expected");
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
            fs::create_dir_all(root.join(".planning"))
                .expect("fixture planning dir should be created");
            fs::create_dir_all(root.join("docs")).expect("fixture docs dir should be created");
            fs::create_dir_all(root.join(".planning/stories"))
                .expect("fixture stories dir should be created");
            fs::create_dir_all(root.join("services/research-gateway/src"))
                .expect("fixture service dir should be created");
            fs::create_dir_all(root.join("tests/api"))
                .expect("fixture tests dir should be created");
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
            let payload = run_cli(&args)
                .await
                .expect("traceability mapping should succeed");
            let parsed: serde_json::Value =
                serde_json::from_str(&payload).expect("json payload expected");
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

        #[test]
        fn classify_coverage_command_requires_commit_sha() {
            let args = vec![
                "research-gateway".to_string(),
                "classify-coverage".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                ".".to_string(),
            ];
            let error = parse_cli_command(&args).expect_err("missing commit sha should fail");
            assert_eq!(error.code, "coverage_invalid_payload");
        }

        #[tokio::test]
        async fn classify_coverage_command_returns_complete_matrix_payload() {
            let fixture = create_traceability_fixture_repo();
            let args = vec![
                "research-gateway".to_string(),
                "classify-coverage".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];
            let payload = run_cli(&args)
                .await
                .expect("coverage classification should succeed");
            let parsed: serde_json::Value =
                serde_json::from_str(&payload).expect("json payload expected");
            assert!(parsed.get("snapshot_id").is_some());
            assert!(parsed.get("commit_sha").is_some());
            assert!(parsed.get("generated_at_utc").is_some());

            let rows = parsed["rows"].as_array().expect("rows must be an array");
            assert!(!rows.is_empty());

            let ids = rows
                .iter()
                .map(|row| {
                    row["canonical_requirement_id"]
                        .as_str()
                        .expect("canonical_requirement_id should be string")
                        .to_string()
                })
                .collect::<Vec<_>>();
            let mut sorted_ids = ids.clone();
            sorted_ids.sort();
            assert_eq!(ids, sorted_ids);

            let unique_ids = ids.iter().collect::<std::collections::BTreeSet<_>>();
            assert_eq!(unique_ids.len(), rows.len());

            for row in rows {
                let class = row["class"].as_str().expect("class should be serialized");
                assert!(matches!(class, "covered" | "partial" | "missing"));
                if matches!(class, "partial" | "missing") {
                    let reason_code = row["reason_code"]
                        .as_str()
                        .expect("reason_code should be string");
                    let rationale = row["rationale"]
                        .as_str()
                        .expect("rationale should be string");
                    assert!(!reason_code.is_empty());
                    assert!(!rationale.is_empty());
                }
                assert!(row["code_anchors"].is_array());
                assert!(row["test_anchors"].is_array());
                assert!(row["ambiguous_candidates"].is_array());
            }

            fs::remove_dir_all(fixture).expect("fixture repo should be removed");
        }

        #[tokio::test]
        async fn classify_coverage_command_fails_closed_with_machine_readable_code() {
            let args = vec![
                "research-gateway".to_string(),
                "classify-coverage".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "invalid".to_string(),
                "--repo-root".to_string(),
                ".".to_string(),
            ];
            let error = run_cli(&args)
                .await
                .expect_err("invalid coverage input should fail closed");
            assert_eq!(error.code, "coverage_invalid_payload");
        }

        #[test]
        fn prioritize_risk_command_requires_commit_sha() {
            let args = vec![
                "research-gateway".to_string(),
                "prioritize-risk".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                ".".to_string(),
            ];
            let error = parse_cli_command(&args).expect_err("missing commit sha should fail");
            assert_eq!(error.code, "risk_invalid_payload");
        }

        #[tokio::test]
        async fn prioritize_risk_command_returns_json_payload_with_priority_rank_and_risk_score() {
            let fixture = create_traceability_fixture_repo();
            let args = vec![
                "research-gateway".to_string(),
                "prioritize-risk".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];
            let first_payload = run_cli(&args)
                .await
                .expect("risk prioritization should succeed");
            let second_payload = run_cli(&args)
                .await
                .expect("risk prioritization replay should succeed");
            assert_eq!(first_payload, second_payload);

            let parsed: serde_json::Value =
                serde_json::from_str(&first_payload).expect("json payload expected");
            assert!(parsed.get("snapshot_id").is_some());
            assert!(parsed.get("commit_sha").is_some());
            assert!(parsed.get("generated_at_utc").is_some());
            let rows = parsed["rows"].as_array().expect("rows must be an array");
            assert!(!rows.is_empty());

            for row in rows {
                let priority_rank = row["priority_rank"]
                    .as_u64()
                    .expect("priority_rank should be numeric");
                let risk_score = row["risk_score"]
                    .as_i64()
                    .expect("risk_score should be numeric");
                let severity = row["severity"]
                    .as_str()
                    .expect("severity should be serialized");
                assert!(priority_rank >= 1);
                assert!(risk_score >= 0);
                assert!(matches!(severity, "critical" | "high" | "medium" | "low"));
            }

            fs::remove_dir_all(fixture).expect("fixture repo should be removed");
        }

        #[tokio::test]
        async fn prioritize_risk_command_fails_closed_with_machine_readable_code() {
            let args = vec![
                "research-gateway".to_string(),
                "prioritize-risk".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "invalid".to_string(),
                "--repo-root".to_string(),
                ".".to_string(),
            ];
            let error = run_cli(&args)
                .await
                .expect_err("invalid risk input should fail closed");
            assert_eq!(error.code, "risk_invalid_payload");
        }

        #[test]
        fn phase5_chain_command_token_is_parsed() {
            let args = vec![
                "research-gateway".to_string(),
                "run-phase5-chain".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                ".".to_string(),
            ];
            let parsed = parse_cli_command(&args);
            assert!(parsed.is_ok(), "phase5 command token should be accepted");
        }

        #[test]
        fn phase5_chain_command_requires_generated_at_utc() {
            let args = vec![
                "research-gateway".to_string(),
                "run-phase5-chain".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--repo-root".to_string(),
                ".".to_string(),
            ];
            let error = parse_cli_command(&args).expect_err("missing timestamp should fail");
            assert_eq!(error.code, "readiness_invalid_payload");
        }

        #[tokio::test]
        async fn phase5_chain_emits_deterministic_readiness_artifacts_and_ids() {
            let fixture = create_traceability_fixture_repo();
            let args = vec![
                "research-gateway".to_string(),
                "run-phase5-chain".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];

            let first = run_cli(&args).await.expect("first phase 5 chain run should succeed");
            let second = run_cli(&args).await.expect("second phase 5 chain run should succeed");
            assert_eq!(first, second, "identical inputs must produce deterministic output");

            let parsed: serde_json::Value =
                serde_json::from_str(&first).expect("output should be valid json");
            assert_eq!(
                parsed["readiness"]["snapshot_id"]
                    .as_str()
                    .expect("snapshot id expected"),
                "readiness_abc123_2026-04-09T00:00:00Z"
            );
            let artifacts = parsed["artifacts"]
                .as_array()
                .expect("artifact metadata should be present");
            assert_eq!(artifacts.len(), 2);
            let artifact_types = artifacts
                .iter()
                .map(|artifact| artifact["artifact_type"].as_str().unwrap_or_default())
                .collect::<Vec<_>>();
            assert!(artifact_types.contains(&"readiness-report.json"));
            assert!(artifact_types.contains(&"readiness-report.md"));
            fs::remove_dir_all(fixture).expect("fixture repo should be removed");
        }

        #[tokio::test]
        async fn readiness_waiver_commands_create_list_and_revoke() {
            let fixture = create_traceability_fixture_repo();
            let create_args = vec![
                "research-gateway".to_string(),
                "create-readiness-waiver".to_string(),
                "--canonical-requirement-id".to_string(),
                "gate-03".to_string(),
                "--owner".to_string(),
                "ops-owner".to_string(),
                "--reason-code".to_string(),
                "approved_exception".to_string(),
                "--justification".to_string(),
                "temporary mitigation".to_string(),
                "--approved-by".to_string(),
                "ops-approver".to_string(),
                "--created-at-utc".to_string(),
                "2026-04-08T00:00:00Z".to_string(),
                "--expires-at-utc".to_string(),
                "2026-04-12T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];
            let created_payload = run_cli(&create_args)
                .await
                .expect("waiver creation should succeed");
            let created: serde_json::Value =
                serde_json::from_str(&created_payload).expect("created payload should be valid json");
            let waiver_id = created["waiver"]["waiver_id"]
                .as_str()
                .expect("waiver id should be present")
                .to_string();

            let list_args = vec![
                "research-gateway".to_string(),
                "list-readiness-waivers".to_string(),
                "--as-of-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];
            let listed_payload = run_cli(&list_args)
                .await
                .expect("waiver listing should succeed");
            let listed: serde_json::Value =
                serde_json::from_str(&listed_payload).expect("listed payload should be valid json");
            assert_eq!(
                listed["active"]
                    .as_array()
                    .expect("active waivers should be an array")
                    .len(),
                1
            );

            let revoke_args = vec![
                "research-gateway".to_string(),
                "revoke-readiness-waiver".to_string(),
                "--waiver-id".to_string(),
                waiver_id,
                "--revoked-by".to_string(),
                "ops-owner".to_string(),
                "--revoked-reason-code".to_string(),
                "mitigation_complete".to_string(),
                "--revoked-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];
            run_cli(&revoke_args)
                .await
                .expect("waiver revocation should succeed");

            let listed_after_revoke_payload = run_cli(&list_args)
                .await
                .expect("waiver listing after revoke should succeed");
            let listed_after_revoke: serde_json::Value = serde_json::from_str(
                &listed_after_revoke_payload,
            )
            .expect("listed payload should be valid json");
            assert_eq!(
                listed_after_revoke["active"]
                    .as_array()
                    .expect("active waivers should be an array")
                    .len(),
                0
            );
            assert_eq!(
                listed_after_revoke["revoked"]
                    .as_array()
                    .expect("revoked waivers should be an array")
                    .len(),
                1
            );
            fs::remove_dir_all(fixture).expect("fixture repo should be removed");
        }

        #[tokio::test]
        async fn phase5_chain_consumes_active_waivers_from_store() {
            let fixture = create_traceability_fixture_repo();
            let chain_args = vec![
                "research-gateway".to_string(),
                "run-phase5-chain".to_string(),
                "--commit-sha".to_string(),
                "abc123".to_string(),
                "--generated-at-utc".to_string(),
                "2026-04-09T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];

            let baseline_payload = run_cli(&chain_args)
                .await
                .expect("baseline phase 5 chain run should succeed");
            let baseline: serde_json::Value =
                serde_json::from_str(&baseline_payload).expect("baseline payload should be valid json");
            let requirement_id = baseline["readiness"]["explainability"]["top_unresolved_risks"]
                .as_array()
                .and_then(|rows| rows.first())
                .and_then(|row| row["canonical_requirement_id"].as_str())
                .expect("fixture should produce at least one unresolved risk row")
                .to_string();

            let create_args = vec![
                "research-gateway".to_string(),
                "create-readiness-waiver".to_string(),
                "--canonical-requirement-id".to_string(),
                requirement_id,
                "--owner".to_string(),
                "ops-owner".to_string(),
                "--reason-code".to_string(),
                "approved_exception".to_string(),
                "--justification".to_string(),
                "temporary mitigation".to_string(),
                "--approved-by".to_string(),
                "ops-approver".to_string(),
                "--created-at-utc".to_string(),
                "2026-04-08T00:00:00Z".to_string(),
                "--expires-at-utc".to_string(),
                "2026-04-12T00:00:00Z".to_string(),
                "--repo-root".to_string(),
                fixture.to_string_lossy().to_string(),
            ];
            run_cli(&create_args)
                .await
                .expect("waiver creation should succeed");

            let with_waiver_payload = run_cli(&chain_args)
                .await
                .expect("phase 5 chain run with waiver should succeed");
            let with_waiver: serde_json::Value = serde_json::from_str(&with_waiver_payload)
                .expect("waiver payload should be valid json");
            let waived_count = with_waiver["readiness"]["explainability"]["waived_count"]
                .as_u64()
                .expect("waived_count should be numeric");
            assert!(
                waived_count >= 1,
                "at least one risk should be waived when active waivers exist"
            );
            fs::remove_dir_all(fixture).expect("fixture repo should be removed");
        }
    }
}
