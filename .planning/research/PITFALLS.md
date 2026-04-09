# Pitfalls Research

**Domain:** BMAD coverage-audit and release-readiness workflows (brownfield engineering systems)  
**Researched:** 2026-04-09  
**Confidence:** MEDIUM

## Critical Pitfalls

### Pitfall 1: No Canonical Requirement IDs Across BMAD Artifacts

**What goes wrong:**  
Teams attempt traceability with prose-only mapping (story titles, loose links), so PRD/architecture/story/roadmap coverage cannot be reliably joined to code evidence.

**Why it happens:**  
Existing systems were not designed for auditability; teams start with spreadsheets before defining a normalized requirement taxonomy.

**How to avoid:**  
Create a single requirement ID schema first (e.g., `PRD-2.3`, `ARCH-4.1`, `STORY-3.2`) and require every audit row, test, and code evidence record to reference those IDs.

**Warning signs:**  
- Same requirement appears under multiple names  
- “Covered” claims cannot be traced to one stable ID  
- Reviewers disagree whether two rows are duplicates

**Phase to address:**  
Phase 1 — Traceability model + evidence schema definition.

---

### Pitfall 2: Manual, Snapshot-Based Evidence Collection

**What goes wrong:**  
Coverage audits are assembled from one-off screenshots and ad hoc notes, then become stale within days as code changes.

**Why it happens:**  
Teams optimize for first report delivery, not repeatability; no CI automation for evidence extraction.

**How to avoid:**  
Automate evidence generation from repository metadata (tests, routes, migrations, feature flags, service startup checks) and regenerate on every merge candidate.

**Warning signs:**  
- Audit file updated only before releases  
- Evidence links point to old commits  
- “Last verified” dates drift from current branch SHA

**Phase to address:**  
Phase 2 — Automation pipeline for evidence harvesting and freshness checks.

---

### Pitfall 3: Binary Coverage Labels Without Risk Weighting

**What goes wrong:**  
Everything is marked Covered/Not Covered without distinguishing “partial but safe” vs “partial and deployment-blocking.”

**Why it happens:**  
Coverage frameworks are treated as completeness exercises, not risk instruments for go/no-go decisions.

**How to avoid:**  
Add severity and deploy-impact dimensions (`Critical/High/Medium/Low`) and require rationale for each partial/missing item tied to production risk.

**Warning signs:**  
- Large “Partial” bucket with no priority order  
- Low-impact docs gaps block release while runtime safety gaps remain open  
- Go/no-go discussions become subjective

**Phase to address:**  
Phase 3 — Risk model and release decision policy.

---

### Pitfall 4: Ambiguous Go/No-Go Gate Criteria

**What goes wrong:**  
Readiness checks exist, but no explicit threshold says when release is permitted, blocked, or exception-approved.

**Why it happens:**  
Teams assume stakeholders share an implicit quality bar; exceptions are handled informally.

**How to avoid:**  
Define explicit gate policy: mandatory checks, failure classes, waiver authority, expiration of waivers, and rollback trigger conditions.

**Warning signs:**  
- “Ship anyway?” debates recur each release  
- Different maintainers make different calls on same evidence  
- Waivers have no owner or expiry

**Phase to address:**  
Phase 3 — Governance policy for confidence gates.

---

### Pitfall 5: Auditing Intent but Ignoring Runtime Reality

**What goes wrong:**  
Teams map BMAD artifacts to code presence, but miss deploy-critical runtime gaps (e.g., service boots but does not serve traffic, in-memory-only audit sinks, unsigned auth assumptions).

**Why it happens:**  
Traceability is scoped to static code references instead of executable behavior checks.

**How to avoid:**  
Include runtime verification evidence in coverage rules: listener bind checks, health/readiness probes, durable audit persistence validation, auth verification tests.

**Warning signs:**  
- “Implemented” features fail in executable startup tests  
- Coverage reports pass while staging environment is non-functional  
- Critical controls rely on placeholders/scaffolds

**Phase to address:**  
Phase 2 (evidence automation) and Phase 4 (release gate integration with runtime checks).

