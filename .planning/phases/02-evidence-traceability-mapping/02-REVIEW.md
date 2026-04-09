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

Reviewed phase 02 traceability mapping changes across domain, persistence, service, CLI, migration, and test coverage.  
Core architecture is coherent, but there are correctness gaps in persistence validation consistency and repository scanning behavior that can produce incorrect or failed mapping outcomes.

## Warnings

### WR-01: `missing_evidence` rows can fail persistence due validator mismatch

**File:** `crates/domain/src/traceability.rs:100-117` (also impacts `services/research-gateway/src/traceability/service.rs:132-139`)  
**Issue:** `build_traceability_link` always requires at least one code anchor, but `missing_evidence` is a valid outcome that should allow zero anchors (migration trigger already allows this). This can reject valid rows during Postgres persistence.  
**Fix:** allow zero anchors for `missing_evidence`, while keeping the code-anchor requirement for non-missing outcomes.

### WR-02: Recursive file collection can loop on symlinked directories

**File:** `services/research-gateway/src/main.rs:191-203`  
**Issue:** `collect_source_files` walks directories with `path.is_dir()` and no symlink/visited-dir protection. A symlink cycle can cause unbounded traversal/hang.  
**Fix:** skip symlinked directories (or track canonical visited directories) before recursing.

### WR-03: Corrupt traceability history is silently ignored (fail-open)

**File:** `services/research-gateway/src/main.rs:280-286`  
**Issue:** Invalid `.traceability-history.json` returns an empty map silently, dropping stale-evidence context and changing outcomes without surfacing an error.  
**Fix:** return a typed CLI error for invalid history JSON instead of silently defaulting to empty history.

---

_Reviewed: 2026-04-09T00:00:00Z_  
_Reviewer: gsd-code-reviewer_  
_Depth: standard_
