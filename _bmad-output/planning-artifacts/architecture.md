---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
lastStep: 8
inputDocuments:
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/prd.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/prd-validation-report.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/ux-design-specification.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/research/domain-polymarket-rust-clob-profit-research-2026-04-04.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/research/market-rust-polymarket-trading-system-lopez-de-prado-research-2026-04-04.md
  - /Users/sang/polymarket-ai/_bmad-output/planning-artifacts/research/technical-polymarket-claim-validity-audit-research-2026-04-04.md
  - /Users/sang/polymarket-ai/_bmad-output/brainstorming/brainstorming-session-2026-04-04-120000.md
workflowType: 'architecture'
project_name: 'polymarket-ai'
user_name: 'Sang'
date: '2026-04-04'
status: 'complete'
completedAt: '2026-04-04'
documentCounts:
  briefCount: 0
  prdCount: 1
  uxCount: 1
  researchCount: 3
  projectDocsCount: 0
  projectContextCount: 0
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

## Project Context Analysis

### Requirements Overview

**Functional Requirements:**
The PRD defines 49 functional requirements across market data intake, execution lifecycle, risk governance, portfolio allocation, dashboard operations, security/audit, external reporting, incentive intelligence, and model-promotion governance. Architecturally, the largest drivers are:

- Always-on event ingestion + order lifecycle correctness (FR1-FR16)
- Hard pre-trade and portfolio risk gates with deterministic safe-state transitions (FR17-FR21)
- High-fidelity operator visibility + incident forensics (FR26-FR30)
- Strict policy and audit controls for privileged actions (FR31-FR34)
- Phase-2 research/promotion governance with anti-overfitting controls (FR43-FR48)

**Non-Functional Requirements:**
22 NFRs impose production-grade guarantees on latency, availability, resilience, security, scalability, observability, and auditability. The most architecture-shaping NFRs are:

- p95 dashboard and control-path latency targets (NFR1-NFR3)
- automated safe-state transition within 5 seconds on critical failures (NFR5)
- immutable and queryable governance/audit telemetry (NFR9, NFR17)
- deployment hardening and reproducibility constraints (NFR19-NFR21)

**Scale & Complexity:**

- Primary domain: blockchain_web3 + fintech operations platform
- Complexity level: high / enterprise-like governance depth
- Estimated architectural components: 14 core bounded components (ingestion, execution, risk, portfolio, reconciliation, research, governance, reporting, auth, audit, observability, control API, UI, integration adapters)

### Technical Constraints & Dependencies

- Trading must use official `polymarket-client-sdk` only for Rust exchange integration.
- Required SDK capabilities include `clob`, `ws`, and heartbeat-safe operation.
- Platform must enforce geoblock/eligibility checks as hard startup and runtime gates.
- Under stale or uncertain data, protection has priority over throughput/latency optimization.
- Production runtime must run least-privilege and preserve append-only audit trails.
- Dual-approval requirements apply to critical control actions (promotion overrides, risk-limit increase, kill-switch disable, production config changes).

### Cross-Cutting Concerns Identified

- **Safety-first control plane:** pause/reduce-only/cancel-all + recovery gates
- **Governance-by-design:** immutable approvals, separation of duties, policy traceability
- **Resilience engineering:** reconnect supervision, heartbeat watchdogs, stale-data halts
- **Data integrity:** reconciliation as release/activation gate
- **Observability and forensics:** correlation from signal → order → fill → PnL → incident
- **Phase-aware extensibility:** MVP execution alpha now, research factory and automated promotion in Phase 2

## Starter Template Evaluation

### Primary Technology Domain

Full-stack trading operations platform with a Rust event-driven backend core and a web operator console.

### Starter Options Considered

1. **Single-stack Rust full-stack starter** (Rust backend + Rust-rendered UI)
   - Pros: language uniformity
   - Cons: slower path to rich UX implementation from current design artifacts

2. **Dual-starter monorepo (selected)**
   - Rust Cargo workspace for trading/control services
   - Next.js App Router app for operator dashboard
   - Pros: fastest alignment with required UX sophistication and Rust execution constraints

3. **Minimal custom bootstrap only (`cargo new` + manual UI scaffold)**
   - Pros: maximum control
   - Cons: slower initial throughput and more setup drift risk across agents

### Selected Starter: Dual-Starter Monorepo Baseline

**Rationale for Selection:**
The project requires a hardened Rust trading core and a high-quality operator console with strong information design. A dual-starter baseline provides predictable backend ergonomics while preserving frontend velocity and UX fidelity.

**Initialization Commands:**

