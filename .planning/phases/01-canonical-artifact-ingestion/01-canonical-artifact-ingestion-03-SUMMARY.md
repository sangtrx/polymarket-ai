---
phase: 01-canonical-artifact-ingestion
plan: 03
subsystem: api
tags: [rust, cli, ingestion, determinism, testing]
requires:
  - phase: 01-02
    provides: discovery/parser/snapshot assembly primitives
provides:
  - Executable `ingest-artifacts` CLI command for canonical full-snapshot ingestion
  - Deterministic snapshot metadata output with canonical ID and conflict evidence
  - Phase-level API and E2E tests covering ARTF-01..ARTF-05
affects: [phase-02, traceability-mapping, qa-gates]
tech-stack:
  added: [sha2]
  patterns: [typed machine-readable CLI errors, deterministic digest replay tests]
key-files:
  created:
    - services/research-gateway/src/ingestion/service.rs
    - tests/api/phase-1-ingestion.test.mjs
    - tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs
  modified:
    - services/research-gateway/src/ingestion/mod.rs
    - services/research-gateway/src/main.rs
    - services/research-gateway/Cargo.toml
    - Cargo.lock
    - package.json
key-decisions:
  - "Canonical ingestion runs full-snapshot mode only and rejects delta mode."
  - "CLI errors remain machine-readable JSON with explicit error codes."
  - "Determinism evidence includes snapshot_digest plus canonical_requirement_ids for replay assertions."
patterns-established:
  - "Phase-level Node tests execute the real Rust CLI against fixture repositories."
requirements-completed: [ARTF-01, ARTF-02, ARTF-03, ARTF-04, ARTF-05]
duration: 8 min
completed: 2026-04-09
---

# Phase 01 Plan 03: Canonical ingestion execution summary

**Research-gateway now exposes a runnable canonical ingestion CLI with deterministic digest/ID evidence, conflict visibility, and one-command phase QA coverage.**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-09T07:31:32Z
- **Completed:** 2026-04-09T07:39:31Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments
- Implemented `ingestion/service.rs` orchestration (discovery → parser → snapshot builder → persistence) with full-snapshot enforcement.
- Added `ingest-artifacts` CLI command with required args, deterministic metadata output, and non-zero machine-readable failures.
- Added `tests/api/phase-1-ingestion.test.mjs` and `tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` for ARTF-01..ARTF-05 coverage.
- Added `qa:test:phase-1` script to run Rust ingestion tests plus phase API/E2E checks.

## Task Commits

1. **Task 1: Implement canonical ingestion orchestration and CLI**
   - `dc1e800` test(01-03): add failing ingestion orchestration and CLI command tests
   - `7913fdf` feat(01-03): implement canonical ingestion service and ingest-artifacts CLI
   - `5f4a81f` chore(01-03): refresh Cargo.lock for ingestion sha2 dependency
2. **Task 2: Add phase API/E2E deterministic tests**
   - `cdc76b8` test(01-03): add failing phase-1 API and e2e ingestion tests
   - `e66fc23` feat(01-03): add phase-1 ingestion API and deterministic e2e test coverage
3. **Task 3: Add phase verification script alias**
   - `47a076f` chore(01-03): add phase-1 canonical ingestion verification script

## Files Created/Modified
- `services/research-gateway/src/ingestion/service.rs` - canonical ingestion orchestrator, persistence ports, deterministic output model, and tests.
- `services/research-gateway/src/main.rs` - CLI command parsing/execution for `ingest-artifacts` with structured error payloads.
- `tests/api/phase-1-ingestion.test.mjs` - ARTF-01..ARTF-04 contract checks + conflict visibility + failure contract.
- `tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` - ARTF-05 replay determinism checks for `snapshot_digest` and `canonical_requirement_ids`.
- `package.json` - adds `qa:test:phase-1`.
- `services/research-gateway/Cargo.toml`, `Cargo.lock` - adds/stabilizes `sha2` dependency for deterministic digests.

## Decisions Made
- Used explicit full-snapshot mode enforcement in service layer to satisfy D-10 and threat T-01-08.
- Emitted deterministic replay evidence (`snapshot_digest`, `canonical_requirement_ids`) and unresolved conflict records to satisfy D-09/D-11/T-01-09.
- Validated CLI required fields and RFC3339 UTC timestamps with typed error codes to satisfy T-01-07.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added canonical ID/conflict evidence to ingestion output**
- **Found during:** Task 2 (GREEN)
- **Issue:** Initial ingestion output lacked explicit unresolved conflict records and canonical ID lists required for deterministic E2E assertions.
- **Fix:** Added `canonical_requirement_ids` and `unresolved_conflicts` to CLI/service output payload.
- **Files modified:** `services/research-gateway/src/ingestion/service.rs`
- **Verification:** `node --test tests/api/phase-1-ingestion.test.mjs tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs`
- **Committed in:** `e66fc23`

**2. [Rule 3 - Blocking] Node tests could not invoke cargo from default PATH**
- **Found during:** Task 2 (RED→GREEN)
- **Issue:** `spawnSync("cargo", ...)` failed in Node test runtime where cargo was not discoverable.
- **Fix:** Updated tests to resolve cargo via `${HOME}/.cargo/bin/cargo` fallback and safe stderr/stdout handling.
- **Files modified:** `tests/api/phase-1-ingestion.test.mjs`, `tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs`
- **Verification:** phase API/E2E test commands passed.
- **Committed in:** `e66fc23`

**3. [Rule 3 - Blocking] Added sha2 dependency + lockfile for deterministic digest generation**
- **Found during:** Task 1 (GREEN)
- **Issue:** Service required deterministic cryptographic hashing for stable digest behavior.
- **Fix:** Added `sha2.workspace = true` and committed lockfile refresh.
- **Files modified:** `services/research-gateway/Cargo.toml`, `Cargo.lock`
- **Verification:** `npm run qa:test:phase-1`
- **Committed in:** `7913fdf`, `5f4a81f`

---

**Total deviations:** 3 auto-fixed (Rule 2: 1, Rule 3: 2)  
**Impact on plan:** All deviations were required for deterministic behavior, correctness, or executable verification; no scope creep beyond plan goal.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Canonical ingestion is runnable and deterministic with replay evidence.
- ARTF-01..ARTF-05 checks are automated behind `npm run qa:test:phase-1`.

## Self-Check: PASSED
- Found summary file `.planning/phases/01-canonical-artifact-ingestion/01-canonical-artifact-ingestion-03-SUMMARY.md`
- Found commits: `dc1e800`, `7913fdf`, `5f4a81f`, `cdc76b8`, `e66fc23`, `47a076f`
