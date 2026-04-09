---
phase: 01-canonical-artifact-ingestion
reviewed: 2026-04-09T07:45:18Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - crates/domain/src/audit_artifacts.rs
  - crates/domain/src/lib.rs
  - crates/persistence/migrations/20260409000100_canonical_artifact_ingestion.sql
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

**Reviewed:** 2026-04-09T07:45:18Z  
**Depth:** standard  
**Files Reviewed:** 16  
**Status:** issues_found

## Summary

Reviewed ingestion domain, parser/discovery, snapshot assembly, persistence SQL/adapter, CLI wiring, and phase-1 tests. Core structure is solid, but there are correctness risks around snapshot identity/versioning, timezone constraints in SQL, and artifact classification false positives.

## Warnings

### WR-01: Snapshot primary key can collide across re-ingestions and overwrite history

**File:** `/home/epic/polymarket-ai/crates/persistence/src/postgres/canonical_artifacts.rs:10-22,273-278`  
**Issue:** `snapshot_id` is derived from `(commit_sha, aggregate_digest, item_count+1)` and does not include `ingested_at_utc`. Re-ingesting the same commit/content at a new timestamp reuses the same `snapshot_id`, hitting `ON CONFLICT (snapshot_id)` and updating the old row instead of creating a new snapshot record.  
**Fix:** Make snapshot identity include ingestion time (or use `UNIQUE(commit_sha, ingested_at_utc)` as conflict target). Example:
```rust
// include ingested_at_utc in deterministic id input
canonical_requirement_id(
    &snapshot.commit_sha,
    &format!("{}-{}", snapshot.aggregate_digest, snapshot.ingested_at_utc),
    snapshot.items.len() as u32 + 1,
)
```
and align SQL upsert conflict target with the intended uniqueness semantics.

### WR-02: Timezone CHECK constraints are environment-dependent and can reject valid writes

**File:** `/home/epic/polymarket-ai/crates/persistence/migrations/20260409000100_canonical_artifact_ingestion.sql:13-14,37,50,66`  
**Issue:** `CHECK (EXTRACT(TIMEZONE FROM ... ) = 0)` on `TIMESTAMPTZ` depends on DB session/server timezone and can fail even for valid UTC instants when DB timezone is non-UTC. This can cause inserts to fail in production despite valid RFC3339 `Z` inputs.  
**Fix:** Remove timezone-offset CHECKs for `TIMESTAMPTZ` columns and enforce UTC at API/domain validation (already done in Rust). Keep columns as `TIMESTAMPTZ` for absolute-time correctness.

### WR-03: Artifact type classification over-matches `"arch"` and can misclassify unrelated files

**File:** `/home/epic/polymarket-ai/services/research-gateway/src/ingestion/artifact_discovery.rs:91-93`  
**Issue:** `lowered.contains("arch")` matches unrelated words (e.g., `research`, `march`), causing wrong artifact typing and potentially incorrect coverage counts.  
**Fix:** Match bounded tokens/filenames instead of substring fragments, e.g. require `architecture` or path segment/file stem boundaries:
```rust
if lowered.contains("architecture")
    || lowered.split(['/', '-', '_', '.']).any(|t| t == "arch") {
    return Some(ArtifactSourceKind::Architecture);
}
```

---

_Reviewed: 2026-04-09T07:45:18Z_  
_Reviewer: gsd-code-reviewer_  
_Depth: standard_