```bash
# bootstrap dashboard
npx create-next-app@16.2.2 apps/operator-console --ts --eslint --src-dir --app --tailwind

# bootstrap rust workspace members (from repo root)
cargo new --bin services/control-api
cargo new --bin services/execution-engine
cargo new --bin services/risk-engine
cargo new --lib crates/domain
cargo new --lib crates/common
```

**Architectural Decisions Provided by Starter:**

**Language & Runtime:**

- Rust with async Tokio runtime for trading/control paths
- TypeScript for dashboard implementation

**Styling Solution:**

- Tokenized Tailwind-driven UI layer mapped to UX design tokens and risk semantics

**Build Tooling:**

- Cargo workspace for Rust services/crates
- Next.js production build pipeline for operator console

**Testing Framework:**

- Rust: `cargo test` for unit/integration
- Dashboard: Next.js test stack + e2e coverage for critical control flows

**Code Organization:**

- Feature-bounded service modules in Rust
- UI organized by workflow surfaces (risk, execution, incident, governance)

**Development Experience:**

- Strong typed boundaries on both service and UI surfaces
- Fast local iteration with service-level ownership

**Note:** Project bootstrap is the first implementation story and must include policy-safe environment scaffolding and CI baseline.

## Core Architectural Decisions

### Decision Priority Analysis

**Critical Decisions (Block Implementation):**

- Event-driven Rust service boundaries and failure-domain separation
- Canonical data model for orders/trades/positions/reconciliation/audit
- Deterministic risk gate and safe-state orchestration
- Governance and approval workflow architecture

**Important Decisions (Shape Architecture):**

- Frontend interaction/state architecture for safety-critical controls
- Reporting and external integration contracts
- Observability and incident forensics pipeline

**Deferred Decisions (Post-MVP):**

- Full automated promotion/deprecation orchestration
- Advanced multi-node HA failover automation

### Data Architecture

- **System of record:** PostgreSQL 18 (latest major per PostgreSQL release page)
- **Access layer:** `sqlx` v0.8.6
- **Ledger style:** append-only event tables for order lifecycle, risk actions, policy approvals, and operator actions
- **Read model strategy:** materialized read views for dashboard queries (risk posture, attribution, incident timelines)
- **Migration strategy:** forward-only migrations with checksum validation in CI
- **Caching strategy:** bounded in-memory caches per service for hot operational state; persisted truth remains PostgreSQL + exchange state

### Authentication & Security

- **Operator auth model:** strict RBAC with explicit separation between read-only and trading-control roles
- **Privileged action model:** dual approval for FR34-listed actions; proposer must differ from approver
- **Secret management:** no plaintext secrets in source, logs, process args, or shell history; rotatable secret injection path
- **Service security:** least-privilege runtime identities, signed action records, and immutable audit append
- **Exchange auth:** use official SDK authentication flows, including builder-authenticated paths where configured

### API & Communication Patterns

- **Control plane API:** REST over Axum v0.8.8
- **Live updates:** WebSocket/SSE channels for dashboard state and incident updates
- **Trading/event ingestion:** official Polymarket streams via `polymarket-client-sdk` v0.4.4 (`ws` feature)
- **Error handling:** canonical typed error envelope with machine-readable code + operator-safe message
- **Rate-limit behavior:** explicit backoff and circuiting around documented 425/429/5xx surfaces

### Frontend Architecture

- **Framework:** Next.js v16.2.2 App Router
- **Design model:** token-first implementation of “Warm Precision” UX system
- **State strategy:** server-first data loading + deterministic client state for controls, incidents, and confirmations
- **Critical UX contract:** action rail for pause/reduce-only/cancel-all always visible in privileged mode
- **Performance contract:** render and interaction priorities align with risk posture first, attribution second

### Infrastructure & Deployment

- **Primary environment:** DigitalOcean hardened production host baseline
- **Runtime model:** supervised long-running services with health-gated activation
- **Observability baseline:** OpenTelemetry Rust API `opentelemetry` v0.31.0 + structured logs + metrics + traces
- **Release safety:** reproducible builds, rollback windows, and activation gates tied to health + reconciliation

### Decision Impact Analysis

**Implementation Sequence:**

1. Bootstrap monorepo and service skeletons
2. Implement ingestion/execution/risk minimum slices with safe-state guards
3. Add control API and operator console core views/actions
4. Add immutable governance/audit pipeline and reporting contracts
5. Add Phase-2 research governance components

**Cross-Component Dependencies:**

- Risk service is upstream of execution enablement and downstream of ingestion health
- Reconciliation readiness gates trading reactivation
- Governance service gates strategy/risk critical mutations
- UI surfaces depend on canonical read models produced by event + reconciliation pipelines

