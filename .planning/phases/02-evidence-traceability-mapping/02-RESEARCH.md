# Phase 02: evidence-traceability-mapping - Research

**Researched:** 2026-04-09  
**Domain:** Canonical requirement → code/test evidence mapping contracts in Rust + Postgres + Node test harness [VERIFIED: .planning/ROADMAP.md, Cargo.toml, package.json]  
**Confidence:** HIGH

## User Constraints (from CONTEXT.md)

### Locked Decisions
### Evidence source policy
- **D-01:** Code evidence must be direct implementation artifacts only (module/function implementing behavior); include schema/config files only when they are behavior-critical.
- **D-02:** Test evidence requires at least one automated test with an explicit assertion tied to the mapped requirement.
- **D-03:** Each requirement mapping needs a minimum of one high-quality code reference; additional references are optional.
- **D-04:** Every evidence link must include file path + symbol/section/line anchor + concise rationale.

### Linking strategy (recommended defaults applied)
- **D-05:** Use deterministic linking first (canonical requirement ID to explicit code/test anchors). Allow semantic fallback only when direct ID linkage is unavailable.
- **D-06:** When semantic fallback is used, emit lower confidence with machine-readable reason code and keep provenance explicit.
- **D-07:** Support many-to-many mappings (one requirement to multiple code/test references) without forced collapse.

### Confidence and rationale model (recommended defaults applied)
- **D-08:** Use a three-level confidence scale (`high`, `medium`, `low`) with deterministic scoring rules.
- **D-09:** Rationale is mandatory for every link; empty rationale is invalid.

### Ambiguity handling (recommended defaults applied)
- **D-10:** If multiple plausible evidence candidates exist, retain all candidates with explicit confidence ranking (no silent winner selection).
- **D-11:** If no qualifying evidence exists, record explicit "missing evidence" outcome for that requirement.
- **D-12:** If previously linked evidence is stale at current commit (path/symbol mismatch), mark it invalid and preserve audit history.

### the agent's Discretion
- User directed: "do your best, yolo" and approved applying recommended defaults for unresolved gray areas.
- Planner/researcher can choose concrete schema/module names and matching heuristics as long as D-01..D-12 are preserved.

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TRAC-01 | User can map each canonical BMAD requirement ID to one or more code evidence references [VERIFIED: .planning/REQUIREMENTS.md] | Deterministic-first + many-to-many link schema + stale-link invalidation pattern |
| TRAC-02 | User can map each canonical BMAD requirement ID to related test evidence when available [VERIFIED: .planning/REQUIREMENTS.md] | Separate evidence_type (`code`/`test`) with required automated-assertion linkage metadata |
| TRAC-03 | User can record rationale and confidence for every traceability link [VERIFIED: .planning/REQUIREMENTS.md] | Non-empty rationale constraints + confidence enum + reason codes |
</phase_requirements>

## Project Constraints (from copilot-instructions.md)

- Keep scope on audit workflow implementation; do not include production deployment actions. [VERIFIED: copilot-instructions.md:12]
- Preserve file-level traceability evidence for BMAD items. [VERIFIED: copilot-instructions.md:13]
- Keep baseline across PRD + architecture + stories + roadmap. [VERIFIED: copilot-instructions.md:14]
- Keep outputs decision-useful for deployment impact. [VERIFIED: copilot-instructions.md:15]
- Use existing monorepo conventions: Rust `snake_case`, Node `*.test.mjs`/`*.e2e.test.mjs`, machine-readable error payloads. [VERIFIED: copilot-instructions.md:69-93]

## Summary

Phase 2 should be implemented as a new traceability module in `services/research-gateway` backed by a Postgres adapter in `crates/persistence/src/postgres/`, following the exact Phase 1 pattern: domain contract validation, deterministic assembly, persistence transaction, and machine-readable failures. [VERIFIED: services/research-gateway/src/ingestion/service.rs, crates/persistence/src/postgres/canonical_artifacts.rs, .planning/phases/02-evidence-traceability-mapping/02-CONTEXT.md]

