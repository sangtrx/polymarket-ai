---
phase: 04-deployment-risk-prioritization
plan: 03
subsystem: risk
tags: [rust, postgres, cli, api, e2e]
requires:
  - phase: 04-deployment-risk-prioritization
    provides: deterministic risk scoring and ranking service contracts
provides:
  - immutable risk snapshot persistence schema and adapter
  - prioritize-risk CLI chaining traceability, coverage, and risk output
  - phase-level API/E2E verification and qa:test:phase-4 aggregator
affects: [research-gateway, persistence, ci-audit]
tech-stack:
  added: []
  patterns: [immutable snapshots, JSON-only CLI output, deterministic replay assertions]
key-files:
  created:
    - crates/persistence/migrations/20260409000500_risk_prioritization.sql
    - crates/persistence/src/postgres/risk_prioritization.rs
    - tests/api/phase-4-risk-prioritization.test.mjs
    - tests/e2e/phase-4-risk-prioritization.e2e.test.mjs
  modified:
    - crates/persistence/src/postgres/mod.rs
    - services/research-gateway/src/risk/service.rs
    - services/research-gateway/src/main.rs
    - package.json
requirements-completed: [RISK-01, RISK-02]
completed: 2026-04-09
---

# Phase 04 Plan 03: Persistence, CLI, and QA summary

**Shipped immutable risk persistence plus a JSON-only `prioritize-risk` command with deterministic API/E2E verification coverage.**

## Accomplishments

- Added immutable Postgres schema (`risk_snapshots`, `risk_rows`, `risk_row_anchors`) with trigger-based update/delete rejection.
- Added SQLx risk persistence adapter with typed machine-code error mapping and deterministic list ordering.
- Wired `PostgresRiskPersistence` into risk service persistence ports.
- Added `prioritize-risk` CLI command that executes ingestion -> traceability -> coverage -> risk prioritization in sequence.
- Added phase-4 API and E2E tests plus `qa:test:phase-4` script to run Rust and Node checks in one command.

## Task Commits

1. **Task 1: Add immutable Postgres risk snapshot schema and SQLx adapter**
   - `bda15b7` feat(04-03): add immutable risk snapshot persistence
2. **Task 2: Add prioritize-risk CLI command chaining traceability to coverage to risk**
   - `fb3d979` feat(04-03): add prioritize-risk CLI chain
3. **Task 3: Add phase-4 API/E2E suites and qa:test:phase-4 aggregator**
   - `27ad999` test(04-03): add phase-4 API and E2E risk suites
4. **Post-review hardening fixes**
   - `d1ea94a` fix(04-03): harden ordering and source traversal

## Verification

- `cargo test -p persistence postgres::risk_prioritization::tests::`
- `cargo test -p research-gateway main::tests::prioritize_risk_`
- `node --test tests/api/phase-4-risk-prioritization.test.mjs`
- `node --test tests/e2e/phase-4-risk-prioritization.e2e.test.mjs`

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

- Found files: `crates/persistence/src/postgres/risk_prioritization.rs`, `services/research-gateway/src/main.rs`, `tests/e2e/phase-4-risk-prioritization.e2e.test.mjs`
- Found commits: `bda15b7`, `fb3d979`, `27ad999`, `d1ea94a`
