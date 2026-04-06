use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CONTRACT_VERSION_V1: &str = "v1";
pub const CONTRACT_KEY_TRADES: &str = "reporting.trades";
pub const CONTRACT_KEY_POSITIONS: &str = "reporting.positions";
pub const CONTRACT_KEY_RISK_EVENTS: &str = "reporting.risk-events";
pub const CONTRACT_KEY_PERFORMANCE: &str = "reporting.performance";
pub const CONTRACT_KEY_ALPHA_ATTRIBUTION: &str = "reporting.alpha-attribution";

const TRADES_SCHEMA_V1: &str = include_str!("artifacts/v1/trades.schema.json");
const POSITIONS_SCHEMA_V1: &str = include_str!("artifacts/v1/positions.schema.json");
const RISK_EVENTS_SCHEMA_V1: &str = include_str!("artifacts/v1/risk-events.schema.json");
const PERFORMANCE_SCHEMA_V1: &str = include_str!("artifacts/v1/performance.schema.json");
const ALPHA_ATTRIBUTION_SCHEMA_V1: &str =
    include_str!("artifacts/v1/alpha-attribution.schema.json");
const CHANGELOG_V1: &str = include_str!("artifacts/v1/changelog.json");

const CHANGELOG_ARTIFACT_PATH_V1: &str =
    "services/reporting-service/src/contracts/artifacts/v1/changelog.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatasetContractSpec {
    pub dataset: &'static str,
    pub contract_key: &'static str,
    pub schema_artifact_path: &'static str,
    pub schema_payload: &'static str,
}

const DATASET_CONTRACT_SPECS_V1: &[DatasetContractSpec] = &[
    DatasetContractSpec {
        dataset: "trades",
        contract_key: CONTRACT_KEY_TRADES,
        schema_artifact_path: "services/reporting-service/src/contracts/artifacts/v1/trades.schema.json",
        schema_payload: TRADES_SCHEMA_V1,
    },
    DatasetContractSpec {
        dataset: "positions",
        contract_key: CONTRACT_KEY_POSITIONS,
        schema_artifact_path: "services/reporting-service/src/contracts/artifacts/v1/positions.schema.json",
        schema_payload: POSITIONS_SCHEMA_V1,
    },
    DatasetContractSpec {
        dataset: "risk-events",
        contract_key: CONTRACT_KEY_RISK_EVENTS,
        schema_artifact_path: "services/reporting-service/src/contracts/artifacts/v1/risk-events.schema.json",
        schema_payload: RISK_EVENTS_SCHEMA_V1,
    },
    DatasetContractSpec {
        dataset: "performance",
        contract_key: CONTRACT_KEY_PERFORMANCE,
        schema_artifact_path: "services/reporting-service/src/contracts/artifacts/v1/performance.schema.json",
        schema_payload: PERFORMANCE_SCHEMA_V1,
    },
    DatasetContractSpec {
        dataset: "alpha-attribution",
        contract_key: CONTRACT_KEY_ALPHA_ATTRIBUTION,
        schema_artifact_path: "services/reporting-service/src/contracts/artifacts/v1/alpha-attribution.schema.json",
        schema_payload: ALPHA_ATTRIBUTION_SCHEMA_V1,
    },
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContractArtifactDocument {
    pub contract_key: String,
    pub contract_version: String,
    pub artifact_kind: String,
    pub artifact_path: String,
    pub checksum_sha256: String,
    pub payload: serde_json::Value,
}

pub fn dataset_contract_spec(dataset: &str) -> Option<DatasetContractSpec> {
    DATASET_CONTRACT_SPECS_V1
        .iter()
        .find(|spec| spec.dataset == dataset)
        .copied()
}

pub fn contract_key_for_dataset(dataset: &str) -> Option<&'static str> {
    dataset_contract_spec(dataset).map(|spec| spec.contract_key)
}

pub fn supported_contract_specs() -> &'static [DatasetContractSpec] {
    DATASET_CONTRACT_SPECS_V1
}

