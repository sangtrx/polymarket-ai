# Credential Rotation Operations Runbook (Story 1.6)

## Scope

This runbook defines operator procedures for privileged credential rotation through authenticated control-plane workflows.

Supported triggers:

1. `scheduled_cadence` (policy rotation cadence)
2. `emergency_compromise` (incident response)

## Scheduled rotation procedure

1. Submit `POST /control/credentials/rotation/scheduled` with:
   - `credential_scope`
   - `credential_reference`
   - `last_rotated_at_utc`
   - readiness metadata:
     - `crypto_posture_verified: true`
     - `runtime_injection_mode: "runtime_only"`
     - provider reference metadata
2. Confirm response contract:
   - `status` (`accepted` or `denied`)
   - `outcome`
   - `state`
   - `reason_code`
   - `rotation_id`
   - `rotation_reference` (required when `state = succeeded`)
3. If denied with `credential_rotation_scheduled_not_due`, record the attempt and retain current credentials.

## Emergency compromise response

1. Submit `POST /control/credentials/rotation/emergency` immediately with:
   - `credential_scope`
   - `credential_reference`
   - `compromise_triggered_at_utc`
   - required readiness metadata (same keys as scheduled flow)
2. Verify decision is completed within `<= 30 minutes` of compromise trigger.
3. If denied/failed:
   - review `reason_code`
   - preserve fail-closed posture
   - escalate on-call if `credential_rotation_provider_unavailable` or `credential_rotation_runtime_unavailable`

## Rollback and verification checklist

1. Verify current workload remains on active credentials (no partial cutover).
2. Validate latest rotation event status in `credential_rotation_events`.
3. Confirm immutable audit/telemetry evidence has:
   - actor id
   - trigger type
   - credential scope
   - outcome/state
   - reason code
   - correlation id
   - timestamp
   - rotation reference (success only)
4. If rollback is required, perform provider-side rollback using the last known good `credential_reference` and create a new tracked rotation event.
5. Re-run smoke checks for control-plane endpoints requiring rotated credentials.

## Safe handling rules (mandatory)

1. Never include plaintext secrets in source files, logs, telemetry payloads, CLI arguments, or persisted records.
2. Use runtime injection only; pass references/metadata, not secret values.
3. Reject payloads containing secret-like metadata keys or values.
4. Treat ambiguous readiness state as deny/fail-closed (`credential_rotation_readiness_ambiguous`).
