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

Story 1.4 must append immutable audit records using authenticated identity context already emitted by the control-plane boundary:

- `actor_id`
- `role`
- `correlation_id`
- `authentication_outcome`
- auth/authorization decision codes
- RFC3339 UTC decision timestamp

The append pipeline should consume these authenticated context fields directly and must not re-introduce raw caller-supplied identity trust.
