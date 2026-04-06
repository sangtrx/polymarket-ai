# Accessibility and Reduced-Motion Critical Flows (Story 3.9)

## Scope

This runbook defines accessibility and reduced-motion guardrails for critical intervention paths in the operator console:

1. `RiskPostureBanner`
2. `SafetyActionRail`
3. `IncidentTimelineCard`
4. `IncidentAlertsPanel`

## Keyboard-only standards

Critical controls must remain fully operable with keyboard-only input:

1. deterministic tab order across emergency actions (`Pause`, `Reduce-only`, `Cancel-all`, `Resume`)
2. danger confirmation semantics with explicit Enter/Space activation and Escape cancel support
3. explicit focus return behavior after confirmation cancel/close and after danger-action commit
4. visible `:focus-visible` affordance on critical action buttons and incident filter controls

## Assistive announcement evidence contract

Critical confirmation and status updates must remain machine-readable and announcement-friendly.

Required evidence:

1. `reason_code`
2. `resulting_mode` (or readiness status)
3. `timestamp_utc`
4. `action_id`/`run_id`
5. `correlation_id`

Operational expectation:

1. post-action confirmation evidence remains visible and announced within `<= 10s` of critical action execution
2. malformed or missing critical evidence fields fail closed with machine-readable errors (no success-shaped fallback)

## Reduced-motion behavior (`prefers-reduced-motion`)

When `prefers-reduced-motion` is enabled:

1. critical transition animations are disabled for `.risk-banner`, `.safety-action-button`, `.incident-timeline-item`, and `.incident-alert-item`
2. state clarity is preserved with explicit textual status metadata (`State`, `Status`, `reason_code`, timestamps)
3. warning/critical/blocked/completed states remain distinguishable without color-only cues

## Incident and alert parity requirements

Incident timeline and alert panels must preserve keyboard and assistive parity:

1. search and refresh actions provide explicit status + timestamp feedback in live regions
2. loading/empty/error/critical surfaces maintain semantic `role="status"` / `role="alert"` usage
3. runbook links and attempt lists remain semantically structured for screen-reader consumption

## Scope boundary and non-goals

Story 3.9 delivers accessibility and reduced-motion hardening only.

Non-goals:

1. introducing new recovery-policy semantics
2. schema or migration changes
3. new alert orchestration behavior outside Story 3.6 contracts

## Related runbooks

1. Incident forensics workflow: `docs/operations/incident-search-causal-timeline-forensics.md`
2. Severity alert delivery: `docs/operations/severity-alert-delivery.md`
3. Controlled recovery readiness gates: `docs/operations/controlled-recovery-readiness-gates.md`
4. Backup integrity and deterministic restore rehearsal: `docs/operations/backup-integrity-restore-rehearsal.md`
