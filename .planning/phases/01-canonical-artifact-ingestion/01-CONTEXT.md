# Phase 1: Canonical Artifact Ingestion - Context

**Gathered:** 2026-04-09  
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the ingestion foundation that loads BMAD PRD, architecture, story, and roadmap artifacts into a stable, versioned canonical dataset. This phase defines data contracts, normalization behavior, and snapshot metadata only; it does not implement traceability scoring, risk ranking, deployment, or code remediation.

</domain>

<decisions>
## Implementation Decisions

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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project intent and constraints
- `.planning/PROJECT.md` — Core value, scope boundaries, and deployment-readiness framing.
- `.planning/REQUIREMENTS.md` — Phase 1 requirement set (`ARTF-01..ARTF-05`) and overall phase mapping.

### Phase contract
- `.planning/ROADMAP.md` — Phase 1 goal and success criteria for canonical ingestion baseline.
- `.planning/STATE.md` — Current project position and active phase context.

### Research guidance
- `.planning/research/SUMMARY.md` — Recommended architecture and risk-informed guidance for audit workflow evolution.
- `.planning/research/FEATURES.md` — Table-stakes vs deferred capabilities to avoid premature scope expansion.

### Existing technical baseline
- `.planning/codebase/ARCHITECTURE.md` — Existing layered architecture and service composition patterns.
- `.planning/codebase/STRUCTURE.md` — Directory/module layout and expected integration points.
- `.planning/codebase/CONVENTIONS.md` — Naming, error handling, and module conventions.
- `.github/get-shit-done/bin/gsd-tools.cjs` — Existing GSD CLI capabilities for roadmap/phase metadata access.
- `.github/get-shit-done/bin/lib/frontmatter.cjs` — Existing frontmatter parsing utility patterns.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `node ".github/get-shit-done/bin/gsd-tools.cjs" roadmap get-phase` can extract scoped phase metadata for ingestion seed context.
- `.github/get-shit-done/bin/lib/frontmatter.cjs` provides reusable parsing patterns for mixed markdown/frontmatter documents.
- `crates/domain/src/` and `crates/persistence/src/postgres/` establish domain-contract and repository layering patterns suitable for ingestion models.

### Established Patterns
- Rust services use explicit typed contracts and fail-closed/typed-error behavior for control paths.
- New persistence behavior should follow migration-first workflow in `crates/persistence/migrations/` and adapter modules in `crates/persistence/src/postgres/`.
- Repo conventions prefer explicit machine-readable codes and structured outputs over implicit success fallbacks.

### Integration Points
- Phase 1 implementation should connect to phase-scoped planning artifacts in `.planning/` and avoid coupling to runtime trading loops.
- If persistent ingestion state is added, integrate through existing persistence crate patterns rather than ad hoc file-only state.
- Any CLI/tooling augmentation should align with existing `gsd-tools.cjs` command surface and naming conventions.

</code_context>

<specifics>
## Specific Ideas

- Preserve provenance by keeping source-specific IDs while linking equivalences.
- Keep v1 deterministic and auditable with immutable snapshot metadata per run.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-canonical-artifact-ingestion*  
*Context gathered: 2026-04-09*
