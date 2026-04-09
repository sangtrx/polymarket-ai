---
phase: 03-coverage-classification-matrix
plan: 03
subsystem: coverage
tags: [rust, cli, node-test, e2e]
requires:
  - phase: 03-01
    provides: coverage classification service
  - phase: 03-02
    provides: persistence and fail-closed coverage service behavior
provides:
  - classify-coverage CLI command and input assembly flow
  - phase-3 API and E2E contract suites
  - qa:test:phase-3 one-command validation entrypoint
affects: [research-gateway, tests, release-readiness]
tech-stack:
  added: []
  patterns: [machine-readable CLI errors, deterministic replay checks, requirement-level contract assertions]
key-files:
  created:
    - tests/api/phase-3-coverage-classification.test.mjs
    - tests/e2e/phase-3-coverage-matrix.e2e.test.mjs
  modified:
    - services/research-gateway/src/main.rs
    - package.json
requirements-completed: [COVR-01, COVR-02, COVR-03]
completed: 2026-04-09
---

# Phase 03 Plan 03: Coverage CLI and QA summary

**Exposed coverage classification as `classify-coverage` and added end-to-end phase QA suites proving complete, explainable, and deterministic matrix behavior.**

## Accomplishments

- Added `classify-coverage` CLI parser/runner path with required flags (`--commit-sha`, `--generated-at-utc`, `--repo-root`).
- Wired CLI flow to build traceability input, run traceability mapping, then run coverage classification with baseline requirement IDs.
- Added `main::tests::classify_coverage_*` Rust command-contract tests for parse, output payload, and fail-closed error behavior.
- Added phase-3 API/E2E Node suites and consolidated `qa:test:phase-3` script.

## Task Commits

1. **Task 1 + Task 2**
   - `f27caff` feat(03-03): add classify-coverage CLI and phase QA

## Verification

- `cargo test -p research-gateway main::tests::classify_coverage_`
- `node --test tests/api/phase-3-coverage-classification.test.mjs`
- `node --test tests/e2e/phase-3-coverage-matrix.e2e.test.mjs`
- `npm run -s qa:test:phase-3`

## Self-Check: PASSED

- Found files: `services/research-gateway/src/main.rs`, `tests/api/phase-3-coverage-classification.test.mjs`, `tests/e2e/phase-3-coverage-matrix.e2e.test.mjs`, `package.json`
- Found commit: `f27caff`