The best planning baseline is: deterministic link pass first (explicit canonical ID anchors in code/tests), then semantic fallback with explicit low/medium confidence + reason code, never silent winner selection, and explicit missing-evidence records. [VERIFIED: .planning/phases/02-evidence-traceability-mapping/02-CONTEXT.md]

Primary risk is not algorithmic complexity but contract drift: missing rationale, unstable ordering, or ambiguous links being collapsed. Prevent with DB constraints, deterministic ordering, and tests mirroring Phase 1 API/E2E style. [VERIFIED: tests/api/phase-1-ingestion.test.mjs, tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs, crates/persistence/migrations/20260409000100_canonical_artifact_ingestion.sql]

**Primary recommendation:** Build `traceability` as deterministic contract-first infrastructure (schema + orchestrator + tests) before improving semantic quality heuristics. [VERIFIED: .planning/phases/02-evidence-traceability-mapping/02-CONTEXT.md]

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust toolchain | 1.91.0 (pinned) | Traceability contracts/services | Existing repo baseline and CI target. [VERIFIED: rust-toolchain.toml, copilot-instructions.md:22,28] |
| `sqlx` | 0.8.6 (workspace pin) | Postgres persistence adapter | Existing persistence path uses bind parameters + typed error mapping. [VERIFIED: Cargo.toml:37, crates/persistence/src/postgres/canonical_artifacts.rs] |
| `serde` + `serde_json` | 1.0.228 / 1.0.145 (workspace pins) | Machine-readable mapping payloads/errors | Existing domain + service DTO serialization pattern. [VERIFIED: Cargo.toml:34-35, services/research-gateway/src/main.rs] |
| `tokio` | 1.48.0 (workspace pin) | Async orchestration + transactional persistence calls | Existing service runtime baseline. [VERIFIED: Cargo.toml:39, services/research-gateway/Cargo.toml] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Node built-in test runner | Node runtime built-in | API/E2E contract tests for CLI outputs | Keep phase behavior tests in `tests/api` and `tests/e2e`. [VERIFIED: package.json:15,55; tests/api/phase-1-ingestion.test.mjs] |
| `sha2` | 0.10.9 (workspace pin) | Stable digesting for deterministic snapshots/ids | Reuse when mapping snapshots require deterministic hashes. [VERIFIED: Cargo.toml:36, services/research-gateway/src/ingestion/service.rs] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Deterministic-first with explicit fallback | Semantic-only mapping | Violates locked D-05/D-06 and reduces audit defensibility. [VERIFIED: 02-CONTEXT.md:23-25] |
| Keep all ambiguous candidates ranked | Force single top match | Violates locked D-10 and hides uncertainty. [VERIFIED: 02-CONTEXT.md:31-33] |

**Installation (if missing locally):**
```bash
rustup toolchain install 1.91.0
rustup component add clippy rustfmt
```

## Architecture Patterns

### Recommended Project Structure
```text
crates/
├── domain/src/traceability.rs                 # Trace link contracts + validation
├── persistence/src/postgres/traceability.rs   # SQL adapter + error code mapping
services/research-gateway/src/traceability/
├── mod.rs
├── service.rs                                 # Deterministic linking pipeline
└── matcher.rs                                 # Deterministic + semantic fallback logic
tests/
├── api/phase-2-traceability.test.mjs
└── e2e/phase-2-traceability-mapping.e2e.test.mjs
```

### Pattern 1: Port + Adapter + Transactional persistence
**What:** Keep orchestration behind trait ports and implement a Postgres adapter with transaction commit/rollback behavior. [VERIFIED: services/research-gateway/src/ingestion/service.rs:63-170]  
**When to use:** Any new traceability write path requiring atomic snapshot + links.  
**Example:**
```rust
// Source: services/research-gateway/src/ingestion/service.rs
pub trait CanonicalSnapshotPersistencePort: Send + Sync {
    fn persist_snapshot<'a>(&'a self, snapshot: &'a CanonicalSnapshot, assembly: &'a SnapshotAssembly)
        -> Pin<Box<dyn Future<Output = Result<(), CanonicalIngestionError>> + Send + 'a>>;
}
```

