# RBAC Role Model and Guardrails

## Canonical roles

Story 1.2 defines three explicit governance roles:

1. `read_only_analytics` — analytics-only visibility (`read_analytics`).
2. `operational_control` — analytics visibility plus control-plane execution (`read_analytics`, `execute_control_action`).
3. `administrative_actions` — operational permissions plus role-governance authority (`read_analytics`, `execute_control_action`, `manage_role_assignments`).

These mappings are canonical and deterministic. Any unknown role, unknown permission, duplicate role-permission mapping, or invalid permission combination is rejected with an explicit machine-readable authorization error.

## Permission boundaries

Boundary behavior is strict:

- Read-only role can never execute control-plane mutation actions.
- Operational control can execute control-plane actions but cannot manage role assignments.
- Administrative actions include inherited operational + analytics permissions.
- No implicit privilege elevation is allowed (including no default admin grants when user role resolution is missing).

## Persistence schema scope

RBAC persistence is intentionally constrained to:

- `roles`
- `role_permissions`
- `user_roles`

Schema constraints enforce:

- Role/permission key allowlists (`CHECK` constraints).
- Role-permission uniqueness (`PRIMARY KEY (role_id, permission_key)`).
- User-role uniqueness (`PRIMARY KEY (actor_id, role_id)`).
- Foreign-key integrity from `role_permissions` and `user_roles` to `roles`.

## Operational review expectations

All control-plane authorization decisions emit traceable evidence with:

- `actor_id`
- `role`
- `action`
- `outcome` (`allow`/`deny`)
- `reason`
- `correlation_id`
- `timestamp_utc`

For access-change operations and incident review:

1. Validate requested assignment against least-privilege policy before writing `user_roles`.
2. Confirm machine-readable deny reasons exist for rejected out-of-scope requests.
3. Review telemetry evidence for both successful and denied decisions during QA and incident triage.

## Control-plane authentication contract (Story 1.3)

Privileged control endpoints (currently `POST /control/rebalance`) are authenticated before RBAC authorization is evaluated.

Required request inputs:

1. `Authorization: Bearer <actor_id>:<role>:<expires_unix>`
2. `x-correlation-id: <traceable-correlation-id>`

Validation is deterministic and fail-closed:

- Missing credentials -> `auth_missing_credentials` (401)
- Malformed credential shape -> `auth_malformed_credentials` (401)
- Invalid credential material -> `auth_invalid_credentials` (401)
- Expired credentials -> `auth_expired_credentials` (401)
- Unknown/invalid actor context fields -> `auth_unknown_actor_context` (400)
- Authenticator adapter verification failures -> `auth_verification_failed` (401)

Authentication denials return machine-readable envelope fields under `error.code`, emit UTC RFC3339 timestamps, and include alert-compatible security signal attributes for unauthorized privileged-attempt detection (targetable to <=30s alert objectives).

## Story 1.4 immutable audit handoff expectations

Story 1.4 appends immutable `audit_log_append` records using authenticated boundary context from the control API.

Canonical audit payload contract:

- `actor_id`
- `role`
- `action_type`
- `parameters` (JSON object; sensitive keys redacted before append)
- `approval_reference` (nullable until Story 1.5 dual-approval rollout)
- `timestamp` (RFC3339 UTC)
- `outcome` (`allow`, `authorization_denied`, `authentication_denied`)
- `reason_code` (machine-readable auth/authz decision code)
- `correlation_id`
- `authentication_outcome`

Operational guardrails:

1. Append-only immutability is enforced in schema with update/delete prevention triggers.
2. Privileged allow paths are fail-closed if audit persistence cannot append.
3. Audit append failures return explicit machine-readable error codes (`audit_invalid_payload`, `audit_persistence_unavailable`, `audit_append_constraint_violation`).
4. Control-path telemetry and audit evidence include actor, role, action, outcome, reason, timestamp, and correlation metadata for QA/incident forensics.

Story handoff dependencies:

1. Story 1.5 should populate `approval_reference` for dual-approval workflows (never inferred in Story 1.4).
2. Story 4 reporting/incident consumers should treat `audit_log_append` as immutable source-of-truth for privileged-action trace reconstruction.

## Story 1.5 dual-approval governance lifecycle

Story 1.5 adds FR34 dual-approval controls for the canonical critical actions:

1. `strategy_promotion_override`
2. `risk_limit_increase`
3. `kill_switch_disable`
4. `production_config_change`

Lifecycle contract:

1. Proposer submits an approval request with explicit `expires_at_utc`.
2. Request starts in `pending` state and is rate-limited to `<= 5` proposer submissions per rolling hour.
3. A distinct approver (`proposer != approver`, canonicalized actor identity) records an `approve` or `reject` vote.
4. Approved requests transition to `approved` with non-null `approval_reference`; rejected requests transition to `rejected`.
5. Any evaluation where `now_utc >= expires_at_utc` transitions/returns `expired` and is denied.
6. Critical mutation execution is blocked unless request state is `approved` with a valid `approval_reference`.

Deterministic machine-readable denial codes include:

- `approval_missing_request`
- `approval_unknown_request`
- `approval_missing_second_approver`
- `approval_self_approval_attempt`
- `approval_duplicate_vote`
- `approval_expired_window`
- `approval_unauthorized_role`
- `approval_rate_limited_actor`
- `approval_invalid_state_transition`

## Incident-review evidence expectations for Story 1.5

For both allowed and denied FR34 decisions, operators should be able to reconstruct:

- `actor_id`
- `action` (canonical critical action id)
- `outcome` / `state`
- `reason_code`
- `correlation_id`
- `timestamp_utc`
- `request_id` linkage
- `approval_reference` linkage (required for approved execution, absent for denied paths)

Denied FR34 responses and telemetry must include alert-compatible security-signal fields:

- `name = unauthorized_privileged_approval_attempt_v1`
- `alert_compatible = true`
- `alert_target_seconds = 30`
- `severity = high`
