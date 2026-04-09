# Architecture Research

**Domain:** Brownfield doc-to-code traceability + release-readiness audit in multi-service trading stack
**Researched:** 2026-04-09
**Confidence:** HIGH

## Standard Architecture

### System Overview

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│                         Audit Interfaces (out-of-band)                      │
├──────────────────────────────────────────────────────────────────────────────┤
│  Operator Console Audit Views   CI Gate Checks   Export/Report API          │
└───────────────────────────────┬──────────────────────────────────────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────────────────────┐
│                      Audit Control / Orchestration Layer                     │
├──────────────────────────────────────────────────────────────────────────────┤
│  Traceability Orchestrator  |  Coverage Classifier  |  Readiness Scorer     │
│  (workflow + snapshots)     |  (covered/partial/miss)| (weighted risk score) │
└───────────────────────────────┬──────────────────────────────────────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────────────────────┐
│                         Evidence & Lineage Layer                             │
├──────────────────────────────────────────────────────────────────────────────┤
│  BMAD Artifact Parser  |  Code Evidence Indexer  |  Link Graph + Audit Log  │
└───────────────────────────────┬──────────────────────────────────────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────────────────────┐
│                       Existing Runtime Services (unchanged)                  │
├──────────────────────────────────────────────────────────────────────────────┤
│  control-api  governance  risk-engine  execution-engine  reporting  console  │
│  (read-only integration via APIs, DB views, git/tree scan, test outputs)    │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| Traceability Orchestrator | Runs audit jobs from artifact set to scored output | New module/service in control plane; async job runner |
| Artifact Parser | Normalizes PRD/architecture/stories/roadmap into canonical requirement records | Deterministic parser + schema validation |
| Code Evidence Indexer | Collects file-level/code-symbol/test evidence per requirement | Static scan + lightweight metadata extraction |
| Coverage Classifier | Labels Covered / Partial / Missing with rationale | Rules engine with explicit confidence fields |
| Readiness Scorer | Computes deployment readiness from weighted gaps | Policy config + score calculator + threshold gates |
| Audit Store | Persists snapshots, links, score history, and provenance | Postgres tables in isolated schema (`audit_*`) |
| Audit API/UI | Exposes reports and drill-down traceability | New read endpoints + operator-console audit pages |

## Recommended Project Structure

```text
services/
├── coverage-audit/                    # New out-of-band audit service
│   ├── src/orchestration/             # Audit workflow + job state machine
│   ├── src/parsing/                   # BMAD artifact ingestion/parsing
│   ├── src/indexing/                  # Code/test evidence indexing
│   ├── src/classification/            # Coverage decision logic
│   ├── src/scoring/                   # Release-readiness scoring policies
│   └── src/api/                       # Audit/report endpoints (read-focused)
crates/
├── domain/src/audit.rs                # Shared audit contracts, enums, DTOs
├── persistence/src/postgres/audit_*.rs# Audit repositories
apps/operator-console/src/
├── app/(dashboard)/audit/             # Audit and readiness pages
└── lib/audit/                         # Typed clients for audit APIs
tests/
└── audit/                             # Contract + workflow tests for audit layer
```

### Structure Rationale