### Pattern 2: Deterministic ordering for reproducibility
**What:** Use sorted data structures and explicit SQL ordering to stabilize outputs between runs. [VERIFIED: services/research-gateway/src/ingestion/service.rs:273-285,366-375; crates/persistence/src/postgres/canonical_artifacts.rs:58-64]  
**When to use:** Canonical requirement → evidence matrix generation for CI comparability.

### Pattern 3: Fail-closed machine-readable errors
**What:** Return code+message payloads and explicit reason codes instead of best-effort success-shaped fallbacks. [VERIFIED: services/research-gateway/src/main.rs:7-90; crates/persistence/src/postgres/canonical_artifacts.rs:66-93]  
**When to use:** Invalid mapping inputs, stale anchors, or persistence constraints.

### Anti-Patterns to Avoid
- **Silent ambiguity collapse:** choosing one candidate when many plausible links exist breaks D-10. [VERIFIED: 02-CONTEXT.md:32]
- **Empty rationale acceptance:** violates D-09 and undermines traceability defensibility. [VERIFIED: 02-CONTEXT.md:28-30]
- **Naive substring artifact classification reuse:** Phase 1 already flags over-broad `contains("arch")` risk. [VERIFIED: 01-VERIFICATION.md:85-90, snapshot_builder.rs:162]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SQL safety | String-concatenated SQL | `sqlx::query(...).bind(...)` pattern | Existing adapters already enforce this safe/typed pattern. [VERIFIED: crates/persistence/src/postgres/canonical_artifacts.rs:124-165] |
| Error taxonomy | Free-form text errors | Typed error structs with stable `code` fields | Existing API/test contracts assert reason codes. [VERIFIED: services/research-gateway/src/main.rs:7-90; tests/api/phase-1-ingestion.test.mjs:75-90] |
| Determinism controls | Hash-map/random order outputs | `BTreeMap` + explicit sort + SQL `ORDER BY` | Needed for replay-stable audit evidence. [VERIFIED: services/research-gateway/src/ingestion/service.rs:14,273-285,366-375; canonical_artifacts.rs:58-64] |

**Key insight:** In this codebase, correctness is primarily contract discipline (stable IDs, stable ordering, explicit error/rationale metadata), not heuristic cleverness. [VERIFIED: 02-CONTEXT.md, 01-VERIFICATION.md]

## Common Pitfalls

### Pitfall 1: Treating semantic suggestions as authoritative
**What goes wrong:** low-quality semantic candidate becomes final link without explicit downgrade.  
**Why it happens:** fallback path does not emit reason code + confidence.  
**How to avoid:** enforce D-06 (`semantic_fallback_used`) reason code + confidence ≤ medium/low. [VERIFIED: 02-CONTEXT.md:24]  
**Warning signs:** links lack deterministic anchor provenance fields.

### Pitfall 2: Missing evidence not represented
**What goes wrong:** requirements disappear from outputs when no links found.  
**Why it happens:** pipelines only emit positive links.  
**How to avoid:** emit explicit `missing evidence` outcome per requirement (D-11). [VERIFIED: 02-CONTEXT.md:33]  
**Warning signs:** output row count < canonical requirement count.

### Pitfall 3: Stale anchor drift after refactors
**What goes wrong:** stored path/symbol/line no longer resolves but remains “valid”.  
**Why it happens:** no revalidation at current commit.  
**How to avoid:** enforce D-12 stale invalidation + audit history retention. [VERIFIED: 02-CONTEXT.md:34]  
**Warning signs:** link points to missing file/line but confidence still high.

## Code Examples

### Machine-readable fail-closed CLI payload
```rust
// Source: services/research-gateway/src/main.rs
#[derive(Debug, Serialize, PartialEq, Eq)]
struct CliErrorPayload {
    code: String,
    message: String,
}
```

