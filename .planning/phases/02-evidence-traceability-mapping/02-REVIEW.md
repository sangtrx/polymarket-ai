---
phase: 02-evidence-traceability-mapping
reviewed: 2026-04-09T00:00:00Z
depth: standard
files_reviewed: 13
files_reviewed_list:
  - crates/domain/src/lib.rs
  - crates/domain/src/traceability.rs
  - crates/persistence/migrations/20260409000300_traceability_mapping.sql
  - crates/persistence/src/postgres/mod.rs
  - crates/persistence/src/postgres/traceability.rs
  - package.json
  - services/research-gateway/src/lib.rs
  - services/research-gateway/src/main.rs
  - services/research-gateway/src/traceability/matcher.rs
  - services/research-gateway/src/traceability/mod.rs
  - services/research-gateway/src/traceability/service.rs
  - tests/api/phase-2-traceability.test.mjs
  - tests/e2e/phase-2-traceability-mapping.e2e.test.mjs
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-04-09T00:00:00Z  
**Depth:** standard  
**Files Reviewed:** 13  
**Status:** issues_found

## Summary

Traceability domain, persistence, CLI integration, and phase-2 tests were reviewed.  
Main concerns are in persistence correctness: deferred DB constraints are incompatible with current non-transactional write flow, reason codes are dropped before persistence, and decoded line numbers can be corrupted by unchecked integer casting.

## Warnings

### WR-01: Deferred code-anchor constraint can fail valid inserts due to non-transactional writes

**File:** `/home/epic/polymarket-ai/crates/persistence/src/postgres/traceability.rs:130-166`  
**Related schema:** `/home/epic/polymarket-ai/crates/persistence/migrations/20260409000300_traceability_mapping.sql:118-127`  
**Issue:** `insert_traceability_snapshot` inserts link rows before anchor rows without an explicit transaction. The migration uses `DEFERRABLE INITIALLY DEFERRED` constraint triggers requiring code anchors. Without a multi-statement transaction, constraint checks occur before anchors are inserted, causing valid link inserts to fail.  
**Fix:** Wrap snapshot/link/anchor inserts in a single SQL transaction and commit only after all rows are inserted.

### WR-02: `reason_code` is lost during persistence

**File:** `/home/epic/polymarket-ai/services/research-gateway/src/traceability/service.rs:98-113`  
**Related:** `/home/epic/polymarket-ai/crates/persistence/src/postgres/traceability.rs:147`  
**Issue:** `TraceabilityMappingRow.reason_code` is never persisted. Service maps rows to `TraceabilityLink` (which has no `reason_code`), and persistence hardcodes `.bind(None::<String>)`. This drops explainability data required by traceability output contracts.  
**Fix:** Include `reason_code` in the persistence write model and bind the actual value when inserting links.

### WR-03: Unchecked signed→unsigned cast can corrupt persisted line numbers

**File:** `/home/epic/polymarket-ai/crates/persistence/src/postgres/traceability.rs:256-263`  
**Issue:** `i64` DB values are cast to `u32` via `as`, which wraps negative/out-of-range values silently. Corrupted line anchors can result from malformed data.  
**Fix:** Validate with checked conversion and return a decode error on invalid ranges.

---

_Reviewed: 2026-04-09T00:00:00Z_  
_Reviewer: gsd-code-reviewer_  
_Depth: standard_