- **services/coverage-audit/** keeps audit complexity out of latency-sensitive runtime services.
- **crates/domain + persistence** reuses existing port/adapter conventions and avoids ad-hoc schema access.
- **console audit pages** consume only read APIs, preserving existing control flows.

## Architectural Patterns

### Pattern 1: Sidecar-Style Control Plane Extension
**What:** Add audit as a parallel control-plane capability, not inside execution/risk hot paths.  
**When to use:** Brownfield systems where runtime stability is primary constraint.  
**Trade-offs:** Slightly more integration plumbing; major reduction in blast radius.

### Pattern 2: Snapshot-Based Traceability
**What:** Build immutable audit snapshots keyed by commit/artifact version/time window.  
**When to use:** Need reproducible release decisions and historical comparisons.  
**Trade-offs:** Extra storage; much better auditability and rollback diagnostics.

### Pattern 3: Policy-Driven Readiness Scoring
**What:** Score is computed from configurable weighted rules (e.g., missing critical controls > missing docs).  
**When to use:** Teams need explicit go/no-go gates and explainable thresholds.  
**Trade-offs:** Requires governance for policy changes; avoids opaque “gut-feel” release calls.

## Data Flow

### Audit Run Flow (doc-to-code)

```text
BMAD docs + code refs + tests
    ↓
Artifact Parser → Canonical requirements
    ↓
Evidence Indexer (code/tests/routes/migrations)
    ↓
Link Graph Builder (requirement ↔ evidence)
    ↓
Coverage Classifier (covered/partial/missing + rationale)
    ↓
Snapshot stored in Postgres audit schema
    ↓
Audit API/UI renders matrix + unresolved gaps
```

### Release-Readiness Flow

```text
Latest audit snapshot + runtime health/test signals + policy weights
    ↓
Readiness Scorer
    ↓
Risk-tiered score + blocking findings
    ↓
CI gate + operator dashboard decision panel
```

### Key Data Flows

1. **Requirement lineage flow:** PRD/architecture/story item → normalized ID → mapped code/test evidence → classification rationale.
2. **Release gate flow:** snapshot + policy → score + blockers → pass/warn/fail signal for release readiness.

## Build Order & Dependencies (Roadmap Implications)

1. **Canonical Requirement Model + Audit Schema**
   - Dependency base for every later phase.
2. **Artifact Parsing + Evidence Indexing (read-only)**
   - Requires model/schema; must remain non-invasive to runtime services.
3. **Coverage Classification Engine**
   - Depends on parsed requirements + indexed evidence.
4. **Readiness Scoring + Policy Configuration**
   - Depends on stable classification outputs.
5. **Audit APIs + Operator Console Views**
   - Depends on persisted snapshots and scorer outputs.
6. **CI/Release Gate Integration**
   - Final phase after score semantics are trusted.

## Anti-Patterns

### Anti-Pattern 1: In-Path Runtime Coupling
**What people do:** Call audit/scoring logic synchronously from execution/risk request paths.  
**Why it’s wrong:** Adds latency and failure coupling to trading-critical workflows.  
**Do this instead:** Keep audit asynchronous/out-of-band with cached snapshot reads.

### Anti-Pattern 2: One-Off Spreadsheet Traceability
**What people do:** Manual mapping outside source-controlled system.  
**Why it’s wrong:** Becomes stale immediately and cannot gate releases reliably.  
**Do this instead:** Persist machine-readable links and recompute per commit/tag.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Git repository | Read-only tree/metadata scan by commit SHA | Primary provenance anchor |
| CI pipeline | Invoke readiness scoring as quality gate | Block on critical missing items |
| Postgres | Isolated `audit_*` tables and snapshot history | No writes into runtime business tables |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| coverage-audit ↔ control-api | Internal API or shared orchestrator traits | Keep existing control routes stable |
| coverage-audit ↔ domain/persistence crates | Port/adapter pattern | Reuse established architectural style |
| operator-console ↔ audit API | Typed read clients | No direct DB coupling from UI |
| audit service ↔ runtime services | Read-only endpoints/events/DB views | Never inject into hot execution loop |

## Sources

- Internal architecture baseline: `/home/epic/polymarket-ai/.planning/codebase/ARCHITECTURE.md` (HIGH)
- Internal structure map: `/home/epic/polymarket-ai/.planning/codebase/STRUCTURE.md` (HIGH)
- Milestone/project scope: `/home/epic/polymarket-ai/.planning/PROJECT.md` (HIGH)

---
*Architecture research for: BMAD coverage-audit and release-readiness workflows*
*Researched: 2026-04-09*
