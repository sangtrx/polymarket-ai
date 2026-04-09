---
phase: 01-canonical-artifact-ingestion
verified: 2026-04-09T08:22:00Z
status: passed
score: 7/7 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 6/7
  gaps_closed:
    - "Snapshot metadata is immutable and includes commit SHA, ingested_at timestamp, per-file digest, and aggregate digest."
  gaps_remaining: []
  regressions: []
---

# Phase 01: canonical-artifact-ingestion Verification Report

**Phase Goal:** Users can load BMAD planning artifacts into one normalized, versioned audit baseline.  
**Verified:** 2026-04-09T08:22:00Z  
**Status:** passed  
**Re-verification:** Yes — after gap closure

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | User can ingest PRD, architecture, story, and roadmap artifacts into one canonical dataset. | ✓ VERIFIED | `run_canonical_ingestion` enforces required classes (`service.rs:326-337`); API test validates all 4 classes (`tests/api/phase-1-ingestion.test.mjs:49-70`). |
| 2 | User can see stable requirement IDs assigned across all ingested BMAD items. | ✓ VERIFIED | Deterministic ID generator (`audit_artifacts.rs:101-121`); E2E replay asserts same `canonical_requirement_ids` (`tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs:53-63`). |
| 3 | User can identify snapshot version metadata for each ingestion run. | ✓ VERIFIED | Ingestion result includes `commit_sha`, `ingested_at_utc`, `snapshot_digest` (`service.rs:42-52`, `237-261`), asserted in API test (`phase-1-ingestion.test.mjs:63-66`). |
| 4 | Ingestion accepts only scoped source locations and requirement-bearing structures. | ✓ VERIFIED | Discovery roots restricted to `.planning`, `docs`, `_bmad`, `_bmad-output`, `.bmad` (`artifact_discovery.rs:23-29`); parser extracts headings/checklists/acceptance tables (`artifact_parser.rs:55-92`, `137-220`). |
| 5 | Schema errors fail ingestion while minor quality issues emit warnings. | ✓ VERIFIED | Parser fails malformed schema with `canonical_artifact_invalid_payload` (`artifact_parser.rs:145-190`, tests `350-362`); warning path for missing source IDs (`112-119`, `365-377`). |
| 6 | Replay is stable for unchanged inputs. | ✓ VERIFIED | Service determinism test passes (`service.rs:531-572`); E2E asserts stable digest and IDs (`phase-1-canonical-ingestion.e2e.test.mjs:59-63`). |
| 7 | Snapshot metadata is immutable (commit/timestamp/digests). | ✓ VERIFIED | Snapshot persistence is insert-only (`canonical_artifacts.rs:9-17`, `110-135`); DB trigger blocks metadata updates (`20260409000200...sql:1-23`); immutability regression tests passed (`cargo test -p persistence ...snapshot_metadata_immutable_` and `...migration_contract_snapshot_immutability_`). |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `crates/persistence/src/postgres/canonical_artifacts.rs` | Insert-only snapshot persistence and typed constraint errors | ✓ VERIFIED | Exists, substantive, wired via `pg_insert_snapshot` call path from service. |
| `crates/persistence/migrations/20260409000200_canonical_snapshot_immutability.sql` | DB-level immutability guard on snapshot metadata | ✓ VERIFIED | Trigger + exception guard present for commit/timestamp/digests. |
| `services/research-gateway/src/ingestion/service.rs` | Ingestion orchestration + snapshot identity composition | ✓ VERIFIED | Uses discovery→parser→snapshot builder→persistence; timestamp-aware snapshot identity. |
| `tests/api/phase-1-ingestion.test.mjs` | ARTF-01..04 API-level behavior checks | ✓ VERIFIED | Non-empty coverage and fail-closed schema behavior verified. |
| `tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` | ARTF-05 replay determinism check | ✓ VERIFIED | Re-run stability on digest + canonical IDs. |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `artifact_discovery.rs` | `artifact_parser.rs` | filtered source manifest | ✓ WIRED | `service.rs` discovers manifest then parses each source (`201-205`, `340-364`). |
| `artifact_parser.rs` | `snapshot_builder.rs` | parsed item stream with validation outcomes | ✓ WIRED | Parsed artifacts flow into `build_snapshot_assembly` (`207-213`). |
| `main.rs` | `ingestion/service.rs` | CLI command handler invokes ingestion service | ✓ WIRED | CLI invokes `run_canonical_ingestion` (`main.rs:57-67`). |
| `service.rs` | `canonical_artifacts.rs` | snapshot insert call path | ✓ WIRED | Postgres persistence calls `pg_insert_snapshot` then item/equivalence/conflict writes (`service.rs:124-159`). |
| `canonical_artifacts.rs` | `20260409000200...sql` | immutable metadata rejection path | ✓ WIRED | Migration contract tests assert immutability trigger/columns (`canonical_artifacts.rs:335-352`). |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| --- | --- | --- | --- | --- |
| `service.rs` | `snapshot_digest` | `aggregate_digest(file_digests)` built from file contents (`parse_sources`) | Yes — reads real markdown files from repo root | ✓ FLOWING |
| `service.rs` | `canonical_requirement_ids` | `snapshot.items` from parser + snapshot builder | Yes — derived from parsed artifact content | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| --- | --- | --- | --- |
| Phase ingestion unit/integration behavior | `$HOME/.cargo/bin/cargo test -p research-gateway ingestion::` | 14 passed, 0 failed | ✓ PASS |
| Phase API/E2E user behaviors | `node --test tests/api/phase-1-ingestion.test.mjs tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` | 4 passed, 0 failed | ✓ PASS |
| Immutability regression guard | `$HOME/.cargo/bin/cargo test -p persistence postgres::canonical_artifacts::tests::snapshot_metadata_immutable_` + `...migration_contract_snapshot_immutability_` | 4 passed, 0 failed | ✓ PASS |
| Snapshot identity timestamp differentiation | `$HOME/.cargo/bin/cargo test -p research-gateway ingestion::service::tests::snapshot_identity_` | 1 passed, 0 failed | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| --- | --- | --- | --- | --- |
| ARTF-01 | 01-02, 01-03 | Ingest PRD artifacts into canonical dataset | ✓ SATISFIED | Discovery classification + API test asserts PRD count=1. |
| ARTF-02 | 01-02, 01-03 | Ingest architecture artifacts into canonical dataset | ✓ SATISFIED | Discovery classification + API test asserts architecture count=1. |
| ARTF-03 | 01-02, 01-03 | Ingest story artifacts into canonical dataset | ✓ SATISFIED | Discovery classification + API test asserts story count=1. |
| ARTF-04 | 01-02, 01-03 | Ingest roadmap artifacts into canonical dataset | ✓ SATISFIED | Discovery classification + API test asserts roadmap count=1. |
| ARTF-05 | 01-01, 01-03, 01-04 | Stable IDs + snapshot version metadata | ✓ SATISFIED | Deterministic ID logic, stable replay tests, insert-only + DB immutability guard. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| --- | --- | --- | --- | --- |
| `services/research-gateway/src/ingestion/artifact_discovery.rs` | 91 | Broad substring match `contains("arch")` | ⚠️ Warning | Could misclassify unrelated paths containing `arch`; does not block current phase truths/tests. |

### Human Verification Required

None.

### Gaps Summary

No remaining gaps. The previous immutability gap is closed by both application behavior (insert-only snapshot persistence) and schema guard (trigger-based metadata immutability), with regression tests passing.

---

_Verified: 2026-04-09T08:22:00Z_  
_Verifier: the agent (gsd-verifier)_
