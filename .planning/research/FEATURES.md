# Feature Research

**Domain:** BMAD traceability audit + deployment-confidence workflow for an existing engineering codebase  
**Researched:** 2026-04-09  
**Confidence:** MEDIUM

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = workflow is not trusted for release decisions.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| BMAD artifact ingestion + normalization (PRD, architecture, stories, roadmap) | Audit must start from complete source-of-intent, not ad hoc docs | MEDIUM | Parse markdown/frontmatter, stable IDs, and version snapshotting per run |
| Documentation-to-code evidence mapping | Teams expect requirement → code/test evidence links | HIGH | Support file-level and symbol-level links across Rust + TS monorepo |
| Coverage status model (Covered / Partial / Missing) with rationale | Binary pass/fail is insufficient for brownfield readiness | MEDIUM | Store explicit rationale and confidence per mapping decision |
| Risk-weighted gap prioritization | Release decisions depend on impact, not raw gap count | MEDIUM | Weight by deployment criticality, security relevance, and runtime exposure |
| CI-integrated audit execution with reproducible outputs | Manual audits are too stale and non-repeatable | MEDIUM | Run in pipeline; produce deterministic markdown + JSON artifacts |
| Release gate signal (advisory or blocking) | Teams expect auditable go/no-go criteria before deploy | MEDIUM | Integrate with required checks/rulesets and explicit override path |
| Exception/waiver workflow (owner + expiry + reason) | Real systems need managed partial compliance | LOW | Avoid permanent waivers; force re-review on expiry |
| Evidence export for humans and machines | Stakeholders need readable report + automation input | LOW | Emit markdown summary plus machine-readable JSON/CSV |

### Differentiators (Competitive Advantage)

Features that raise confidence quality and reduce maintainer effort.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| AI-assisted semantic traceability suggestions with reviewer confirmation | Cuts mapping toil while preserving audit integrity via human-in-the-loop | HIGH | Require confidence scoring and mandatory reviewer accept/reject |
| PR delta audit (changed docs/changed code only) | Fast feedback for daily development, full scan reserved for milestone gates | HIGH | Needs diff-aware dependency graph and invalidation strategy |
| Runtime-aware confidence (telemetry/incidents/tests folded into readiness score) | Prevents “mapped in docs” but operationally unsafe releases | HIGH | Blend static coverage with live reliability/security signals |
| Policy-as-code deployment readiness profiles (e.g., strict/progressive) | Allows consistent governance across environments without manual interpretation | MEDIUM | Encode gate rules declaratively; version policy with repo |
| Auto-remediation backlog generation (ranked tickets from uncovered gaps) | Turns audit output into execution plan immediately | MEDIUM | Create issue payloads with owners, risk tags, and evidence links |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Fully automated “AI decides pass/fail” with no reviewer | Looks fast and “objective” | High false confidence; brittle in ambiguous requirements; poor audit defensibility | AI suggestions + mandatory human approval for gate-affecting decisions |
| Single vanity coverage score as primary KPI | Easy dashboarding | Hides critical missing controls under averaged score; gaming risk | Show score plus mandatory critical-gap list and weighted risk bands |
| Always-on full-repo deep scans on every commit | Appears thorough | Slow, expensive, and noisy; teams bypass gates | Use PR delta scans by default + scheduled/full milestone scans |
| Unbounded custom schemas per team | Feels flexible | Breaks comparability and cross-service governance | Provide extensible core schema with constrained extension points |

## Feature Dependencies

```
Artifact Ingestion + Normalization
    └──requires──> Traceability Mapping
                      └──requires──> Coverage Status Model
                                         └──requires──> Risk Prioritization
                                                        └──requires──> Release Gate Signal

Coverage Status Model ──requires──> Exception/Waiver Workflow
Traceability Mapping ──enhances──> Evidence Export
Policy-as-Code Profiles ──enhances──> Release Gate Signal
PR Delta Audit ──requires──> Baseline Full Audit + Dependency Graph
AI Semantic Suggestions ──requires──> Traceability Mapping + Reviewer Workflow

Single Vanity Score ──conflicts──> Risk-Weighted Prioritization
Fully Automated AI Pass/Fail ──conflicts──> Reviewer Sign-off
```