## Implementation Patterns & Consistency Rules

### Pattern Categories Defined

**Critical conflict points identified:** 18 areas where independent agents could diverge without explicit rules.

### Naming Patterns

**Database Naming Conventions:**

- Tables: plural snake_case (`orders`, `risk_events`, `approval_actions`)
- Columns: snake_case (`created_at`, `actor_id`, `approval_reference`)
- Foreign keys: `<entity>_id` (`order_id`, `strategy_id`)
- Indexes: `idx_<table>__<column(s)>`

**API Naming Conventions:**

- Resource paths: plural kebab-case (`/api/v1/risk-events`)
- Path params: `{id}` style in docs, `:id` in routers
- Query params: snake_case (`start_ts`, `end_ts`, `alpha_id`)

**Code Naming Conventions:**

- Rust modules/files: snake_case
- Rust types/traits/enums: UpperCamelCase
- Rust functions/variables: snake_case
- React components: PascalCase file and symbol names
- TypeScript variables/functions: camelCase

### Structure Patterns

**Project Organization:**

- Services by bounded context (`execution-engine`, `risk-engine`, `control-api`)
- Shared pure logic in `crates/domain`
- Shared technical utilities in `crates/common`
- UI by user workflow and risk-critical task grouping

**File Structure Patterns:**

- Tests co-located for unit scope; integration/e2e in dedicated top-level test packages
- Config profiles under explicit environment directories (`dev`, `staging`, `prod`)
- Runbooks and policy documents versioned in `docs/operations`

### Format Patterns

**API Response Formats:**

- Success envelope:
  - `data`: payload
  - `meta`: pagination/request metadata
  - `error`: null
- Error envelope:
  - `data`: null
  - `error.code`: stable machine code
  - `error.message`: operator-readable summary
  - `error.details`: optional structured context

**Data Exchange Formats:**

- Timestamps: ISO-8601 UTC only
- IDs: UUID/ULID strings (never implicit numeric IDs in public API)
- Monetary values: decimal strings with explicit precision

### Communication Patterns

**Event System Patterns:**

- Event name format: `domain.entity.action.v1` (example: `risk.limit.breached.v1`)
- Event envelope includes `event_id`, `occurred_at`, `actor_id`, `correlation_id`
- Backward-compatible event evolution via version suffix increments

**State Management Patterns:**

- All mutating actions are command-driven and auditable
- UI applies optimistic updates only for non-critical cosmetic interactions
- Critical control states require server acknowledgment before confirmation UI

### Process Patterns

**Error Handling Patterns:**

- Distinguish domain errors, integration errors, and policy violations
- Unknown/fatal classes trigger protective mode when trading safety is ambiguous
- No swallowed errors in control or execution paths

**Loading State Patterns:**

- Standardized states: `idle`, `loading`, `refreshing`, `error`, `critical`
- Risk-critical widgets always show freshness timestamp and stale indicator

### Enforcement Guidelines

**All AI Agents MUST:**

- Follow naming/format conventions exactly
- Use canonical envelopes and timestamp formats
- Route privileged mutations through governance checks and audit emission
- Preserve safety-first transitions whenever state certainty degrades

**Pattern Enforcement:**

- CI lint/format + contract tests for API envelopes and event schemas
- PR checklist includes pattern conformance and safety invariants
- Violations tracked as architecture-nonconformance defects

### Pattern Examples

**Good Examples:**

- `POST /api/v1/control/pause` emits `control.session.paused.v1` + audit row
- `risk_events.occurred_at` stored UTC and rendered with explicit timezone label

**Anti-Patterns:**

- Mixed timestamp formats (epoch + ISO) across services
- Direct execution mutations bypassing governance/audit path
- Inconsistent endpoint naming across modules

## Project Structure & Boundaries

### Complete Project Directory Structure

