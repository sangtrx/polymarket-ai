# Phase 1: Canonical Artifact Ingestion - Research

**Researched:** 2026-04-09  
**Domain:** Canonical BMAD artifact ingestion + normalization + snapshot persistence in existing Rust service architecture [VERIFIED: .planning/ROADMAP.md; .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md]  
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
### ID schema and normalization
- **D-01:** Use hierarchical IDs in `artifact.section.item` form for canonical requirement identity.
- **D-02:** Preserve source-specific IDs per artifact and link semantically equivalent items through explicit equivalence relationships (no forced flattening).
- **D-03:** Keep IDs stable using `artifact path + heading slug + item index` anchoring.
- **D-04:** Validation policy is fail-on-structural/schema-errors and warn-on-minor-quality-issues.

### Artifact source boundaries
- **D-05:** Ingest only BMAD intent sources for this phase: PRD, architecture, stories, and roadmap documents.
- **D-06:** Prioritize canonical source locations under `.planning/`, `docs/`, and BMAD-managed docs directories.
- **D-07:** Include requirement-bearing structures (headings, checklists, acceptance criteria tables); exclude operational chatter and generated telemetry output.

### Snapshot and versioning
- **D-08:** Create immutable ingestion snapshots keyed by commit SHA + ingestion timestamp.
- **D-09:** Store per-file content digests and an aggregate snapshot digest to support deterministic re-runs.
- **D-10:** Use full-snapshot ingestion in v1 (no delta-only ingestion in this phase).

### Conflict handling and validation behavior
- **D-11:** Record semantic conflicts as explicit unresolved conflict records; do not auto-merge conflicting intent.
- **D-12:** Keep ingestion successful when schema is valid, but surface conflicts as high-visibility findings for downstream phases.

### the agent's Discretion
- User explicitly allowed recommended defaults ("just do your best, yolo") for remaining gray areas after ID-schema discussion.
- Planner/researcher may choose implementation details for parser internals, storage schema naming, and helper structure as long as D-01..D-12 remain intact.

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ARTF-01 | User can ingest PRD artifacts into a canonical audit dataset [VERIFIED: .planning/REQUIREMENTS.md] | Source-scoped discovery + parsing pattern + canonical persistence model [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md; crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql] |
| ARTF-02 | User can ingest architecture artifacts into a canonical audit dataset [VERIFIED: .planning/REQUIREMENTS.md] | Same ingestion contract, separate artifact type channel + provenance IDs [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md] |
| ARTF-03 | User can ingest story artifacts into a canonical audit dataset [VERIFIED: .planning/REQUIREMENTS.md] | Requirement-bearing extraction strategy (headings/checklists/tables) [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md] |
| ARTF-04 | User can ingest roadmap artifacts into a canonical audit dataset [VERIFIED: .planning/REQUIREMENTS.md] | Roadmap parser in same canonical schema + per-item provenance [VERIFIED: .planning/ROADMAP.md; .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md] |
| ARTF-05 | User can normalize all ingested BMAD items to stable requirement IDs with snapshot version metadata [VERIFIED: .planning/REQUIREMENTS.md] | Deterministic ID composition, immutable snapshot keying, file/aggregate digests [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md] |
</phase_requirements>

## Project Constraints (from copilot-instructions.md)

