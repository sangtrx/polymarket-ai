# Recurring Report Scheduling Operations Runbook

## Scope

This runbook covers Story 4.3 recurring summary scheduling for report generation and run-history evidence (`report_schedules`, `report_runs`).

## Cadence Policy (UTC-only)

- **Daily:** executes at `00:00 UTC`.
- **Weekly:** executes at `Monday 00:00 UTC`.
- **Monthly:** executes at `day 1, 00:00 UTC`.

All schedule boundaries are deterministic and UTC-only. Local time zones and DST adjustments are not applied.

## Control-Plane Endpoints

- `POST /control/report-schedules/{schedule_id}` — create/update schedule.
- `POST /control/report-schedules/{schedule_id}/pause` — pause schedule execution.
- `POST /control/report-schedules/{schedule_id}/resume` — resume execution and recompute next UTC boundary.
- `GET /control/report-schedules/{schedule_id}/runs` — query bounded run history evidence.

All endpoints require authenticated control-plane authorization and emit machine-readable failures when denied.

## Scheduler Lifecycle

Each execution window persists deterministic run transitions:

`pending -> running -> succeeded | failed | missed`

Run evidence includes schedule id, cadence, UTC window bounds, reason code, actor/source context, correlation id, and timestamps.

## Missed Runs and Recovery

A due schedule that has not started within `30 seconds` of its boundary is marked `missed` with explicit reason code evidence and no success fallback.

Recommended operator flow:

1. Query `GET /control/report-schedules/{schedule_id}/runs` for failed/missed windows.
2. Pause the schedule when containment is required.
3. Restore dependencies and resume the schedule.
4. Verify the next run transitions to `succeeded`.

## Critical Failure Escalation (NFR15)

For critical run failures (dependency unavailable, stale evidence, persistence unavailable), the scheduler emits critical incident-alert evidence within 30 seconds, including:

- failure cause,
- impacted system (`reporting-service scheduler`),
- runbook reference (`https://docs.example.com/operations/recurring-report-scheduling`),
- correlation linkage to the failed `report_runs` record.

If deterministic alert evidence cannot be emitted, execution fails closed and does not produce success-shaped completion.
