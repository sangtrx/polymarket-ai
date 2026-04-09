---
phase: 04-deployment-risk-prioritization
plan: 01
subsystem: risk
tags: [rust, domain, classifier, determinism]
requires:
  - phase: 03-coverage-classification-matrix
    provides: coverage row taxonomy and explainability payloads
provides:
  - deterministic risk severity/scoring contracts
  - unresolved-row risk classifier with fail-closed reason handling
affects: [research-gateway, deployment-audit]
tech-stack:
  added: []
  patterns: [fail-closed reason mapping, deterministic score explainability]
key-files:
  created:
    - crates/domain/src/risk_prioritization.rs
    - services/research-gateway/src/risk/mod.rs
    - services/research-gateway/src/risk/classifier.rs
  modified:
    - crates/domain/src/lib.rs
    - services/research-gateway/src/lib.rs
requirements-completed: [RISK-01]
completed: 2026-04-09
---

# Phase 04 Plan 01: Risk contracts and classifier summary

**Delivered deterministic risk severity/scoring contracts and an unresolved-row classifier that fails closed for unknown reason lineage.**

## Accomplishments

- Added domain risk contracts (`RiskSeverity`, `RemediationFocus`, `RiskScoreBreakdown`) with deterministic weight tables.
- Added fail-closed unknown-reason behavior that maps to `RiskSeverity::High` with machine code `risk_weight_unmapped_reason`.
- Added research-gateway risk classifier that excludes covered rows and emits explainability weights (`severity_weight`, `reason_weight`, `evidence_penalty`).
- Added deterministic classifier tests for covered-row exclusion, unknown-reason fallback, and replay stability.

## Task Commits

1. **Task 1: Create risk prioritization domain contracts and deterministic weight tables**
   - `742c216` feat(04-01): add deterministic risk prioritization contracts
2. **Task 2: Implement research-gateway risk classifier module for unresolved coverage rows only**
   - `c079436` feat(04-01): add deterministic unresolved risk classifier

## Verification

- `cargo test -p domain risk_prioritization::tests::`
- `cargo test -p research-gateway risk::tests::severity_`

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

- Found files: `crates/domain/src/risk_prioritization.rs`, `services/research-gateway/src/risk/classifier.rs`
- Found commits: `742c216`, `c079436`
