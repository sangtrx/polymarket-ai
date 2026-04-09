# Project Research Summary

**Project:** Polymarket-AI BMAD Coverage Audit  
**Domain:** Brownfield doc-to-code traceability and deployment-readiness auditing  
**Researched:** 2026-04-09  
**Confidence:** MEDIUM-HIGH

## Executive Summary

This project is an out-of-band audit/control-plane capability for an existing Rust + Next.js monorepo. The product goal is not feature delivery in runtime trading paths; it is to produce reproducible evidence that BMAD intent (PRD, architecture, stories, roadmap) is actually implemented, risk-ranked, and suitable for release gating.

The recommended approach is: establish a canonical requirement model first, automate artifact/evidence ingestion, classify coverage with rationale, then compute policy-driven readiness scores and surface them in CI and operator views. Keep all audit logic asynchronous and read-only against runtime services, with immutable snapshots for traceability and post-release defensibility.

The main risks are false confidence (weak IDs, manual evidence, binary labels), operational friction (strict gates introduced too early), and governance drift (waivers without owners/expiry). Mitigation is clear: canonical IDs, CI-generated evidence freshness, risk-weighted policy thresholds, progressive gate rollout (observe → warn → block), and explicit owner/SLA on unresolved gaps.

## Key Findings

### Recommended Stack

Use the existing repo baseline: Rust 1.91 + Axum + SQLx/Postgres for audit orchestration and persistence, and TypeScript/Next.js for operator-facing views. This maximizes reuse of current architecture and minimizes integration risk in a brownfield codebase.

**Core technologies:**
- **Rust 1.91.x + Axum 0.8.x:** audit orchestrator/API — consistent with current backend patterns.
- **SQLx 0.8.x + Postgres 15+:** immutable audit snapshots and lineage — durable, queryable history.
- **TypeScript 5.x + Next.js 16.x:** audit dashboards in operator console — existing UI/runtime fit.
- **GitHub Actions checks:** CI gate signal — direct alignment with required checks/rulesets.

**Critical version constraints:**
- Axum 0.8.x with Tokio 1.48.x / tower-http 0.6.x
- SQLx 0.8.x with Rust 1.91.x and Postgres 15+
- Next.js 16.x with React 19.x and TypeScript 5.x

### Expected Features

v1 should prioritize deterministic, auditable release-confidence workflows; advanced automation should follow only after the baseline is trusted.

**Table stakes (must have):**
- BMAD artifact ingestion + normalization with stable requirement IDs
- Doc-to-code/test evidence mapping with explicit rationale
- Coverage model (Covered / Partial / Missing)
- Risk-weighted prioritization (not raw gap counts)
- CI execution + export + advisory/blocking gate signal
- Exception/waiver workflow (owner, reason, expiry)

**Differentiators (should have):**
- PR delta audits over trusted full baseline
- Policy-as-code readiness profiles (strict/progressive by context)
- Auto-remediation backlog generation from unresolved gaps

**Defer (v2+):**
- AI semantic mapping suggestions (human-reviewed only)
- Runtime-aware confidence fusion from telemetry/incidents

### Table Stakes vs Differentiators vs Risks (Concise)

- **Table stakes:** canonical traceability, reproducible evidence, risk-aware scoring, enforceable CI signal.
- **Differentiators:** incremental/delta workflows, policy profiles, automation of follow-up work.
- **Primary risks:** non-canonical IDs, stale manual evidence, ambiguous gate criteria, and hard-cutover gating.

### Architecture Approach

Adopt a sidecar-style control-plane extension (`services/coverage-audit`) with snapshot-based traceability and policy-driven scoring. Core components are: (1) artifact parser + canonical model, (2) evidence indexer + link graph, (3) coverage classifier, (4) readiness scorer, (5) audit store/API/UI, (6) CI gate integration. Keep runtime trading services unchanged and read-only from audit flows.

**Major components:**
1. **Traceability Orchestrator** — manages audit runs and snapshots.
2. **Artifact Parser + Evidence Indexer** — produces normalized requirements and evidence graph.
3. **Coverage Classifier + Readiness Scorer** — turns evidence into explainable gate decisions.

### Critical Pitfalls

1. **No canonical requirement IDs** — define/enforce ID schema before first matrix import.
2. **Manual evidence collection** — generate evidence in CI on every merge candidate with SHA stamps.
3. **Binary labels without risk weighting** — require severity/deploy-impact and rationale per gap.
4. **Ambiguous go/no-go criteria** — define explicit thresholds, waiver authority, and expiry.
5. **Big-bang blocking gate rollout** — stage rollout (observe, warn, block) after calibration.