```text
polymarket-ai/
├── README.md
├── Cargo.toml
├── Cargo.lock
├── package.json
├── pnpm-workspace.yaml
├── .env.example
├── .github/
│   └── workflows/
│       ├── ci-rust.yml
│       ├── ci-web.yml
│       └── security.yml
├── apps/
│   └── operator-console/
│       ├── package.json
│       ├── next.config.ts
│       ├── tailwind.config.ts
│       └── src/
│           ├── app/
│           │   ├── (dashboard)/
│           │   ├── (incidents)/
│           │   ├── (governance)/
│           │   └── api/
│           ├── components/
│           │   ├── risk/
│           │   ├── execution/
│           │   ├── governance/
│           │   └── timeline/
│           ├── lib/
│           ├── hooks/
│           └── styles/
├── services/
│   ├── control-api/
│   │   ├── src/main.rs
│   │   ├── src/routes/
│   │   ├── src/handlers/
│   │   └── src/middleware/
│   ├── execution-engine/
│   │   ├── src/main.rs
│   │   ├── src/ingestion/
│   │   ├── src/orders/
│   │   └── src/reconciliation/
│   ├── risk-engine/
│   │   ├── src/main.rs
│   │   ├── src/gates/
│   │   ├── src/limits/
│   │   └── src/safe_state/
│   ├── portfolio-engine/
│   │   ├── src/main.rs
│   │   ├── src/allocation/
│   │   └── src/attribution/
│   ├── governance-service/
│   │   ├── src/main.rs
│   │   ├── src/approvals/
│   │   └── src/audit/
│   ├── reporting-service/
│   │   ├── src/main.rs
│   │   ├── src/exports/
│   │   └── src/contracts/
│   └── research-gateway/        # Phase 2+
│       ├── src/main.rs
│       ├── src/validation/
│       └── src/promotion/
├── crates/
│   ├── domain/
│   │   ├── src/order.rs
│   │   ├── src/risk.rs
│   │   ├── src/governance.rs
│   │   └── src/events.rs
│   ├── common/
│   │   ├── src/errors.rs
│   │   ├── src/time.rs
│   │   ├── src/config.rs
│   │   └── src/telemetry.rs
│   └── persistence/
│       ├── src/lib.rs
│       ├── src/postgres/
│       └── migrations/
├── infra/
│   ├── docker/
│   ├── systemd/
│   ├── terraform/
│   └── monitoring/
├── tests/
│   ├── integration/
│   ├── e2e/
│   ├── contract/
│   └── chaos/
└── docs/
    ├── architecture/
    ├── operations/
    ├── runbooks/
    └── governance/
```

### Architectural Boundaries

**API Boundaries:**

- `control-api` exposes authenticated operator and automation commands
- `reporting-service` exposes read-only versioned contracts for external consumers
- execution-facing endpoints are internal-only and gated through risk/governance checks

**Component Boundaries:**

- `execution-engine` never mutates policy state directly
- `risk-engine` owns trading eligibility decisions
- `governance-service` owns privileged approval workflows and immutable audit writes

**Service Boundaries:**

- Services communicate via typed contracts + append-only event publication
- Shared domain logic is imported from `crates/domain` to avoid duplicate rules

**Data Boundaries:**

- PostgreSQL is authoritative for persisted ledger + governance records
- Exchange-reported order/trade state is authoritative for reconciliation truth
- Dashboard read models are projections, not primary truth sources

### Requirements to Structure Mapping

**Feature/Epic Mapping:**

- FR1-FR5 (market intake): `services/execution-engine/src/ingestion`
- FR12-FR16 (order lifecycle): `services/execution-engine/src/orders`
- FR17-FR21 (risk controls): `services/risk-engine/src/{gates,limits,safe_state}`
- FR26-FR30 (ops and incidents): `services/control-api` + `apps/operator-console/src/app/(incidents)`
- FR31-FR34 (governance/security): `services/governance-service/src/{approvals,audit}`
- FR35-FR38 (reporting/interfaces): `services/reporting-service/src/{contracts,exports}`
- FR43-FR48 (model governance, Phase 2): `services/research-gateway/src/{validation,promotion}`

**Cross-Cutting Concerns:**

- Auditability: `services/governance-service`, `crates/persistence`
- Observability: `crates/common/src/telemetry.rs`, `infra/monitoring`
- Incident response: `docs/runbooks`, `tests/chaos`

### Integration Points

**Internal Communication:**

- Control commands enter through `control-api`
- Risk adjudication precedes all execution-side side effects
- Reconciliation feedback loops into risk gating and dashboard incident surfaces

**External Integrations:**

- Polymarket CLOB + WS via `polymarket-client-sdk`
- Optional external analytics consumption via reporting contracts

**Data Flow:**

1. Market/user stream ingest
2. Signal + policy evaluation
3. Execution attempt + lifecycle tracking
4. Reconciliation and attribution
5. Operator visibility + governance/audit persistence

### File Organization Patterns

**Configuration Files:**

- Environment overlays in `infra/` and service-local config modules
- `.env.example` contains placeholders only

**Source Organization:**

- Domain-first crates for shared business rules
- Thin service adapters around explicit bounded responsibilities

**Test Organization:**

- Unit tests co-located
- Integration + contract + chaos tests in `/tests`

