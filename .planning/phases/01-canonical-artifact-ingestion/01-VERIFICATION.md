---
phase: 01-canonical-artifact-ingestion
verified: 2026-04-09T07:51:14Z
status: gaps_found
score: 6/7 must-haves verified
overrides_applied: 0
gaps:
  - truth: "Snapshot metadata is immutable and includes commit SHA, ingested_at timestamp, per-file digest, and aggregate digest."
    status: partial
    reason: "Metadata fields exist, but immutability is not enforced; snapshot upsert path allows metadata updates on conflict."
    artifacts:
      - path: "crates/persistence/src/postgres/canonical_artifacts.rs"
        issue: "UPSERT_SNAPSHOT_SQL uses ON CONFLICT (snapshot_id) DO UPDATE for file_digests_json and aggregate_digest."
      - path: "crates/persistence/migrations/20260409000100_canonical_artifact_ingestion.sql"
        issue: "Schema enforces presence/uniqueness but does not prohibit updates to snapshot metadata columns."
    missing:
      - "Make snapshot persistence insert-only (or immutable-once-created) for metadata fields."
      - "Add a test proving update attempts on existing snapshots are rejected or no-op."
---

# Phase 01: canonical-artifact-ingestion Verification Report

**Phase Goal:** Users can load BMAD planning artifacts into one normalized, versioned audit baseline.  
**Status:** gaps_found

## Goal Achievement

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | User can ingest PRD, architecture, story, and roadmap artifacts into one canonical dataset. | VERIFIED | `services/research-gateway/src/ingestion/service.rs` requires all artifact classes; `tests/api/phase-1-ingestion.test.mjs` validates class counts. |
| 2 | User can see stable requirement IDs assigned across all ingested BMAD items. | VERIFIED | Deterministic ID generation in `crates/domain/src/audit_artifacts.rs`; E2E asserts stable IDs. |
| 3 | User can identify snapshot version metadata for each ingestion run. | VERIFIED | CLI result includes `commit_sha`, `ingested_at_utc`, `snapshot_digest` and artifact counts. |
| 4 | Ingestion accepts only scoped source locations and requirement-bearing structures. | VERIFIED | Discovery roots are restricted and parser extracts headings/checklists/acceptance tables. |
| 5 | Schema errors fail ingestion while minor quality issues emit warnings. | VERIFIED | Parser returns machine-readable schema errors and warning findings for minor quality issues. |
| 6 | Replay is stable for unchanged inputs. | VERIFIED | `tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` asserts stable IDs and digest across reruns. |
| 7 | Snapshot metadata is immutable (commit/timestamp/digests). | PARTIAL | Snapshot upsert currently permits metadata updates on conflict. |

## Requirement Coverage

- ARTF-01: satisfied
- ARTF-02: satisfied
- ARTF-03: satisfied
- ARTF-04: satisfied
- ARTF-05: partially satisfied (immutability gap)

## Behavioral Evidence

- `npm run -s qa:test:phase-1` passed in the orchestrator environment.
- API and E2E phase-1 tests passed.

## Gaps Summary

One gap blocks full completion:

1. Snapshot metadata immutability is not strictly enforced due upsert update semantics.

Recommended closure:

1. Make snapshot metadata insert-once immutable in persistence.
2. Add regression coverage proving existing snapshot metadata cannot be mutated.