### Dependency Notes

- **Traceability mapping requires normalized artifacts:** inconsistent IDs/naming breaks stable requirement-to-code links.
- **Coverage model requires mapping:** status labels are meaningless without explicit evidence graph.
- **Risk prioritization requires coverage status:** only unresolved partial/missing items should influence deployment risk.
- **Release gate requires risk model + waiver model:** otherwise teams cannot unblock urgent releases with accountable exceptions.
- **PR delta audit requires a trusted baseline:** incremental checks are only valid when full-scan baseline exists.
- **AI suggestions require reviewer workflow:** keeps differentiator from becoming unsafe automation.

## MVP Definition

### Launch With (v1)

- [ ] BMAD artifact ingestion + normalization — foundation for all downstream traceability
- [ ] Documentation-to-code evidence mapping — core audit function
- [ ] Coverage status model (Covered/Partial/Missing) — clear audit semantics
- [ ] Risk-weighted prioritization — deployment-relevant decision support
- [ ] CI execution + report export + advisory/blocking gate — operational release-readiness workflow
- [ ] Exception/waiver workflow — pragmatic and auditable rollout control

### Add After Validation (v1.x)

- [ ] PR delta audit — add once full audits are stable and trusted
- [ ] Policy-as-code readiness profiles — add when multiple environments/teams need differentiated governance
- [ ] Auto-remediation backlog generation — add when owners/workflows for ticket automation are agreed

### Future Consideration (v2+)

- [ ] AI semantic traceability suggestions — add after high-quality labeled mapping corpus exists
- [ ] Runtime-aware confidence fusion — add once telemetry quality and incident taxonomy are mature

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| BMAD artifact ingestion + normalization | HIGH | MEDIUM | P1 |
| Documentation-to-code evidence mapping | HIGH | HIGH | P1 |
| Coverage status model + rationale | HIGH | MEDIUM | P1 |
| Risk-weighted gap prioritization | HIGH | MEDIUM | P1 |
| CI audit + release gate integration | HIGH | MEDIUM | P1 |
| Exception/waiver workflow | HIGH | LOW | P1 |
| PR delta audit | HIGH | HIGH | P2 |
| Policy-as-code readiness profiles | MEDIUM | MEDIUM | P2 |
| Auto-remediation backlog generation | MEDIUM | MEDIUM | P2 |
| AI semantic suggestions | MEDIUM | HIGH | P3 |
| Runtime-aware confidence fusion | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have after core workflow is trusted
- P3: Strategic differentiator after operational baseline is stable

## Competitor Feature Analysis

| Feature | Competitor A | Competitor B | Our Approach |
|---------|--------------|--------------|--------------|
| Release gating | GitHub rulesets + required status checks | Azure DevOps branch policies/checks | Use CI audit status as first-class required gate with explicit waiver mechanism |
| Security/compliance signal ingestion | GitHub code scanning/SARIF ecosystem | GitLab security & compliance dashboards | Blend BMAD traceability + code/test evidence + risk policy in one deploy-confidence view |
| Provenance/assurance posture | SLSA + attestations in modern CI | Vendor-specific compliance pipelines | Start with traceability + risk gaps; evolve toward provenance-aware readiness profiles |

## Sources

- https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets (MEDIUM)
- https://docs.github.com/en/code-security/code-scanning/integrating-with-code-scanning/uploading-a-sarif-file-to-github (MEDIUM)
- https://slsa.dev/spec/v1.0/ (MEDIUM)
- https://openssf.org/projects/scorecard/ (MEDIUM)
- https://www.openpolicyagent.org/docs/latest/ (MEDIUM)
- https://csrc.nist.gov/pubs/sp/800/218/final (MEDIUM)
- Internal context: `.planning/PROJECT.md`, `.planning/codebase/CONCERNS.md`, `.planning/codebase/TESTING.md` (HIGH)

---
*Feature research for: BMAD coverage-audit and release-readiness workflows*  
*Researched: 2026-04-09*
