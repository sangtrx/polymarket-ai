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