---

### Pitfall 6: Gates Introduced All-at-Once in Mature CI/CD

**What goes wrong:**  
A strict gate is flipped on in one change, causing noisy failures, team bypass behavior, and emergency disablement.

**Why it happens:**  
No baseline period to calibrate signal quality, flake rate, or expected failure modes.

**How to avoid:**  
Roll out in stages: observe-only mode, warning mode with SLO, then blocking mode once false-positive rate is acceptable.

**Warning signs:**  
- Sudden spike in red pipelines after gate launch  
- Frequent “temporarily disable gate” PRs  
- Engineers rerun/rebase to “green by luck”

**Phase to address:**  
Phase 4 — Progressive rollout and calibration.

---

### Pitfall 7: No Ownership or SLA for Partial/Missing Items

**What goes wrong:**  
Audit identifies gaps, but none are assigned with deadlines; debt accumulates and readiness reports repeat the same findings.

**Why it happens:**  
Audit is treated as a document deliverable, not an operational control loop.

**How to avoid:**  
Require owner, due date, risk class, and planned mitigation for every non-covered item; track in the same system as delivery work.

**Warning signs:**  
- Same unresolved findings across multiple releases  
- “Known issue” tags without closure dates  
- No escalation path for overdue high-risk gaps

**Phase to address:**  
Phase 5 — Operationalization and remediation governance.

---

### Pitfall 8: Measuring Documentation Presence Instead of Evidence Quality

**What goes wrong:**  
Teams score high on “coverage” by linking docs to docs, while lacking proof of behavior (tests, runtime logs, failure-mode validation).

**Why it happens:**  
Metric gaming: easier to create references than to produce verifiable execution evidence.

**How to avoid:**  
Define evidence-quality tiers (Design-only, Code-linked, Test-verified, Runtime-verified) and enforce minimum tier per requirement class.

**Warning signs:**  
- High coverage percentage with low confidence in deployment meetings  
- Few links to tests/telemetry compared to links to markdown  
- Post-release incidents in supposedly “fully covered” areas

**Phase to address:**  
Phase 1 (quality rubric) and Phase 4 (gate enforcement on evidence tier).

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Traceability model setup | Requirement IDs are inconsistent | Freeze canonical ID schema before first matrix import |
| Evidence automation | Collection is manual and stale | CI-generated evidence manifest with commit SHA stamping |
| Risk scoring policy | Coverage labels are not decision-useful | Add deploy-impact severity + blocking thresholds |
| Gate rollout | Blocking gate creates release paralysis | Start non-blocking, measure noise, then enforce |
| Ongoing operations | Findings never close | Owner + SLA + escalation for each unresolved high-risk gap |

## “Looks Done But Isn’t” Checklist

- [ ] Matrix exists, but each row lacks stable requirement IDs  
- [ ] Coverage percentages exist, but no risk-weighted prioritization  
- [ ] Gate is configured, but waiver policy/expiry is undefined  
- [ ] Evidence links exist, but do not include executable/runtime verification  
- [ ] Findings are documented, but no owner/due date/escalation

## Sources

- `.planning/PROJECT.md` (project goals, constraints, deployment decision utility) — HIGH  
- `.planning/codebase/CONCERNS.md` (runtime/serving/audit/auth risks relevant to readiness gating) — HIGH  
- GitHub Docs — Protected branches and required checks: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches — MEDIUM  
- GitHub Docs — Deployment environments and protection rules: https://docs.github.com/en/actions/deployment/targeting-different-environments/using-environments-for-deployment — MEDIUM  
- Google SRE Book — Reliable Product Launches: https://sre.google/sre-book/reliable-product-launches/ — MEDIUM  
- Google SRE Workbook — Canarying Releases: https://sre.google/workbook/canarying-releases/ — MEDIUM  
- NIST SP 800-218 (SSDF): https://csrc.nist.gov/pubs/sp/800/218/final — MEDIUM

---
*Pitfalls research for BMAD coverage-audit and release-readiness workflows*
