## Deferred from: code review of 1-1-set-up-initial-project-from-starter-template (2026-04-04T19:19:16Z)

- `.scripts/bmad-auto/copilot/bmad-progress.log` appears in git status but is outside story application scope and not part of the story implementation file list.

## Deferred from: code review of 1-6-add-scheduled-and-emergency-credential-rotation-flows (2026-04-05T15:52:26Z)

- Non-story workspace drift in `.gitignore`, `_bmad/**`, and `.scripts/**` was detected during git cross-check and excluded from application-source review scope.

## Deferred from: code review of 3-4-build-cost-aware-pnl-and-attribution-surfaces (2026-04-06T09:29:22Z)

- `.scripts/bmad-auto/copilot/bmad-progress.log` appeared in git reality but is outside application-source review scope for Story 3.4.

## Deferred from: code review of 4-3-implement-recurring-summary-report-scheduling (2026-04-06T22:38:02Z)

- Add a dedicated due-loader performance index on `(status, next_run_at_utc, schedule_id)` for large production datasets after sizing and query-plan validation.
- `.scripts/bmad-auto/copilot/bmad-progress.log` appeared in git status cross-check but sits outside Story 4.3 application-source review scope.

## Deferred from: code review of 6-2-configure-leakage-and-data-quality-gate-definitions (2026-04-07T11:07:10Z)

- `.scripts/bmad-auto/copilot/bmad-progress.log` appeared in git status cross-check but is outside Story 6.2 application-source review scope.
- Repository-wide `TIMESTAMPTZ` timezone-offset CHECK convention appears across migrations; changing only Story 6.2 constraints requires a coordinated platform migration policy update.

## Deferred from: code review of 6-3-implement-validation-workflow-and-diagnostics-artifact-store (2026-04-07T12:38:04Z)

- Repository-wide `rust:lint` currently fails on pre-existing clippy findings in `crates/domain/src/recovery.rs` (`collapsible_if`), outside Story 6.3 scope.
- `.scripts/bmad-auto/copilot/bmad-progress.log` appeared in git status cross-check but is outside Story 6.3 application-source review scope.
