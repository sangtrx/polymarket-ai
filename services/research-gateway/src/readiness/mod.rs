use crate::risk::service::RiskPrioritizationResult;
use domain::readiness::ReadinessState;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct RunReadinessInput {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub repo_root: String,
    pub risk_prioritization: Option<RiskPrioritizationResult>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReadinessServiceError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReadinessEvaluationResult {
    pub snapshot_id: String,
    pub commit_sha: String,
    pub generated_at_utc: String,
    pub readiness_state: ReadinessState,
    pub reason_code: String,
}

pub async fn run_readiness(
    _input: RunReadinessInput,
) -> Result<ReadinessEvaluationResult, ReadinessServiceError> {
    todo!("implemented in task green phase");
}

#[cfg(test)]
pub mod tests {
    use super::*;

    fn valid_input() -> RunReadinessInput {
        RunReadinessInput {
            snapshot_id: "readiness_abc123_2026-04-09T00:00:00Z".to_string(),
            commit_sha: "abc123".to_string(),
            generated_at_utc: "2026-04-09T00:00:00Z".to_string(),
            repo_root: ".".to_string(),
            risk_prioritization: None,
        }
    }

    #[tokio::test]
    async fn signal_not_ready_when_unwaived_critical_exists() {
        let _ = run_readiness(valid_input()).await.expect("placeholder");
    }

    #[tokio::test]
    async fn signal_caution_when_unwaived_high_or_medium_exists() {
        let _ = run_readiness(valid_input()).await.expect("placeholder");
    }

    #[tokio::test]
    async fn signal_ready_when_no_unwaived_critical_high_or_medium_exists() {
        let _ = run_readiness(valid_input()).await.expect("placeholder");
    }

    #[tokio::test]
    async fn signal_fails_closed_when_risk_snapshot_missing() {
        let _ = run_readiness(valid_input()).await.expect("placeholder");
    }
}
