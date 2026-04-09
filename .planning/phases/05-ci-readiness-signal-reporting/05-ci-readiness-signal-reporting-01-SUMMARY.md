---
phase: 05-ci-readiness-signal-reporting
plan: 01
subsystem: database
tags: [readiness, waivers, postgres, rust, fail-closed]
requires:
  - phase: 04-deployment-risk-prioritization
    provides: deterministic unresolved risk rows and risk snapshot schema
provides:
  - readiness/waiver validation contracts in domain
  - immutable waiver persistence schema with unresolved-risk gating
  - deterministic active/expiring/expired waiver query surfaces
affects: [research-gateway, reporting-service, control-api]
tech-stack:
  added: []
  patterns: [machine-readable invalid payload contracts, immutable append-only waiver revocation ledger]
key-files:
  created:
    - crates/domain/src/readiness.rs
    - crates/persistence/migrations/20260409000600_ci_readiness.sql
    - crates/persistence/src/postgres/readiness.rs
  modified:
    - crates/domain/src/lib.rs
    - crates/persistence/src/postgres/mod.rs
key-decisions:
  - "Use append-only waiver + revocation tables instead of mutable waiver state updates."
  - "Gate waiver inserts against unresolved partial|missing risk rows before persistence."
patterns-established:
  - "Readiness contracts mirror reporting domain typed field-error validation style."
  - "Waiver listing queries sort by expires_at_utc, canonical_requirement_id, and waiver_id for deterministic replay."
requirements-completed: [GATE-03]
duration: 3min
completed: 2026-04-09
---

# Phase 05 Plan 01: CI Readiness Signal & Reporting Summary

**Readiness waiver contracts and immutable persistence now enforce governed metadata, UTC validation, unresolved-risk eligibility, and deterministic waiver listing behavior.**

## Performance

- **Duration:** 3 min
- **Started:** 2026-04-09T19:38:40Z
- **Completed:** 2026-04-09T19:41:18Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added `domain::readiness` with readiness/waiver enums, reason codes, UTC parser, and fail-closed waiver validation.
- Added immutable readiness migration with unresolved-risk trigger and append-only revocation ledger.
- Added Postgres readiness adapter with insert/revoke/list APIs and machine-readable persistence errors.

## Task Commits

1. **Task 1: Add readiness/waiver domain contracts with fail-closed validation** - `e3e81b7` (feat)
2. **Task 2: Add immutable readiness/waiver persistence schema and deterministic read adapters** - `226b34d` (feat)

## Files Created/Modified
- `crates/domain/src/readiness.rs` - Domain contracts, validators, UTC timestamp parsing, waiver state resolution tests.
- `crates/domain/src/lib.rs` - Exports readiness module.
- `crates/persistence/migrations/20260409000600_ci_readiness.sql` - Readiness snapshot/waiver schema, unresolved-only trigger, immutability triggers.
- `crates/persistence/src/postgres/readiness.rs` - Typed SQL adapter and deterministic waiver list query contracts.
- `crates/persistence/src/postgres/mod.rs` - Exports readiness persistence module.

## Decisions Made
- Modeled waiver revocation as append-only records to preserve immutable history while supporting deterministic revoked state.
- Enforced unresolved-risk eligibility both in SQL trigger and adapter pre-check to fail closed before writes.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Cargo binary not on PATH**
- **Found during:** Task 1 verification
- **Issue:** `cargo test` failed with `cargo: command not found`
- **Fix:** Executed verification commands using `~/.cargo/bin/cargo`
- **Files modified:** none
- **Verification:** All plan verification commands passed with absolute cargo path
- **Committed in:** n/a

---

**Total deviations:** 1 auto-fixed (Rule 3: 1)
**Impact on plan:** No scope change; only execution environment adjustment.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Readiness/waiver contracts and persistence primitives are in place for phase-5 scoring and reporting orchestration.
- Deterministic waiver state surfaces (`active`, `expired`, `revoked`) are available for CI/reporting consumers.

## Self-Check: PASSED
