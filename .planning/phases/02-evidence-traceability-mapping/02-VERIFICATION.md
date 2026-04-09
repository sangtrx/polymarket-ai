---
phase: 02-evidence-traceability-mapping
verified: 2026-04-09T09:40:30Z
status: gaps_found
score: 3/5 must-haves verified
overrides_applied: 0
gaps:
  - truth: "User can map every canonical requirement ID to one or more code evidence references."
    status: failed
    reason: "Traceability candidate discovery treats Markdown/docs as code evidence and allows rows with zero code anchors."
    artifacts:
      - path: "services/research-gateway/src/main.rs"
        issue: "is_supported_source_file includes .md; candidate_from_file classifies non-test files as code"
      - path: "tests/api/phase-2-traceability.test.mjs"
        issue: "Contract test only asserts code_anchors is an array, not non-empty implementation evidence"
    missing:
      - "Restrict code evidence candidates to implementation artifacts (e.g., source/config needed for behavior), not planning markdown"
      - "Enforce and test non-empty code_anchors for mapped (non-missing) outcomes"
  - truth: "Traceability persistence foundation is validated and reliably wired for deterministic writes/reads."
    status: failed
    reason: "Persistence path bypasses domain validator and link insert ordering can violate deferred code-anchor constraint."
    artifacts:
      - path: "services/research-gateway/src/traceability/service.rs"
        issue: "Builds TraceabilityLink structs directly; build_traceability_link validator is not used"
      - path: "crates/persistence/src/postgres/traceability.rs"
        issue: "Inserts link before anchors without explicit transaction, while migration enforces deferred code-anchor constraint trigger"
    missing:
      - "Use validated link construction before persistence (or enforce equivalent validation at persistence boundary)"
      - "Wrap snapshot/link/anchor inserts in one SQL transaction so deferred constraints evaluate after anchors are inserted"
---

# Phase 2: Evidence Traceability Mapping Verification Report

**Phase Goal:** Users can trace each canonical requirement to concrete implementation and validation evidence.  
**Verified:** 2026-04-09T09:40:30Z  
**Status:** gaps_found  
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | User can map every canonical requirement ID to one or more code evidence references. | ✗ FAILED | `services/research-gateway/src/main.rs:333-337` includes `.md` in candidate scan; `candidate_from_file` marks non-test files as code (`309-317`). |
| 2 | User can attach related test evidence to requirement mappings when available. | ✓ VERIFIED | Matcher and CLI emit `test_anchors`; API test asserts test evidence appears after assertion-backed fixture (`tests/api/phase-2-traceability.test.mjs:79-106`). |
| 3 | User can review rationale and confidence for every traceability link. | ✓ VERIFIED | Output row contains `rationale` and `confidence` fields in service DTO (`services/research-gateway/src/traceability/service.rs:38-48`) and CLI contract test (`main.rs:491-510`). |
| 4 | Missing, ambiguous, and stale evidence outcomes are explicit. | ✓ VERIFIED | `LinkOutcome` includes all states (`crates/domain/src/traceability.rs:61-66`); matcher emits missing/stale/ambiguous explicitly (`matcher.rs:73-109`, `121-130`). |
| 5 | Persistence foundation is contract-validated and reliably wired. | ✗ FAILED | Domain validator function exists but is unused outside its own tests (`grep build_traceability_link`); insert ordering + deferred trigger mismatch (`traceability.rs:138-166` vs migration `...sql:118-127`). |