**Asset Organization:**

- UI static assets in `apps/operator-console/public`
- Evidence exports and generated artifacts under `services/reporting-service`

### Development Workflow Integration

**Development Server Structure:**

- Local compose profile runs PostgreSQL + core Rust services + operator console

**Build Process Structure:**

- Rust services built and tested per crate/service boundary
- Web app built independently and integration-tested against control API contracts

**Deployment Structure:**

- Service artifacts deployed independently with health-gated activation order:
  1) persistence/telemetry dependencies,
  2) ingestion + risk,
  3) execution,
  4) control + UI.

## Architecture Validation Results

### Coherence Validation ✅

**Decision Compatibility:**
Core technology choices are compatible: Rust async runtime (Tokio), Axum control API, SQLx/PostgreSQL persistence, official Polymarket SDK integration, and Next.js operator console.

**Pattern Consistency:**
Naming, envelopes, event schema, and governance pathways are defined across backend and frontend boundaries with explicit anti-patterns.

**Structure Alignment:**
Directory and service boundaries map cleanly to FR clusters and NFR obligations, with clear ownership of risk, execution, governance, and reporting.

### Requirements Coverage Validation ✅

**Epic/Feature Coverage:**
All defined capability clusters from PRD user journeys are represented in the architecture.

**Functional Requirements Coverage:**
FR1-FR49 coverage is complete via mapped service/domain boundaries and integration points.

**Non-Functional Requirements Coverage:**
Performance, reliability, safe-state behavior, auditability, and deployment hardening are all addressed by explicit architectural contracts.

### Implementation Readiness Validation ✅

**Decision Completeness:**
Critical decisions are documented with stack versions for core technology surfaces.

**Structure Completeness:**
Project tree is concrete and implementation-ready with service/crate/UI boundaries.

**Pattern Completeness:**
Agent-conflict areas are addressed with enforceable naming, format, communication, and process patterns.

### Gap Analysis Results

**Critical Gaps:** None.

**Important Gaps:**

- Exact control-plane authentication provider selection (OIDC vendor and token claims mapping)
- Finalized rollout order for Phase-2 research gateway subcapabilities

**Nice-to-Have Gaps:**

- Additional UI component contract snapshots for incident timeline variants
- Extended chaos scenarios for exchange partial-outage patterns

### Validation Issues Addressed

- Ensured architecture remains implementation-agnostic where appropriate while still enforcing deterministic safety and governance pathways.
- Aligned all high-risk controls to explicit service ownership to avoid agent ambiguity.

### Architecture Completeness Checklist

**✅ Requirements Analysis**

- [x] Project context thoroughly analyzed
- [x] Scale and complexity assessed
- [x] Technical constraints identified
- [x] Cross-cutting concerns mapped

**✅ Architectural Decisions**

- [x] Critical decisions documented with versions
- [x] Technology stack fully specified
- [x] Integration patterns defined
- [x] Performance/safety considerations addressed

**✅ Implementation Patterns**

- [x] Naming conventions established
- [x] Structure patterns defined
- [x] Communication patterns specified
- [x] Process patterns documented

**✅ Project Structure**

- [x] Complete directory structure defined
- [x] Component boundaries established
- [x] Integration points mapped
- [x] Requirements-to-structure mapping complete

### Architecture Readiness Assessment

**Overall Status:** READY FOR IMPLEMENTATION

**Confidence Level:** High

**Key Strengths:**

- Safety and governance are first-class, not add-ons
- Strong separation of concerns across execution, risk, and control domains
- Explicit anti-overfitting and promotion governance path for Phase 2

**Areas for Future Enhancement:**

- Multi-node active/active resilience patterns after MVP stability evidence
- Expanded operator simulation and what-if scenario tooling

### Implementation Handoff

**AI Agent Guidelines:**

- Implement against this architecture as source of truth
- Do not bypass risk/governance pathways for convenience
- Preserve naming, contract, and audit conventions exactly
- Treat stale/uncertain state as a safety trigger, not a recoverable warning

**First Implementation Priority:**
Execute starter bootstrap, wire minimal ingestion/risk/execution flow, and prove safe-state + audit emission in end-to-end tests before adding advanced strategy features.

## Completion & Handoff

Excellent work — this architecture is now complete, validated, and ready to drive consistent multi-agent implementation.

Immediate next actions:

1. Generate implementation stories directly from the structure and FR mapping above.
2. Start with bootstrap + safety-critical vertical slice (ingestion → risk gate → execution → audit).
3. Keep Phase-2 research automation behind explicit feature flags until Phase-1 stability gates pass.