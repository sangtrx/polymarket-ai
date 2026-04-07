# Report Export Workflows Operations Runbook

## Scope

This runbook covers Story 4.4 export workflow orchestration for FR36 evidence packages across `export_jobs` and `export_artifacts`.

## Weekly Scheduled Flow

1. Scheduler completes a weekly run in `report_runs`.
2. Reporting service dispatches `scheduled_weekly` export creation using schedule linkage (`schedule_id`, `schedule_window_key`, `report_run_id`).
3. Export lifecycle persists deterministic transitions:

`queued -> running -> succeeded | failed`

4. Weekly replay stays idempotent through `uq_export_jobs_weekly_idempotency`.

## On-Demand Trigger Flow

- Endpoint: `POST /control/report-exports/on-demand`
- Authorization: control mutation roles through critical-action policy.
- Required evidence: actor, role, correlation id, UTC request time, and machine-readable reason code.

A successful trigger returns accepted job evidence with package reference/checksum metadata.

## Incident-Triggered Flow

- Endpoint: `POST /control/report-exports/incidents/{incident_id}`
- Required payload field: `incident_severity`.
- Incident context is persisted on the export job for postmortem traceability.

If incident export assembly fails (dependency/staleness/persistence/integrity), the job remains `failed`, emits alert-compatible evidence, and never returns a synthetic success.
The workflow fails closed when required FR36 artifacts are unavailable or integrity checks do not pass.

## Retrieval and Audit Procedure

- `GET /control/report-exports/{job_id}` returns export job lifecycle evidence.
- `GET /control/report-exports/{job_id}/artifacts` returns deterministically ordered artifact metadata.
- `GET /control/report-exports/{job_id}/artifacts/{artifact_id}` returns canonical artifact retrieval metadata.

Read routes require analytics/read authorization. Mutation and read operations append privileged audit records with actor/action/parameters/reason/correlation/timestamp evidence.

## Failure-Mode Remediation

When exports fail:

1. Inspect machine-readable reason code and missing artifact categories.
2. Restore dependency availability and evidence freshness for failed artifact sources.
3. Re-run with on-demand or incident trigger, preserving correlation linkage.
4. Confirm artifacts include all required FR36 categories:
   - promotion decisions
   - validation evidence
   - reconciliation summary
   - access audits
   - incident postmortems

Runbook links:

- Recurring scheduler operations: `docs/operations/recurring-report-scheduling.md`
- Severity alert delivery: `docs/operations/severity-alert-delivery.md`