pub fn schema_artifact_for(
    contract_key: &str,
    contract_version: &str,
) -> Option<ContractArtifactDocument> {
    if contract_version != CONTRACT_VERSION_V1 {
        return None;
    }
    let spec = DATASET_CONTRACT_SPECS_V1
        .iter()
        .find(|candidate| candidate.contract_key == contract_key)?;
    let checksum_sha256 = sha256_hex(spec.schema_payload.as_bytes());
    let payload = serde_json::from_str(spec.schema_payload).ok()?;
    Some(ContractArtifactDocument {
        contract_key: spec.contract_key.to_string(),
        contract_version: contract_version.to_string(),
        artifact_kind: "schema".to_string(),
        artifact_path: spec.schema_artifact_path.to_string(),
        checksum_sha256,
        payload,
    })
}

pub fn changelog_artifact_for(contract_version: &str) -> Option<ContractArtifactDocument> {
    if contract_version != CONTRACT_VERSION_V1 {
        return None;
    }
    let checksum_sha256 = sha256_hex(CHANGELOG_V1.as_bytes());
    let payload = serde_json::from_str(CHANGELOG_V1).ok()?;
    Some(ContractArtifactDocument {
        contract_key: "reporting.all".to_string(),
        contract_version: contract_version.to_string(),
        artifact_kind: "changelog".to_string(),
        artifact_path: CHANGELOG_ARTIFACT_PATH_V1.to_string(),
        checksum_sha256,
        payload,
    })
}

pub fn expected_schema_checksum(contract_key: &str, contract_version: &str) -> Option<String> {
    schema_artifact_for(contract_key, contract_version).map(|artifact| artifact.checksum_sha256)
}

pub fn expected_changelog_checksum(contract_version: &str) -> Option<String> {
    changelog_artifact_for(contract_version).map(|artifact| artifact.checksum_sha256)
}

pub fn compute_record_checksum(
    contract_key: &str,
    contract_version: &str,
    schema_checksum_sha256: &str,
    changelog_checksum_sha256: &str,
) -> String {
    sha256_hex(
        format!(
            "{contract_key}|{contract_version}|{schema_checksum_sha256}|{changelog_checksum_sha256}"
        )
        .as_bytes(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_registry_includes_all_story_4_2_datasets() {
        let specs = supported_contract_specs();
        assert_eq!(specs.len(), 5);
        assert!(specs.iter().any(|spec| spec.dataset == "trades"));
        assert!(specs.iter().any(|spec| spec.dataset == "positions"));
        assert!(specs.iter().any(|spec| spec.dataset == "risk-events"));
        assert!(specs.iter().any(|spec| spec.dataset == "performance"));
        assert!(specs.iter().any(|spec| spec.dataset == "alpha-attribution"));
    }

    #[test]
    fn schema_and_changelog_artifacts_parse_to_json_with_stable_checksums() {
        for spec in supported_contract_specs() {
            let artifact = schema_artifact_for(spec.contract_key, CONTRACT_VERSION_V1)
                .expect("schema artifact must exist");
            assert_eq!(artifact.artifact_kind, "schema");
            assert_eq!(artifact.contract_version, CONTRACT_VERSION_V1);
            assert_eq!(artifact.checksum_sha256.len(), 64);
            assert!(
                artifact
                    .checksum_sha256
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
            );
        }

        let changelog =
            changelog_artifact_for(CONTRACT_VERSION_V1).expect("changelog artifact must exist");
        assert_eq!(changelog.artifact_kind, "changelog");
        assert_eq!(changelog.checksum_sha256.len(), 64);
        assert!(
            changelog
                .checksum_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );
    }

    #[test]
    fn schemas_retain_required_envelope_and_meta_fields() {
        for spec in supported_contract_specs() {
            let artifact = schema_artifact_for(spec.contract_key, CONTRACT_VERSION_V1)
                .expect("schema artifact must exist");
            let required = artifact
                .payload
                .get("required")
                .and_then(serde_json::Value::as_array)
                .expect("top-level required array must exist");
            assert!(required.contains(&serde_json::Value::String("data".to_string())));
            assert!(required.contains(&serde_json::Value::String("meta".to_string())));
            assert!(required.contains(&serde_json::Value::String("error".to_string())));

            let meta_required = artifact
                .payload
                .get("$defs")
                .and_then(|value| value.get("meta"))
                .and_then(|value| value.get("required"))
                .and_then(serde_json::Value::as_array)
                .expect("meta required array must exist");
            assert!(
                meta_required.contains(&serde_json::Value::String("contract_version".to_string()))
            );
            assert!(meta_required.contains(&serde_json::Value::String("reason_code".to_string())));
            assert!(
                meta_required.contains(&serde_json::Value::String("correlation_id".to_string()))
            );
        }
    }
}
