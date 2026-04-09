---
phase: 03-coverage-classification-matrix
plan: 01
subsystem: coverage
tags: [rust, domain, classifier, tdd]
requires:
  - phase: 02-evidence-traceability-mapping
    provides: traceability outcome/confidence/reason semantics
provides:
  - coverage domain contracts and fail-closed row validation
  - deterministic classifier rules for covered|partial|missing mapping
  - baseline-complete coverage matrix assembly service with provenance
affects: [research-gateway, coverage-audit]
tech-stack:
  added: []
  patterns: [deterministic classification, fail-closed validation, stable ordering]
key-files:
  created:
    - crates/domain/src/coverage.rs
    - services/research-gateway/src/coverage/mod.rs
    - services/research-gateway/src/coverage/classifier.rs
    - services/research-gateway/src/coverage/service.rs
  modified:
    - crates/domain/src/lib.rs
    - services/research-gateway/src/lib.rs
requirements-completed: [COVR-01, COVR-02]
completed: 2026-04-09
---

# Phase 03 Plan 01: Coverage classification core summary

**Implemented typed coverage contracts and deterministic traceability-to-coverage mapping with a complete, sorted matrix output over canonical requirement baselines.**

## Accomplishments

- Added `CoverageClass` contracts with exact serialized values `covered|partial|missing`.
- Added fail-closed `build_coverage_row` validation requiring reason/rationale payloads and preserving separate anchor buckets.
- Implemented rule-table classifier behavior for deterministic covered links, degraded partial links, and explicit missing evidence rows.
- Implemented matrix assembly service that emits one row per canonical requirement and preserves `snapshot_id`, `commit_sha`, and `generated_at_utc`.

## Task Commits

1. **Task 1 + Task 2**
   - `faa1122` feat(03-01): implement coverage classification core

## Verification

- `cargo test -p domain coverage::tests::`
- `cargo test -p research-gateway coverage::tests::classifier_`
- `cargo test -p research-gateway coverage::tests::classifies_all_requirements`

## Self-Check: PASSED

- Found files: `crates/domain/src/coverage.rs`, `services/research-gateway/src/coverage/classifier.rs`, `services/research-gateway/src/coverage/service.rs`
- Found commit: `faa1122`