- Scope remains audit/control-plane; avoid coupling ingestion work to runtime trading loops [VERIFIED: copilot-instructions.md; .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
- Maintain full BMAD baseline coverage (PRD, architecture, stories, roadmap) rather than partial ingestion [VERIFIED: copilot-instructions.md; .planning/REQUIREMENTS.md].  
- Keep machine-readable, typed error surfaces and explicit reason codes (no implicit success fallbacks) [VERIFIED: copilot-instructions.md; services/research-gateway/src/validation/workflow_runs.rs].  
- Follow existing layered architecture: domain contracts → persistence adapters → service orchestration; persistence changes are migration-first [VERIFIED: copilot-instructions.md; .planning/codebase/ARCHITECTURE.md; .planning/codebase/STRUCTURE.md].  
- Respect existing Rust conventions (snake_case modules, typed `Result<T,E>`, clippy warnings as errors) [VERIFIED: copilot-instructions.md; package.json].

## Summary

Phase 1 should be planned as a Rust-first ingestion subsystem in the existing domain/persistence/service layering, not as an ad-hoc script path [VERIFIED: .planning/codebase/ARCHITECTURE.md; .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md]. Reuse established contract patterns already present in `domain::research`, `persistence::postgres::*`, and `research-gateway` orchestration to keep identifiers canonicalized, timestamps UTC RFC3339, and persistence deterministic [VERIFIED: crates/domain/src/research.rs; crates/persistence/src/postgres/validation_runs.rs; services/research-gateway/src/validation/workflow_runs.rs].

The highest-value planning decision is to model ingestion outputs as immutable snapshot records keyed by commit SHA + ingestion timestamp with per-file and aggregate digests, while preserving source-specific IDs and explicit equivalence/conflict records [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md]. This directly satisfies D-01..D-12 and avoids downstream remapping churn in phases 2-5 [VERIFIED: .planning/ROADMAP.md; .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].

**Primary recommendation:** Implement Phase 1 as a new canonical-ingestion module in `services/research-gateway` backed by new migration-managed tables and typed domain contracts, using deterministic ID/snapshot rules from CONTEXT as non-negotiable invariants [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md; .planning/codebase/STRUCTURE.md].

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8.6 (workspace pin), crates.io latest 0.8.6, updated 2025-10-15 [VERIFIED: Cargo.toml; VERIFIED: crates.io API https://crates.io/api/v1/crates/sqlx] | Persist immutable ingestion snapshots and query deterministic audit baselines | Already standard in repo persistence layer and migration flow [VERIFIED: Cargo.toml; crates/persistence/src/postgres/validation_runs.rs] |
| tokio | 1.48.0 (workspace pin) [VERIFIED: Cargo.toml] | Async orchestration for ingestion run workflows | Already service runtime standard in repo [VERIFIED: Cargo.toml; services/research-gateway/Cargo.toml] |
| serde + serde_json | 1.0.228 / 1.0.145 [VERIFIED: Cargo.toml] | Typed canonical records + structured diagnostics payloads | Existing domain/service contract pattern uses these consistently [VERIFIED: crates/domain/src/research.rs; services/research-gateway/src/validation/workflow_runs.rs] |
| sha2 | 0.10.9 (workspace pin), crates.io latest 0.11.0, updated 2026-03-25 [VERIFIED: Cargo.toml; VERIFIED: crates.io API https://crates.io/api/v1/crates/sha2] | Per-file and aggregate digest generation for snapshot determinism | Already available in workspace and directly matches D-09 digest requirement [VERIFIED: Cargo.toml; .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| walkdir | crates.io latest 2.5.0, updated 2024-03-01 [VERIFIED: crates.io API https://crates.io/api/v1/crates/walkdir; Cargo.lock] | Controlled recursive discovery under `.planning/`, `docs/`, BMAD-managed dirs | Use for D-06 scoped file enumeration [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md] |
| pulldown-cmark | crates.io latest 0.13.3, updated 2026-03-22 [VERIFIED: crates.io API https://crates.io/api/v1/crates/pulldown-cmark] | CommonMark parser for heading/list extraction | Use if parser implementation is done in Rust service layer [CITED: https://crates.io/crates/pulldown-cmark] |
| gray_matter | crates.io latest 0.3.2, updated 2025-07-10 [VERIFIED: crates.io API https://crates.io/api/v1/crates/gray_matter] | Frontmatter extraction if needed beyond markdown body parsing | Use only where BMAD files rely on frontmatter metadata [CITED: https://crates.io/crates/gray_matter] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| pulldown-cmark | comrak 0.52.0 [VERIFIED: crates.io API https://crates.io/api/v1/crates/comrak] | comrak offers full GFM compatibility but adds heavier parser/formatter surface for v1 scope [CITED: https://crates.io/crates/comrak] |
| Rust parser path | Node frontmatter utility in `.github/get-shit-done/bin/lib/frontmatter.cjs` [VERIFIED: .github/get-shit-done/bin/lib/frontmatter.cjs] | Fast reuse for docs tooling, but planning should prefer runtime service consistency in Rust layer [ASSUMED] |

**Installation:**
```bash
cargo add pulldown-cmark gray_matter --package research-gateway
```

**Version verification:**  
```bash
python3 - <<'PY'
import json,urllib.request
for c in ['pulldown-cmark','gray_matter','sqlx','sha2','walkdir']:
  d=json.load(urllib.request.urlopen(f'https://crates.io/api/v1/crates/{c}',timeout=10))
  print(c, d['crate'].get('max_stable_version') or d['crate'].get('max_version'), d['crate']['updated_at'])
PY
```

## Architecture Patterns

### Recommended Project Structure
```text
crates/
├── domain/src/audit_artifacts.rs              # Canonical artifact/item/snapshot contracts + validators
├── persistence/src/postgres/canonical_artifacts.rs  # SQL adapters
└── persistence/migrations/20*_canonical_artifact_ingestion.sql
services/
└── research-gateway/src/ingestion/
    ├── mod.rs                                 # Orchestrator + ports
    ├── artifact_parser.rs                     # Markdown/frontmatter extraction
    └── snapshot_builder.rs                    # Digest + immutable snapshot assembly
```
[VERIFIED: .planning/codebase/STRUCTURE.md; .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md]

### Pattern 1: Canonicalize IDs at contract boundary
**What:** Normalize IDs before persistence; enforce non-empty and stable canonical format [VERIFIED: crates/domain/src/research.rs].  
**When to use:** On every parsed artifact item before storage [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**Example:**
```rust
// Source: crates/domain/src/research.rs
pub fn normalize_research_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}
```

### Pattern 2: Fail on contract/schema errors, keep typed reason codes
**What:** Return machine-readable code/message/field_errors from service layer [VERIFIED: services/research-gateway/src/validation/workflow_runs.rs].  
**When to use:** Parser schema violations, invalid timestamps, malformed IDs [VERIFIED: crates/domain/src/research.rs].  
**Example:**
```rust
// Source: services/research-gateway/src/validation/workflow_runs.rs
pub struct ValidationWorkflowServiceError {
    pub code: &'static str,
    pub message: String,
    pub field_errors: Vec<ValidationWorkflowValidationIssue>,
}
```

### Pattern 3: Deterministic ordering and immutable evidence records
**What:** Persist run + artifact records with deterministic sort/order logic and immutable run key [VERIFIED: services/research-gateway/src/validation/workflow_runs.rs; crates/persistence/src/postgres/validation_runs.rs].  
**When to use:** Snapshot assembly and replay-safe listing [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**Example:**
```rust
// Source: crates/domain/src/research.rs
pub fn compose_validation_run_id(candidate_id: &str, started_at_utc: &str) -> Result<String, ValidationWorkflowContractError> {
    let normalized_candidate_id = normalize_research_identifier(candidate_id);
    let started_at = parse_validation_utc_timestamp(started_at_utc)?;
    Ok(format!("{}::{}", normalized_candidate_id, started_at.unix_timestamp_nanos()))
}
```

### Anti-Patterns to Avoid
- **Hand-built ad-hoc JSON blobs without domain structs:** breaks typed validation and downstream determinism [VERIFIED: crates/domain/src/research.rs; services/research-gateway/src/validation/workflow_runs.rs].  
- **Auto-merging semantic conflicts:** violates D-11; must persist unresolved conflict records [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
- **Delta-only ingestion in v1:** violates D-10 and undermines reproducible baseline [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Markdown parsing | Custom regex parser for headings/lists/tables | `pulldown-cmark`/`comrak` parser crates [CITED: https://crates.io/crates/pulldown-cmark; https://crates.io/crates/comrak] | Markdown edge cases are non-trivial and easy to misparse [ASSUMED] |
| Digesting | Custom hash logic | `sha2` crate [CITED: https://crates.io/crates/sha2] | Security-sensitive + correctness-critical primitive [CITED: https://crates.io/crates/sha2] |
| Persistence ordering/constraints | Manual in-memory ordering assumptions | SQL constraints + indexes + explicit ORDER BY patterns [VERIFIED: crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql; crates/persistence/src/postgres/validation_runs.rs] | Deterministic reads are already codified as repo standard |
| ID normalization | Per-call ad hoc string transforms | single canonical normalize function in domain contract [VERIFIED: crates/domain/src/research.rs] | Prevents drift between parser/service/persistence layers |

**Key insight:** Keep complexity in proven crates and typed contracts; Phase 1 risk is inconsistency, not missing algorithm novelty [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md; .planning/research/SUMMARY.md].

## Common Pitfalls

### Pitfall 1: Non-deterministic IDs across reruns
**What goes wrong:** Same requirement receives different ID when file order/headings shift unexpectedly [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**Why it happens:** IDs not anchored to path + heading slug + item index (D-03) [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**How to avoid:** Lock ID derivation function in domain + test fixtures for rerun stability [VERIFIED: crates/domain/src/research.rs; services/research-gateway/src/validation/workflow_runs.rs].  
**Warning signs:** Snapshot digest changes without source file content changes [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].

### Pitfall 2: Silent acceptance of malformed structure
**What goes wrong:** Ingestion appears successful but unusable items leak into canonical dataset [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**Why it happens:** Parser only warns for schema/structural errors instead of failing [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**How to avoid:** Enforce fail-on-structural/schema and warn-only for minor quality issues (D-04) [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**Warning signs:** Missing required fields but run still marked completed [ASSUMED].

### Pitfall 3: Full-snapshot invariants eroded by premature delta logic
**What goes wrong:** Baseline can’t be reproduced from a commit/timestamp pair [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**Why it happens:** Early optimization into delta-only ingestion [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
**How to avoid:** Keep v1 full snapshot only; defer incremental strategy to later phases [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md; .planning/ROADMAP.md].  
**Warning signs:** Missing unchanged files in canonical snapshot outputs [ASSUMED].

## Code Examples

Verified patterns from repository sources:

### Deterministic run identity composition
```rust
// Source: crates/domain/src/research.rs
Ok(format!(
    "{}::{}",
    normalized_candidate_id,
    started_at.unix_timestamp_nanos()
))
```

### Canonical SQL ordering for deterministic listing
```sql
-- Source: crates/persistence/src/postgres/validation_runs.rs
ORDER BY started_at_utc DESC, run_id ASC
```

### Typed persistence error classification
```rust
// Source: crates/persistence/src/postgres/validation_runs.rs
fn classify_query_error(operation: &'static str, error: sqlx::Error) -> ValidationRunPersistenceError {
    if is_constraint_error(&error) {
        return ValidationRunPersistenceError::constraint_violation(operation, error);
    }
    ValidationRunPersistenceError::query_failure(operation, error)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Implicit IDs or display labels as keys [ASSUMED] | Canonicalized lowercase identifiers + deterministic composed IDs [VERIFIED: crates/domain/src/research.rs] | Present in current codebase (2026 domain contracts) [VERIFIED: crates/domain/src/research.rs] | Safer joins and replay stability |
| Flat persistence without strong constraints [ASSUMED] | SQL CHECK constraints + canonical indexes + deterministic ORDER BY [VERIFIED: crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql; crates/persistence/src/postgres/validation_runs.rs] | Present in current migration set [VERIFIED: crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql] | Lower drift/ambiguity under reruns |
| Best-effort validation on ingestion [ASSUMED] | Fail-closed validation in service/domain contracts with reason codes [VERIFIED: services/research-gateway/src/validation/workflow_runs.rs; crates/domain/src/research.rs] | Present in current service patterns [VERIFIED: services/research-gateway/src/validation/workflow_runs.rs] | Predictable operator behavior and better debugging |

**Deprecated/outdated:**
- Delta-only ingestion for phase baseline is out of scope for v1 [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].  
- Auto-merging conflicting intent is explicitly disallowed in Phase 1 [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Rust-service parser path is preferable to Node utility reuse for long-term consistency | Standard Stack / Alternatives | Could overcomplicate if team intends CLI-first ingestion path |
| A2 | Regex-based markdown parsing would be too brittle for BMAD structures | Don’t Hand-Roll | Might introduce unnecessary dependency if structures are highly constrained |
| A3 | Missing required fields would be the main early warning sign in failed ingestion | Common Pitfalls | Detection heuristics may need adjustment |

## Open Questions (RESOLVED)

1. **Canonical storage shape for equivalence and conflict records**
   - Resolution: Schema boundaries are locked to four tables: `canonical_ingestion_snapshots`, `canonical_artifact_items`, `canonical_item_equivalences`, and `canonical_item_conflicts` [RESOLVED: .planning/phases/01-canonical-artifact-ingestion/01-01-PLAN.md].
   - Enforcement: Migration and persistence adapter tests in Plan 01 are the source of truth for these boundaries [RESOLVED: .planning/phases/01-canonical-artifact-ingestion/01-01-PLAN.md].

2. **Parser implementation boundary**
   - Resolution: v1 ingestion logic is owned by `services/research-gateway` runtime modules; CLI entrypoint in Plan 03 invokes the same service orchestration and does not introduce a separate Node parser path [RESOLVED: .planning/phases/01-canonical-artifact-ingestion/01-03-PLAN.md].
   - Enforcement: No parallel parser implementation in GSD Node tooling is allowed for Phase 1 scope [RESOLVED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md].

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | Existing scripts + optional tooling glue [VERIFIED: package.json] | ✓ [VERIFIED: local command output] | v24.14.1 [VERIFIED: local command output] | — |
| pnpm | Workspace scripts [VERIFIED: package.json] | ✓ [VERIFIED: local command output] | 10.10.0 [VERIFIED: local command output] | npm scripts partially usable |
| Rust/Cargo | Core implementation + tests [VERIFIED: Cargo.toml; package.json] | ✗ [VERIFIED: local command output] | — | Blocking for execution (no viable fallback for Rust compile/test) |
| PostgreSQL CLI (`psql`) | Local DB inspection/migration validation [ASSUMED] | ✗ [VERIFIED: local command output] | — | Use sqlx test harness/CI DB if configured [ASSUMED] |
| Docker | Optional local Postgres container [ASSUMED] | ✓ [VERIFIED: local command output] | 29.1.3 [VERIFIED: local command output] | Could run Postgres container if image pull allowed [ASSUMED] |

**Missing dependencies with no fallback:**
- Cargo/rustc are missing locally; Phase 1 implementation/testing is blocked until Rust toolchain is installed [VERIFIED: local command output; rust-toolchain.toml].

**Missing dependencies with fallback:**
- `psql` missing; can use CI-backed DB or Dockerized DB workflows if planner includes setup steps [ASSUMED].

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` + Node `node --test` harness [VERIFIED: package.json; tests/*.test.mjs] |
| Config file | none explicit (command-driven) [VERIFIED: package.json; repository scan] |
| Quick run command | `cargo test -p research-gateway ingestion::tests:: -x` [ASSUMED] |
| Full suite command | `npm run rust:test && npm run test` [VERIFIED: package.json] |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ARTF-01 | PRD ingestion into canonical dataset | integration | `cargo test -p research-gateway ingestion::tests::ingest_prd_ -x` | ❌ Wave 0 |
| ARTF-02 | Architecture ingestion into canonical dataset | integration | `cargo test -p research-gateway ingestion::tests::ingest_architecture_ -x` | ❌ Wave 0 |
| ARTF-03 | Story ingestion into canonical dataset | integration | `cargo test -p research-gateway ingestion::tests::ingest_story_ -x` | ❌ Wave 0 |
| ARTF-04 | Roadmap ingestion into canonical dataset | integration | `cargo test -p research-gateway ingestion::tests::ingest_roadmap_ -x` | ❌ Wave 0 |
| ARTF-05 | Stable IDs + snapshot version metadata | unit + integration | `cargo test -p domain research::tests::canonical_artifact_id_ -x && cargo test -p persistence postgres::canonical_artifacts::tests:: -x` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** targeted cargo test for touched ingestion module(s) [ASSUMED]  
- **Per wave merge:** `npm run rust:test` [VERIFIED: package.json]  
- **Phase gate:** full Rust + relevant Node tests green before `/gsd-verify-work` [VERIFIED: package.json; workflow.nyquist_validation=true in .planning/config.json]

### Wave 0 Gaps
- [ ] `services/research-gateway/src/ingestion/mod.rs` tests for ARTF-01..ARTF-04 — currently missing [VERIFIED: services/research-gateway/src].  
- [ ] `crates/domain/src/audit_artifacts.rs` contract tests for canonical ID and snapshot invariants (ARTF-05) — file not present [VERIFIED: crates/domain/src].  
- [ ] `crates/persistence/src/postgres/canonical_artifacts.rs` adapter tests + migration assertions — file not present [VERIFIED: crates/persistence/src/postgres; crates/persistence/migrations].  
- [ ] Rust toolchain install prerequisite before any automated phase tests can run locally [VERIFIED: local command output; rust-toolchain.toml].

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (if ingestion is internal batch) [ASSUMED] | Reuse existing control-api auth middleware only if exposed via API [VERIFIED: .planning/codebase/ARCHITECTURE.md] |
| V3 Session Management | no (non-session ingestion workflow) [ASSUMED] | N/A |
| V4 Access Control | yes [ASSUMED] | Role checks pattern as used in `ValidationWorkflowRunService` [VERIFIED: services/research-gateway/src/validation/workflow_runs.rs] |
| V5 Input Validation | yes | Domain-level validation + typed contract errors [VERIFIED: crates/domain/src/research.rs; services/research-gateway/src/validation/workflow_runs.rs] |
| V6 Cryptography | yes | `sha2` crate for digests; never custom hash implementation [VERIFIED: Cargo.toml; CITED: https://crates.io/crates/sha2] |

### Known Threat Patterns for Rust ingestion + SQL persistence

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed markdown/frontmatter leading to corrupt canonical data | Tampering | Fail-on-structural/schema validation + typed errors [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md; services/research-gateway/src/validation/workflow_runs.rs] |
| Digest mismatch or replay ambiguity | Repudiation | Per-file + aggregate digests and immutable snapshot identifiers [VERIFIED: .planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md] |
| Unsafe DB writes / inconsistent IDs | Tampering | Canonical normalization + DB CHECK constraints + indexed lookup [VERIFIED: crates/domain/src/research.rs; crates/persistence/migrations/20260407193000_validation_runs_validation_artifacts.sql] |
| Overly permissive ingestion mutation access | Elevation of Privilege | Reuse role-gated orchestrator checks [VERIFIED: services/research-gateway/src/validation/workflow_runs.rs] |

## Sources

### Primary (HIGH confidence)
- `.planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md` — locked decisions D-01..D-12 and scope.  
- `.planning/REQUIREMENTS.md` / `.planning/ROADMAP.md` / `.planning/STATE.md` — requirements and phase success criteria.  
- `copilot-instructions.md` — project constraints, architecture/convention enforcement.  
- `Cargo.toml`, `package.json`, `rust-toolchain.toml` — pinned stack/runtime/test commands.  
- `crates/domain/src/research.rs` — canonicalization, typed contracts, deterministic run-ID composition.  
- `services/research-gateway/src/validation/workflow_runs.rs` — orchestrator/error/ordering patterns.  
- `crates/persistence/src/postgres/validation_runs.rs` + migration `20260407193000_validation_runs_validation_artifacts.sql` — persistence constraints/index/order patterns.

### Secondary (MEDIUM confidence)
- Crates registry API (official):  
  - https://crates.io/api/v1/crates/pulldown-cmark  
  - https://crates.io/api/v1/crates/comrak  
  - https://crates.io/api/v1/crates/gray_matter  
  - https://crates.io/api/v1/crates/sha2  
  - https://crates.io/api/v1/crates/walkdir

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - mostly repository-pinned versions + official crates registry verification.  
- Architecture: HIGH - derived from existing in-repo architecture and service/persistence patterns.  
- Pitfalls: MEDIUM - grounded in locked decisions and known repo patterns; a few detection heuristics remain assumed.

**Research date:** 2026-04-09  
**Valid until:** 2026-05-09 (30 days, assuming no major stack shifts)
