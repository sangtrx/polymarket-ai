---
phase: 01-canonical-artifact-ingestion
reviewed: 2026-04-09T00:00:00Z
depth: standard
files_reviewed: 18
files_reviewed_list:
  - Cargo.lock
  - crates/domain/src/audit_artifacts.rs
  - crates/domain/src/lib.rs
  - crates/persistence/migrations/20260409000100_canonical_artifact_ingestion.sql
  - crates/persistence/migrations/20260409000200_canonical_snapshot_immutability.sql
  - crates/persistence/src/postgres/canonical_artifacts.rs
  - crates/persistence/src/postgres/mod.rs
  - package.json
  - services/research-gateway/Cargo.toml
  - services/research-gateway/src/ingestion/artifact_discovery.rs
  - services/research-gateway/src/ingestion/artifact_parser.rs
  - services/research-gateway/src/ingestion/mod.rs
  - services/research-gateway/src/ingestion/service.rs
  - services/research-gateway/src/ingestion/snapshot_builder.rs
  - services/research-gateway/src/lib.rs
  - services/research-gateway/src/main.rs
  - tests/api/phase-1-ingestion.test.mjs
  - tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-04-09T00:00:00Z  
**Depth:** standard  
**Files Reviewed:** 18  
**Status:** issues_found

## Summary

Reviewed ingestion/domain/persistence additions for canonical artifact ingestion and snapshot immutability. Core structure is solid, but there are correctness gaps that can cause invalid IDs at persistence time, non-persistent CLI behavior, and artifact misclassification.

## Warnings

### WR-01: Slug normalization allows punctuation that violates canonical ID DB constraints

**File:** `crates/domain/src/audit_artifacts.rs:188-194`  
**Related:** `services/research-gateway/src/ingestion/artifact_parser.rs:287-293`, `crates/persistence/migrations/20260409000100_canonical_artifact_ingestion.sql:30`  
**Issue:** `normalize_slug`/`slugify` only replace spaces/underscores. Characters like `:` `/` `(` `)` remain, but DB constraint for `canonical_requirement_id` section allows only `[a-z0-9_-]`. This can cause runtime insert failures for common heading text.  
**Fix:** Normalize with a strict allowlist and collapse separators before building IDs, e.g. map non `[a-z0-9_-]` to `-`, trim repeated `-`.

### WR-02: CLI ingestion path uses in-memory persistence, so snapshots are not persisted

**File:** `services/research-gateway/src/ingestion/service.rs:265-270`  
**Related:** `services/research-gateway/src/main.rs:60-65`  
**Issue:** Top-level `run_canonical_ingestion` always builds `CanonicalIngestionService::in_memory()`. The CLI uses this path, so no database writes occur despite Postgres persistence implementation/migrations being present.  
**Fix:** In `main.rs`, create `PgPool` from `DATABASE_URL`, construct `PostgresCanonicalSnapshotPersistence`, and invoke `CanonicalIngestionService::new(Arc::new(...))` instead of the in-memory helper for production CLI runs.

### WR-03: Artifact type classification is overly broad (`contains("arch")`)

**File:** `services/research-gateway/src/ingestion/artifact_discovery.rs:91`  
**Related:** `services/research-gateway/src/ingestion/snapshot_builder.rs:162`  
**Issue:** Paths containing substrings like `"search"` can be incorrectly classified as architecture artifacts. This can skew required-class checks and reporting counts.  
**Fix:** Match path segments/filenames with stricter patterns (e.g., `architecture`, `arch-`, `/arch/`) rather than raw substring `arch`.

---

_Reviewed: 2026-04-09T00:00:00Z_  
_Reviewer: the agent (gsd-code-reviewer)_  
_Depth: standard_
