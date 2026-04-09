# Phase 2: Evidence Traceability Mapping - Context

**Gathered:** 2026-04-09  
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the traceability layer that links each canonical BMAD requirement ID to concrete code and test evidence, including rationale and confidence metadata. This phase defines evidence-link contracts and mapping behavior only; it does not classify coverage outcomes, prioritize risk, or produce final readiness recommendations.

</domain>

<decisions>
## Implementation Decisions

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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` — Phase 2 goal, dependencies, and success criteria.
- `.planning/REQUIREMENTS.md` — Traceability requirements (`TRAC-01..TRAC-03`) and phase mapping.
- `.planning/PROJECT.md` — Project constraints and deployment-readiness framing.
- `.planning/STATE.md` — Current project state and phase progression.

### Upstream ingestion contracts (Phase 1 baseline)
- `.planning/phases/01-canonical-artifact-ingestion/01-CONTEXT.md` — Locked Phase 1 decisions to preserve.
- `.planning/phases/01-canonical-artifact-ingestion/01-VERIFICATION.md` — Verified phase truths and evidence expectations.
- `.planning/phases/01-canonical-artifact-ingestion/01-canonical-artifact-ingestion-04-SUMMARY.md` — Gap-closure behavior for immutable snapshots and deterministic IDs.

### Existing implementation anchors
- `crates/domain/src/audit_artifacts.rs` — Canonical requirement/snapshot contracts and ID behavior.
- `crates/persistence/src/postgres/canonical_artifacts.rs` — Persistence patterns for canonical artifacts and typed errors.
- `services/research-gateway/src/ingestion/service.rs` — Current ingestion orchestration and output metadata.
- `services/research-gateway/src/ingestion/snapshot_builder.rs` — Canonical item assembly and conflict/equivalence handling.

### Conventions and testing patterns
- `.planning/codebase/CONVENTIONS.md` — Error handling, naming, and verification conventions.
- `.planning/codebase/STRUCTURE.md` — Integration points for new phase logic.
- `tests/api/phase-1-ingestion.test.mjs` — API contract test style for phase-level behavior.
- `tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` — Determinism E2E test pattern.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Canonical contract types and deterministic ID helpers in `crates/domain/src/audit_artifacts.rs`.
- Postgres adapter and typed persistence error mapping patterns in `crates/persistence/src/postgres/canonical_artifacts.rs`.
- Research-gateway ingestion pipeline outputs (`canonical_requirement_ids`, `counts_by_artifact_type`, `snapshot_digest`) in `services/research-gateway/src/ingestion/service.rs`.

### Established Patterns
- Machine-readable error payloads and explicit reason codes over success-shaped fallbacks.
- Migration-first persistence evolution under `crates/persistence/migrations/` with adapter tests in the corresponding module.
- Deterministic behavior guarded by explicit tests (unit + API + E2E) and stable ordering.

### Integration Points
- Phase 2 mapping logic should consume canonical Phase 1 outputs from research-gateway ingestion.
- Persistent traceability storage should follow `crates/persistence/src/postgres/` adapter conventions.
- Phase-level verification should extend existing Node test structure under `tests/api/` and `tests/e2e/`.

</code_context>

<specifics>
## Specific Ideas

- Produce deterministic mapping rows keyed by `canonical_requirement_id`.
- Require explicit rationale and confidence for every code/test link.
- Keep unresolved or ambiguous mappings visible instead of collapsing uncertainty.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 02-evidence-traceability-mapping*  
*Context gathered: 2026-04-09*
