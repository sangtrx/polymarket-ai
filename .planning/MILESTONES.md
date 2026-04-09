# Milestones

## v1.0 BMAD Coverage Audit (Shipped: 2026-04-09)

**Phases completed:** 5 phases, 16 plans, 26 tasks

**Key accomplishments:**

- Deterministic canonical requirement ID contracts, immutable snapshot schema constraints, and typed Postgres adapter flows for snapshots/items/equivalences/conflicts were delivered for ARTF-05.
- Scoped BMAD markdown ingestion now discovers intent files, parses requirement-bearing structures with fail/warn validation, and assembles canonical snapshot inputs with explicit equivalence/conflict findings.
- Research-gateway now exposes a runnable canonical ingestion CLI with deterministic digest/ID evidence, conflict visibility, and one-command phase QA coverage.
- Insert-only snapshot writes plus DB trigger guard now prevent canonical metadata rewrites while timestamp-aware snapshot identity avoids replay collisions.
- Deterministic requirement→code/test traceability contracts, immutable Postgres mapping schema, and SQLx adapter APIs with machine-readable error codes.
- Traceability mapping is now executable end-to-end with deterministic requirement→evidence JSON, explicit unresolved outcomes, and one-command phase QA validation.
- Traceability gap closure now enforces implementation-only code evidence, validator-approved persisted links with reason_code retention, and atomic deferred-constraint-safe writes.
- Implemented typed coverage contracts and deterministic traceability-to-coverage mapping with a complete, sorted matrix output over canonical requirement baselines.
- Added transactional persistence for coverage snapshots with immutable schema guarantees, deterministic retrieval ordering, and service-level persistence wiring.
- Exposed coverage classification as `classify-coverage` and added end-to-end phase QA suites proving complete, explainable, and deterministic matrix behavior.
- Delivered deterministic risk severity/scoring contracts and an unresolved-row classifier that fails closed for unknown reason lineage.
- Implemented deterministic risk ranking that emits complete fix items sorted by deployment impact with stable one-based priority ranks.
- Shipped immutable risk persistence plus a JSON-only `prioritize-risk` command with deterministic API/E2E verification coverage.
- Readiness waiver contracts and immutable persistence now enforce governed metadata, UTC validation, unresolved-risk eligibility, and deterministic waiver listing behavior.
- Waiver-aware readiness scoring now runs inside a single deterministic phase-5 CLI chain that also emits readiness-report JSON/Markdown artifacts and dispatches report-export orchestration.
- Deterministic readiness JSON/markdown export artifacts were wired into reporting workflows with authenticated route aliases and one-command phase-5 QA coverage.

**Known gaps accepted as tech debt:**

- **GATE-01** — phase QA command portability depends on `cargo` being on PATH in CI shells.
- **RPTG-02** — `run-phase5-chain` markdown rendering diverges from canonical readiness markdown contract.

---
