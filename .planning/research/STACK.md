# Stack Research

**Domain:** BMAD artifact traceability + deployment-readiness audit for a brownfield engineering monorepo  
**Researched:** 2026-04-09  
**Confidence:** MEDIUM-HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | 1.91.x | Audit orchestrator, classifiers, policy evaluation | Matches existing service stack and domain contracts, minimizes integration friction |
| Axum | 0.8.x | Audit/report API surface | Already used in existing backend services, consistent middleware/state patterns |
| SQLx + PostgreSQL | SQLx 0.8.x / PG 15+ | Durable audit snapshots, traceability links, readiness history | Existing persistence model and migrations already use this path |
| TypeScript + Next.js | TS 5.x / Next 16.x | Operator-facing audit matrix and readiness dashboards | Reuses existing operator console runtime and deployment flow |
| GitHub Actions | Current repo workflows | CI audit execution and release gate signal | Native fit for existing CI model and branch protection checks |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `comrak` or `pulldown-cmark` (Rust) | Current stable | Parse BMAD markdown artifacts into structured nodes | Required for robust PRD/roadmap/story ingestion |
| `serde` / `serde_json` | Current workspace-compatible | Canonical artifact/evidence schemas and report serialization | Always, for deterministic audit outputs |
| `petgraph` | Current stable | Requirement↔evidence graph representation and traversal | Needed when matrix dependency reasoning grows beyond flat joins |
| `time` | 0.3.x | Snapshot timestamps, waiver expiry, freshness windows | Required for repeatable evidence and gate timing |
| `uuid` | Current stable | Stable audit-run IDs and evidence references | Recommended for multi-run traceability and audit lineage |
| `jsonschema` (Rust crate) | Current stable | Validation of machine-readable audit output format | Use when external tooling consumes generated JSON |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo fmt`, `clippy`, `test` | Rust quality gates | Already in repo scripts; keep audit modules on same gates |
| `tsc`, `eslint` | UI/client quality gates | Reuse existing operator-console checks for audit UI surfaces |
| `gh` + workflow required checks | CI gate integration | Use generated audit status as explicit deploy signal |
| `jq` | Local report inspection/debug | Useful for verifying machine-readable audit artifacts quickly |
| `sqlx migrate` | Audit schema evolution | Keep audit tables under dedicated migration files and review path |

## Installation

```bash
# Core runtime (already present in repo baseline)
cargo add axum sqlx serde serde_json time uuid

# Markdown + schema support (audit layer)
cargo add comrak petgraph jsonschema

# Optional local tooling
sudo apt-get install -y jq
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Rust audit service integrated into existing workspace | Standalone Python audit service | Use only if team has no Rust bandwidth and accepts cross-runtime ops cost |
| SQLx + PostgreSQL snapshots | Flat markdown/CSV-only output | Acceptable for one-off manual reviews, not for repeatable release gates |
| Axum API + Next.js UI integration | External BI/dashboard tooling | Use if organization already mandates a centralized BI platform |
| GitHub Actions check-run gating | Manual release checklist | Use only in very small projects where release frequency is low |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Purely manual spreadsheet mapping | Becomes stale quickly and is not reproducible in CI | Deterministic audit generation with committed artifacts |
| Single aggregate “coverage score” without risk bands | Hides deployment-critical missing controls | Weighted readiness model + explicit critical gap list |
| Runtime-path invasive audit hooks | Increases risk in latency-sensitive execution/risk flows | Out-of-band audit orchestration with read-only integrations |
| Opaque AI-only pass/fail decisions | Weak audit defensibility and hard to trust | Human-reviewed classifier outputs with rationale fields |

## Stack Patterns by Variant

**If the milestone is audit-only (current milestone):**
- Keep implementation read-heavy and non-invasive (artifact parsing + evidence linking + reporting)
- Prioritize reproducibility and explainability over runtime coupling

**If later milestones include automated remediation:**
- Add issue/ticket integration adapters
- Add policy profile management and controlled gate escalation paths

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `axum 0.8.x` | `tokio 1.48.x`, `tower-http 0.6.x` | Align with current service stack for shared middleware patterns |
| `sqlx 0.8.x` | Postgres 15+, Rust 1.91.x | Matches existing persistence foundation |
| Next.js 16.x | React 19.x, TypeScript 5.x | Keep parity with operator-console baseline |
| `comrak` (stable) | Rust 1.91.x | Confirm crate MSRV before locking |

## Sources

- `.planning/PROJECT.md` — Project scope, constraints, and milestone goals
- `.planning/codebase/STACK.md` — Existing runtime/framework/dependency baseline
- `.planning/codebase/ARCHITECTURE.md` — Existing service boundaries and integration points
- `.planning/research/FEATURES.md` — Expected audit capabilities and dependency relationships
- `.planning/research/ARCHITECTURE.md` — Suggested audit component model and data flow
- `.planning/research/PITFALLS.md` — Common failure modes that influence stack choices

---
*Stack research for: BMAD traceability + deployment-readiness audit*
*Researched: 2026-04-09*