## Implications for Roadmap

Based on dependencies and risk controls, suggested phase structure:

### Phase 1: Canonical Traceability Foundation
**Rationale:** All downstream automation depends on stable IDs and schema.  
**Delivers:** Canonical requirement model, audit schema (`audit_*`), evidence-quality rubric.  
**Addresses:** BMAD normalization, baseline mapping prerequisites.  
**Avoids:** Pitfalls 1 and 8 (ID inconsistency, low-quality evidence inflation).

### Phase 2: Automated Evidence Ingestion & Linking
**Rationale:** Fresh, repeatable evidence is required before scoring/gating.  
**Delivers:** Artifact parsing, code/test evidence indexer, link graph, snapshot persistence.  
**Addresses:** Core mapping + reproducible exports.  
**Avoids:** Pitfalls 2 and 5 (stale snapshots, static-only confidence).

### Phase 3: Coverage Classification & Policy Scoring
**Rationale:** Decision utility requires weighted risk semantics, not raw coverage.  
**Delivers:** Covered/Partial/Missing engine, severity model, readiness scoring policy.  
**Addresses:** Risk-weighted prioritization, deployment-readiness computation.  
**Avoids:** Pitfalls 3 and 4 (binary ambiguity, subjective release calls).

### Phase 4: Reporting UX + Progressive CI Gate
**Rationale:** Gate only after score semantics and signal quality are trusted.  
**Delivers:** Audit API + console views, waiver workflow, observe→warn→block gate rollout.  
**Addresses:** Release signal operationalization.  
**Avoids:** Pitfalls 4 and 6 (unclear policy, release paralysis from hard cutover).

### Phase 5: Operational Remediation Loop
**Rationale:** Audit must close gaps, not repeatedly report them.  
**Delivers:** Ownership/SLA enforcement for unresolved items, escalation and backlog integration.  
**Addresses:** Sustainable governance and closure behavior.  
**Avoids:** Pitfall 7 (persistent unmanaged findings).

### Research Flags

Phases likely needing deeper `/gsd-research-phase`:
- **Phase 2:** Symbol-level Rust/TS evidence extraction strategy and diff invalidation model.
- **Phase 3:** Policy-weight calibration and threshold design by deployment risk class.
- **Phase 4:** CI/ruleset integration details (waiver controls, rollout SLOs, false-positive budget).

Phases with standard patterns (can usually skip extra research):
- **Phase 1:** Canonical schema/ID modeling and Postgres snapshot tables in existing architecture.
- **Phase 5:** Ownership/SLA governance patterns are operationally standard.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM-HIGH | Strong alignment with existing codebase baseline; minor uncertainty in parser crate lock choices. |
| Features | MEDIUM | Feature set is clear, but differentiator sequencing depends on adoption maturity. |
| Architecture | HIGH | Internally consistent with current monorepo boundaries and proven sidecar pattern. |
| Pitfalls | MEDIUM | Well-supported by practice/docs, but some mitigations need local calibration data. |

**Overall confidence:** MEDIUM-HIGH

### Gaps to Address

- **Evidence granularity boundary:** define when file-level mapping is sufficient vs symbol/test/runtime proof required per requirement class.
- **Scoring calibration data:** establish baseline runs before locking hard block thresholds.
- **Waiver governance details:** finalize approver roles, expiry defaults, and emergency override auditability.
- **Runtime-signal scope:** decide minimum viable runtime checks for v1 gate without over-coupling.

## Sources

### Primary (HIGH confidence)
- `.planning/PROJECT.md` — project scope, constraints, and deployment decision utility.
- `.planning/codebase/ARCHITECTURE.md` — existing boundaries and integration expectations.
- `.planning/codebase/STRUCTURE.md` — repository layout for practical component placement.
- Internal research files: `STACK.md`, `FEATURES.md`, `ARCHITECTURE.md`, `PITFALLS.md`.

### Secondary (MEDIUM confidence)
- GitHub rulesets/protected branches/checks docs.
- GitHub SARIF and deployment environment protection docs.
- NIST SSDF (SP 800-218), SLSA, OpenSSF Scorecard.
- OPA policy-as-code docs.
- Google SRE launch/canary guidance.

---
*Research completed: 2026-04-09*  
*Ready for roadmap: yes*