### Deterministic canonical ID contract
```rust
// Source: crates/domain/src/audit_artifacts.rs
pub fn canonical_requirement_id(artifact_path: &str, heading_slug: &str, item_index: u32)
    -> Result<String, CanonicalArtifactContractError> {
    // validates required fields, normalizes tokens, returns deterministic id
}
```

### SQLx bind-based persistence
```rust
// Source: crates/persistence/src/postgres/canonical_artifacts.rs
sqlx::query(UPSERT_ITEM_SQL)
    .bind(&item.canonical_requirement_id)
    .bind(snapshot_id)
    .bind(item.artifact_type.as_str())
    .execute(executor)
    .await?;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Implicit/loose requirement references | Deterministic canonical IDs from path+heading+index | Phase 1 completion (2026-04-09) [VERIFIED: 01-VERIFICATION.md] | Enables stable keying for traceability rows |
| Mutable snapshot metadata | Insert-only snapshots + DB immutability trigger | 20260409000200 migration [VERIFIED: 01-VERIFICATION.md, crates/persistence/migrations/20260409000200_canonical_snapshot_immutability.sql] | Prevents evidence tampering/drift |
| Single-winner ambiguity handling | Retain multiple ranked candidates + explicit missing outcomes | Locked for Phase 2 [VERIFIED: 02-CONTEXT.md:32-34] | Preserves audit transparency |

**Deprecated/outdated:**
- Silent winner selection for ambiguous evidence links. [VERIFIED: 02-CONTEXT.md:32]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Line anchor (`path + line`) can satisfy D-04 when symbol extraction is not available in a file type. [ASSUMED] | Architecture Patterns | Could require additional parser work for strict symbol extraction |

## Open Questions (RESOLVED)

1. **Canonical anchor schema detail — RESOLVED**
   - Decision: use structured anchor payload with deterministic fields: `{path, symbol?, section?, line_start?, line_end?}` plus required `rationale`.
   - Rationale: satisfies D-04 while allowing language/file-type variability without losing deterministic anchors.

2. **Deterministic link source for TRAC-01 — RESOLVED**
   - Decision: deterministic probe order is fixed to: (1) explicit canonical requirement ID literals, (2) structured anchor metadata already stored in traceability records, then (3) semantic fallback with downgraded confidence + reason code.
   - Rationale: preserves deterministic-first behavior from D-05/D-06 and avoids non-repeatable matching outcomes.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Node.js | Node API/E2E tests (`node --test`) | ✓ [VERIFIED: local command] | v24.14.1 [VERIFIED: local command] | — |
| pnpm | Monorepo script execution | ✓ [VERIFIED: local command] | 10.10.0 [VERIFIED: local command] | npm for direct `node --test` runs (partial) |
| Rust/Cargo toolchain | Core Phase 2 implementation/testing | ✗ [VERIFIED: local command] | — | None (blocking local execution) |
| PostgreSQL CLI (`psql`) | Local DB inspection/migration checks | ✗ [VERIFIED: local command] | — | Use SQL migration contract tests in Rust once cargo exists |

**Missing dependencies with no fallback:**
- Rust/Cargo toolchain (cannot compile/test Phase 2 locally).

**Missing dependencies with fallback:**
- `psql` missing; migration syntax can still be validated via Rust test contracts after toolchain install.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness + Node built-in test runner [VERIFIED: package.json, existing tests] |
| Config file | none (command-driven scripts) [VERIFIED: package.json] |
| Quick run command | `cargo test -p research-gateway traceability:: && node --test tests/api/phase-2-traceability.test.mjs -t` [ASSUMED] |
| Full suite command | `cargo test -p research-gateway traceability:: && node --test tests/api/phase-2-traceability.test.mjs tests/e2e/phase-2-traceability-mapping.e2e.test.mjs` [ASSUMED] |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TRAC-01 | Requirement maps to ≥1 code evidence link | unit + API | `cargo test -p research-gateway traceability::tests::maps_requirement_to_code_evidence` [ASSUMED] | ❌ Wave 0 |
| TRAC-02 | Requirement maps to test evidence when available | unit + API | `cargo test -p research-gateway traceability::tests::maps_requirement_to_test_evidence` [ASSUMED] | ❌ Wave 0 |
| TRAC-03 | Every link stores rationale + confidence | unit + API + e2e | `cargo test -p domain traceability::tests::rejects_empty_rationale` [ASSUMED] | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** targeted `cargo test -p research-gateway traceability::` + relevant `node --test` file. [ASSUMED]
- **Per wave merge:** all Phase 2 traceability Rust + Node tests.
- **Phase gate:** Full Phase 2 suite green before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `crates/domain/src/traceability.rs` tests for rationale/confidence contract
- [ ] `crates/persistence/src/postgres/traceability.rs` adapter + migration contract tests
- [ ] `services/research-gateway/src/traceability/service.rs` deterministic/fallback/ambiguity tests
- [ ] `tests/api/phase-2-traceability.test.mjs`
- [ ] `tests/e2e/phase-2-traceability-mapping.e2e.test.mjs`
- [ ] Add phase script in `package.json` (`qa:test:phase-2`) — only `qa:test:phase-1` exists today. [VERIFIED: package.json:55]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no (phase is mapping contract/service internals) [ASSUMED] | N/A |
| V3 Session Management | no (no session surface in scope) [ASSUMED] | N/A |
| V4 Access Control | yes (if exposed via API/read endpoint) [ASSUMED] | Reuse role-gated service patterns with typed unauthorized reason codes. [VERIFIED: services/research-gateway/src/validation/workflow_runs.rs:58-74] |
| V5 Input Validation | yes | Validate IDs/timestamps/anchors fail-closed before persistence. [VERIFIED: audit_artifacts.rs:101-137, ingestion/service.rs:287-324] |
| V6 Cryptography | yes (integrity hashing, not crypto protocol design) [ASSUMED] | Reuse `sha2` crate; never custom hash implementations. [VERIFIED: Cargo.toml:36, ingestion/service.rs:377-381] |

### Known Threat Patterns for Rust + SQLx traceability mapping

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection via evidence-link writes | Tampering | Parameterized `sqlx` binds only. [VERIFIED: canonical_artifacts.rs:124-165,181-213] |
| Evidence tampering via metadata rewrite | Tampering/Repudiation | Insert-only snapshots + immutability trigger pattern for metadata-bearing records. [VERIFIED: 01-VERIFICATION.md:35,44] |
| Path-scope expansion during repo scan | Information disclosure | Restrict scanning to fixed roots (`.planning`, `docs`, `_bmad`, etc.). [VERIFIED: artifact_discovery.rs:23-29] |

## Sources

### Primary (HIGH confidence)
- `.planning/phases/02-evidence-traceability-mapping/02-CONTEXT.md` — locked decisions D-01..D-12
- `.planning/REQUIREMENTS.md` — TRAC requirement definitions
- `services/research-gateway/src/ingestion/service.rs` — orchestration, deterministic assembly, error behavior
- `crates/domain/src/audit_artifacts.rs` — canonical ID and validation contract
- `crates/persistence/src/postgres/canonical_artifacts.rs` — persistence adapter/error classification/query ordering
- `crates/persistence/migrations/20260409000100_canonical_artifact_ingestion.sql` — DB constraints/index patterns
- `tests/api/phase-1-ingestion.test.mjs`, `tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` — phase-level test style
- `copilot-instructions.md` — project constraints/conventions
- `.planning/config.json` — nyquist validation enabled

### Secondary (MEDIUM confidence)
- `.planning/research/STACK.md`, `.planning/research/ARCHITECTURE.md`, `.planning/research/PITFALLS.md` — prior internal research guidance

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — directly from workspace pins and current code usage.
- Architecture: HIGH — follows existing Phase 1 implemented patterns + locked Phase 2 decisions.
- Pitfalls: MEDIUM-HIGH — mostly locked decisions + Phase 1 verifier findings; some forward-looking risks.

**Research date:** 2026-04-09  
**Valid until:** 2026-05-09
