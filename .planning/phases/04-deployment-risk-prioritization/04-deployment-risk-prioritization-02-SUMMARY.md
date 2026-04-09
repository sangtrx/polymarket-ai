---
phase: 04-deployment-risk-prioritization
plan: 02
subsystem: risk
tags: [rust, service, ranking, determinism]
requires:
  - phase: 04-deployment-risk-prioritization
    provides: unresolved risk row classification contracts
provides:
  - deterministic risk prioritization service and fix-item schema
  - fail-closed risk input validation and deterministic ranking guardrails
affects: [research-gateway, deployment-audit]
tech-stack:
  added: []
  patterns: [stable comparator chain, contiguous ranking, fail-closed payload validation]
key-files:
  created: []
  modified:
    - services/research-gateway/src/risk/service.rs
    - services/research-gateway/src/risk/mod.rs
requirements-completed: [RISK-01, RISK-02]
completed: 2026-04-09
---

# Phase 04 Plan 02: Risk prioritization service summary

**Implemented deterministic risk ranking that emits complete fix items sorted by deployment impact with stable one-based priority ranks.**

## Accomplishments

- Added `RunRiskPrioritizationInput`, `RiskPrioritizationResult`, and `RiskFixItem` contracts with all required deployment-remediation fields.
- Implemented deterministic ordering with comparator keys: severity desc, `risk_score` desc, `canonical_requirement_id` asc.
- Added contiguous 1-based `priority_rank` assignment after final ordering.
- Added fail-closed payload validation and machine-readable service error codes.
- Added replay and guardrail tests for covered-row exclusion, unknown-reason fallback, and byte-equivalent JSON determinism.

## Task Commits

1. **Task 1: Build risk prioritization service and fix-item output schema**
   - `f5937b5` feat(04-02): add deterministic risk prioritization pipeline
2. **Task 2: Add fail-closed validation and unresolved-row guardrails for prioritization input**
   - `5d54430` feat(04-02): add fail-closed risk prioritization guardrails

## Verification

- `cargo test -p research-gateway risk::tests::ranking_`
- `cargo test -p research-gateway risk::tests::`

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

- Found file: `services/research-gateway/src/risk/service.rs`
- Found commits: `f5937b5`, `5d54430`
