# Roadmap: Polymarket-AI BMAD Coverage Audit

## Overview

This roadmap delivers an end-to-end BMAD coverage audit workflow: ingest and normalize BMAD artifacts, link them to code/test evidence, classify coverage, prioritize risk, and produce CI/readiness outputs suitable for deployment decisions.

## Phases

- [x] **Phase 1: Canonical Artifact Ingestion** - BMAD artifacts are ingested into a stable, versioned requirement dataset.
- [x] **Phase 2: Evidence Traceability Mapping** - Each normalized requirement is linked to code/test evidence with rationale and confidence.
- [x] **Phase 3: Coverage Classification Matrix** - All scoped requirements receive explainable Covered/Partial/Missing outcomes in one complete matrix. (completed 2026-04-09)
- [ ] **Phase 4: Deployment Risk Prioritization** - Partial/Missing gaps are severity-ranked into a deployment-focused fix order.
- [ ] **Phase 5: CI Readiness Signal & Reporting** - Deterministic CI audit outputs, waiver controls, and readiness reports are produced for release decisions.

## Phase Details

### Phase 1: Canonical Artifact Ingestion
**Goal**: Users can load BMAD planning artifacts into one normalized, versioned audit baseline.
**Depends on**: Nothing (first phase)
**Requirements**: ARTF-01, ARTF-02, ARTF-03, ARTF-04, ARTF-05
**Success Criteria** (what must be TRUE):
  1. User can ingest PRD, architecture, story, and roadmap artifacts into one canonical dataset.
  2. User can see stable requirement IDs assigned across all ingested BMAD items.
  3. User can identify snapshot version metadata for each ingestion run.
**Plans**: 4 plans

Plans:
- [x] 01-01-PLAN.md — Define canonical contracts and persistence schema for stable IDs and immutable snapshots
- [x] 01-02-PLAN.md — Implement scoped artifact discovery, parsing, and canonical snapshot assembly pipeline
- [x] 01-03-PLAN.md — Wire executable ingestion command and end-to-end deterministic verification
- [x] 01-04-PLAN.md — Gap closure: enforce immutable snapshot metadata and deterministic snapshot identity regression coverage

### Phase 2: Evidence Traceability Mapping
**Goal**: Users can trace each canonical requirement to concrete implementation and validation evidence.
**Depends on**: Phase 1
**Requirements**: TRAC-01, TRAC-02, TRAC-03
**Success Criteria** (what must be TRUE):
  1. User can map every canonical requirement ID to one or more code evidence references.
  2. User can attach related test evidence to requirement mappings when available.
  3. User can review rationale and confidence for every traceability link.
**Plans**: 3 plans

Plans:
- [x] 02-01-PLAN.md — Define traceability domain/persistence contracts for deterministic requirement-to-evidence links
- [x] 02-02-PLAN.md — Implement traceability mapping service, CLI wiring, and phase-2 API/E2E verification
- [x] 02-03-PLAN.md — Gap closure: tighten code-evidence qualification and transactional persistence validation

### Phase 3: Coverage Classification Matrix
**Goal**: Users can evaluate coverage status for every scoped requirement with complete, explainable classification.
**Depends on**: Phase 2
**Requirements**: COVR-01, COVR-02, COVR-03
**Success Criteria** (what must be TRUE):
  1. User can classify every scoped requirement as Covered, Partial, or Missing.
  2. User can inspect explicit rationale for each Partial or Missing result.
  3. User can access a complete coverage matrix that includes all scoped BMAD requirements and linked evidence.
**Plans**: 3 plans

Plans:
- [x] 03-01-PLAN.md — Implement coverage contracts, deterministic classifier rules, and full-baseline matrix assembly service
- [x] 03-02-PLAN.md — Add transactional coverage persistence schema/adapter and service persistence wiring
- [x] 03-03-PLAN.md — Expose classify-coverage CLI and phase-3 API/E2E verification command

### Phase 4: Deployment Risk Prioritization
**Goal**: Users can identify which unresolved coverage gaps most threaten safe deployment.
**Depends on**: Phase 3
**Requirements**: RISK-01, RISK-02
**Success Criteria** (what must be TRUE):
  1. User can assign deployment-impact severity to every Partial or Missing requirement.
  2. User can generate a fix list ordered by highest deployment risk first.
**Plans**: TBD

### Phase 5: CI Readiness Signal & Reporting
**Goal**: Users can run deterministic CI audits, manage justified waivers, and consume readiness outputs for go/no-go decisions.
**Depends on**: Phase 4
**Requirements**: GATE-01, GATE-02, GATE-03, RPTG-01, RPTG-02
**Success Criteria** (what must be TRUE):
  1. User can execute the audit in CI and receive deterministic artifacts for a given commit snapshot.
  2. User can receive an advisory deployment-readiness signal derived from current audit results.
  3. User can manage waivers for unresolved gaps with owner, reason, and expiry metadata.
  4. User can export audit results as markdown and JSON and review a summary report with coverage posture, top risks, and readiness recommendation.
**Plans**: TBD
**UI hint**: yes

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Canonical Artifact Ingestion | 4/4 | Complete | 2026-04-09 |
| 2. Evidence Traceability Mapping | 3/3 | Complete | 2026-04-09 |
| 3. Coverage Classification Matrix | 3/3 | Complete    | 2026-04-09 |
| 4. Deployment Risk Prioritization | 0/TBD | Not started | - |
| 5. CI Readiness Signal & Reporting | 0/TBD | Not started | - |
