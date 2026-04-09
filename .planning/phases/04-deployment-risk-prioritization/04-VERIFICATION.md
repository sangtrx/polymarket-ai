---
phase: 04-deployment-risk-prioritization
verified: 2026-04-09T18:38:27Z
status: passed
score: 6/6 must-haves verified
overrides_applied: 0
---

# Phase 4: Deployment Risk Prioritization Verification Report

**Phase Goal:** Users can identify which unresolved coverage gaps most threaten safe deployment.  
**Verified:** 2026-04-09T18:38:27Z  
**Status:** passed  
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Every Partial/Missing requirement can be assigned deployment-impact severity. | ✓ VERIFIED | `classify_coverage_row` excludes only `Covered` and scores unresolved rows via `score_unresolved_row` (`services/research-gateway/src/risk/classifier.rs:28-43`). |
| 2 | Users can generate a deterministic fix list ordered by highest deployment risk first. | ✓ VERIFIED | Comparator chain is severity desc, risk_score desc, canonical_requirement_id asc (`services/research-gateway/src/risk/service.rs:200-206`); ranking tests pass (`services/research-gateway/src/risk/mod.rs:105-125`). |
| 3 | Unknown reason_code fails closed to high severity with machine code mapping. | ✓ VERIFIED | Unmapped reasons map to `RiskSeverity::High` + `risk_weight_unmapped_reason` (`crates/domain/src/risk_prioritization.rs:169-174`), covered by tests (`:200-205`, `services/research-gateway/src/risk/mod.rs:179-190`). |
| 4 | Priority ranks are contiguous and 1-based. | ✓ VERIFIED | Ranks assigned with `enumerate(... index + 1)` (`services/research-gateway/src/risk/service.rs:177-179`), tested (`services/research-gateway/src/risk/mod.rs:127-145`). |
| 5 | `prioritize-risk` CLI emits JSON-only output with risk ranking payload fields. | ✓ VERIFIED | CLI dispatches `PrioritizeRisk`, serializes only JSON (`services/research-gateway/src/main.rs:243-296`, `513-523`), CLI tests assert `priority_rank` and `risk_score` fields (`:927-972`). |
| 6 | Immutable risk snapshots + typed persistence errors + phase QA command are implemented and passing. | ✓ VERIFIED | Migration adds immutable triggers (`crates/persistence/migrations/20260409000500_risk_prioritization.sql:95-115`), adapter maps `risk_invalid_payload|risk_query_failed|risk_constraint_violation` (`crates/persistence/src/postgres/risk_prioritization.rs:95-115`), and `npm run -s qa:test:phase-4` passes with Rust+Node checks (`package.json:58`, command run success). |

**Score:** 6/6 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `crates/domain/src/risk_prioritization.rs` | Deterministic severity/scoring contracts + fail-closed mapping | ✓ VERIFIED | Exists (216 lines), substantive scoring logic and tests. |
| `services/research-gateway/src/risk/classifier.rs` | Deterministic unresolved-row classifier | ✓ VERIFIED | Exists (85 lines), filters covered rows and emits explainability weights. |
| `services/research-gateway/src/risk/service.rs` | Deterministic ordering + priority ranks + persistence handoff | ✓ VERIFIED | Exists (295 lines), wired to classifier and persistence adapter. |
| `services/research-gateway/src/main.rs` | `prioritize-risk` CLI parse + dispatch + JSON output | ✓ VERIFIED | Exists (992 lines), command branch and tests present. |
| `crates/persistence/migrations/20260409000500_risk_prioritization.sql` | Immutable risk snapshot schema | ✓ VERIFIED | Exists (115 lines), insert-only constraints and mutation-reject triggers. |
| `crates/persistence/src/postgres/risk_prioritization.rs` | Typed persistence adapter + deterministic readback | ✓ VERIFIED | Exists (591 lines), transactional insert + deterministic ORDER BY + typed errors. |
| `tests/api/phase-4-risk-prioritization.test.mjs` | API-level risk contracts | ✓ VERIFIED | Exists (119 lines), asserts uncovered-row output + unknown-reason fallback. |
| `tests/e2e/phase-4-risk-prioritization.e2e.test.mjs` | E2E deterministic ordering + contiguous ranks | ✓ VERIFIED | Exists (117 lines), validates full comparator order and ranks. |
| `package.json` | `qa:test:phase-4` aggregator command | ✓ VERIFIED | Script includes required Rust+Node command chain. |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `risk/classifier.rs` | `domain/risk_prioritization.rs` | contract-based severity + scoring functions | ✓ WIRED | `score_unresolved_row` imported/used in classifier. |
| `risk/service.rs` | `risk/classifier.rs` | classification-to-ranking pipeline | ✓ WIRED | `classify_unresolved_rows(&input.coverage_matrix.rows)` used before sorting. |
| `main.rs` | `risk/service.rs` | `prioritize-risk` dispatch | ✓ WIRED | CLI branch builds `RunRiskPrioritizationInput` and calls `run_risk_prioritization`. |
| `risk/service.rs` | `postgres/risk_prioritization.rs` | persist snapshot with typed errors | ✓ WIRED | `insert_risk_snapshot` invoked and mapped to service error codes. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| --- | --- | --- | --- | --- |
| `risk/service.rs` | `scored_rows` / `rows` | `input.coverage_matrix.rows` -> classifier -> sort -> fix items | Yes | ✓ FLOWING |
| `main.rs` | `coverage_output`, risk `output` | ingest -> traceability -> coverage -> risk chain | Yes | ✓ FLOWING |
| `postgres/risk_prioritization.rs` | listed persisted rows | SQL query `LIST_RISK_ROWS_BY_SNAPSHOT_SQL` with ORDER BY | Yes | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| --- | --- | --- | --- |
| Phase QA command covers Rust+Node and passes | `npm run -s qa:test:phase-4` (with `PATH=$HOME/.cargo/bin:$PATH`) | persistence tests pass, risk tests pass, CLI tests pass, Node API/E2E pass | ✓ PASS |
| `prioritize-risk` success payload includes ranking fields | Node spot-check invoking cargo CLI on fixture repo | Output parse succeeded and included `priority_rank`, `risk_score`, `severity`, `canonical_requirement_id` | ✓ PASS |
| `prioritize-risk` fail-closed emits machine-readable JSON error | Node spot-check invoking CLI without `--commit-sha` | Non-zero exit with JSON error code `risk_invalid_payload` | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| --- | --- | --- | --- | --- |
| RISK-01 | 04-01, 04-02, 04-03 | Assign deployment-impact severity to every Partial/Missing requirement | ✓ SATISFIED | Classifier/domain mapping and tests confirm severity assignment + unknown fallback. |
| RISK-02 | 04-02, 04-03 | Generate prioritized fix list ordered by deployment risk | ✓ SATISFIED | Deterministic comparator + contiguous rank assignment + CLI/E2E tests. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| --- | --- | --- | --- | --- |
| `tests/api/phase-4-risk-prioritization.test.mjs` | 19-21 | `placeholder` fixture filename/content | ℹ️ Info | Test fixture seed only; not production logic. |
| `tests/e2e/phase-4-risk-prioritization.e2e.test.mjs` | 19-21 | `placeholder` fixture filename/content | ℹ️ Info | Test fixture seed only; not production logic. |

### Gaps Summary

No blocking gaps found. Phase 4 goal is achieved with deterministic severity assignment, prioritized ranking, immutable persistence, CLI JSON contracts, and passing phase QA coverage.

---

_Verified: 2026-04-09T18:38:27Z_  
_Verifier: the agent (gsd-verifier)_