**Score:** 3/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/domain/src/traceability.rs` | Contracts/validation for anchors/rationale/confidence/outcomes | ⚠️ PARTIAL | Exists and substantive, but validator path is not enforced by persistence/service write flow. |
| `crates/persistence/migrations/20260409000300_traceability_mapping.sql` | Traceability schema and constraints | ✓ VERIFIED | Tables/constraints/triggers for outcomes, confidence enum, and immutability exist. |
| `crates/persistence/src/postgres/traceability.rs` | Deterministic insert/list adapter with typed errors | ⚠️ HOLLOW | Query constants and deterministic read order exist; write flow has deferred-trigger ordering risk. |
| `services/research-gateway/src/traceability/service.rs` | Mapping orchestration + fallback handling | ✓ VERIFIED | Deterministic-first + semantic fallback + explicit outcomes implemented. |
| `services/research-gateway/src/main.rs` | `trace-evidence` CLI output | ⚠️ PARTIAL | CLI works structurally, but evidence candidate scope includes markdown, weakening “concrete implementation evidence.” |
| `tests/api/phase-2-traceability.test.mjs` | TRAC contract checks | ⚠️ PARTIAL | Asserts field presence but does not enforce non-empty code evidence for mapped rows. |
| `tests/e2e/phase-2-traceability-mapping.e2e.test.mjs` | Deterministic replay + unresolved outcomes | ✓ VERIFIED | Replay ordering and explicit ambiguous/missing/stale checks are present. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `crates/domain/src/traceability.rs` | `crates/persistence/src/postgres/traceability.rs` | validated `TraceabilityLink` structs | ⚠️ PARTIAL | Type is shared, but `build_traceability_link` validator is not used in live write path. |
| `crates/persistence/src/postgres/traceability.rs` | migration SQL | traceability table queries | ✓ WIRED | Adapter SQL references migration tables with deterministic `ORDER BY`. |
| `services/research-gateway/src/traceability/service.rs` | persistence adapter | `persist_traceability` | ⚠️ PARTIAL | Port is wired, but default exported runner uses in-memory persistence only. |
| `services/research-gateway/src/main.rs` | traceability service | CLI dispatch | ✓ WIRED | `trace-evidence` command builds mapping input and calls `run_traceability_mapping`. |
| `tests/api/phase-2-traceability.test.mjs` | `trace-evidence` CLI | `spawnSync cargo run ... trace-evidence` | ✓ WIRED | API contract suite executes the CLI command path. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `services/research-gateway/src/main.rs` | `requirements` / candidates | `run_canonical_ingestion` + filesystem scan | Yes, but includes `.md` non-implementation sources | ⚠️ FLOWING_WITH_NOISE |
| `services/research-gateway/src/traceability/service.rs` | `rows` | `match_requirement_to_evidence(...)` | Yes | ✓ FLOWING |
| `crates/persistence/src/postgres/traceability.rs` | `records` | SQL query over traceability tables | Yes | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Phase-2 QA command runs | `npm run qa:test:phase-2` | `cargo: not found` | ? SKIP (environment missing Rust toolchain) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| TRAC-01 | 02-01, 02-02 | Map each canonical requirement ID to code evidence references | ✗ BLOCKED | Candidate generation currently accepts markdown/docs as code evidence and tests do not enforce non-empty mapped code anchors. |
| TRAC-02 | 02-02 | Map requirement IDs to related test evidence when available | ✓ SATISFIED | Matcher carries test anchors; API test verifies test anchor emission after adding assertion-backed test file. |
| TRAC-03 | 02-01, 02-02 | Record rationale and confidence for every traceability link | ✓ SATISFIED | Domain + service models require/emit rationale/confidence; CLI tests assert both fields in output. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `services/research-gateway/src/main.rs` | 333-337 | Markdown (`.md`) treated as source evidence input | 🛑 Blocker | Can misclassify docs as implementation evidence, weakening TRAC-01 integrity. |
| `crates/persistence/src/postgres/traceability.rs` | 138-166 | Multi-step insert without explicit transaction with deferred constraints in migration | 🛑 Blocker | Valid non-missing link writes can fail due code-anchor trigger timing. |
| `crates/persistence/src/postgres/traceability.rs` | 262-263 | unchecked `i64 as u32` cast | ⚠️ Warning | Potential line-anchor corruption on malformed DB values. |
| `crates/persistence/src/postgres/traceability.rs` | 147 | hardcoded `reason_code` bind to `None` | ⚠️ Warning | Drops explainability metadata at persistence boundary. |

### Gaps Summary

Phase 2 has core mapping scaffolding and explicit unresolved outcomes, but two goal-blocking issues remain: (1) evidence qualification is too permissive (docs can be counted as code evidence), and (2) persistence reliability/validation wiring is incomplete (validator bypass + deferred-trigger write ordering risk).  
Closure path: tighten candidate filters + add strict TRAC-01 assertions, then enforce validated persistence writes inside an explicit SQL transaction.

---

_Verified: 2026-04-09T09:40:30Z_  
_Verifier: the agent (gsd-verifier)_
