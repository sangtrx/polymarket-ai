use common::time::timestamp_utc;
use governance_service::credentials::CredentialRotationService;
use governance_service::rbac::GovernanceAuthorizationService;

#[tokio::main]
async fn main() {
    let authorization_service = GovernanceAuthorizationService::default();
    let _credential_rotation_service = CredentialRotationService::default();
    let bootstrap_decision = authorization_service.evaluate(
        "governance-bootstrap",
        "administrative_actions",
        "manage_role_assignments",
        "governance-service-startup",
    );
    let _bootstrap_evidence = authorization_service.telemetry_evidence(&bootstrap_decision);
    println!("governance-service scaffold ready at {}", timestamp_utc());
}
