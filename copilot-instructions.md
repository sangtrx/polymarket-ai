<!-- GSD:project-start source:PROJECT.md -->
## Project

**Polymarket-AI BMAD Coverage Audit**

This project defines a brownfield audit initiative for `polymarket-ai` to verify and operationalize implementation coverage against BMAD artifacts. The immediate roadmap builds ingestion, mapping, and reporting capabilities that produce code-linked evidence and deployment-readiness signals. This work is for a solo maintainer who needs a reliable go/no-go decision before deployment.

**Core Value:** Establish deployment confidence by producing an evidence-based coverage audit of BMAD intent versus implemented code.

### Constraints

- **Scope**: Audit workflow implementation is allowed by roadmap phases; production deployment actions remain deferred
- **Traceability**: Must include file-level evidence for every BMAD item assessed
- **Coverage Baseline**: Comparison must include PRD, architecture, stories, and roadmap artifacts (not a subset)
- **Decision Utility**: Output must prioritize gaps by deployment impact, not by document order
<!-- GSD:project-end -->

<!-- GSD:stack-start source:codebase/STACK.md -->
## Technology Stack

## Languages
- Rust 1.91.0 (toolchain pinned) - backend services and shared crates in `services/*` and `crates/*` via `Cargo.toml` and `rust-toolchain.toml`
- TypeScript 5.x - Next.js operator UI in `apps/operator-console/src/**` (`apps/operator-console/package.json`, `apps/operator-console/tsconfig.json`)
- JavaScript (Node ESM) - repo tooling and bootstrap/security scripts in `tools/bootstrap/*.mjs`, root scripts in `package.json`
## Runtime
- Rust async runtime: Tokio (`tokio = "1.48.0"`) in workspace deps at `Cargo.toml`
- Node.js `>=20.9.0` for repo and web workflows in `package.json`
- CI uses Node 22 and Rust 1.91.0 in `.github/workflows/ci-rust.yml`, `.github/workflows/ci-web.yml`, `.github/workflows/security.yml`
- JS package manager: pnpm `10.10.0` (`packageManager` in `package.json`, setup in `.github/workflows/*.yml`)
- Rust package manager/build: Cargo (`Cargo.toml`)
- Lockfile: present (`pnpm-lock.yaml`, `Cargo.lock`)
## Frameworks
- Axum `0.8.8` for HTTP APIs in `services/control-api/src/main.rs`, `services/reporting-service/src/api.rs`, workspace pin in `Cargo.toml`
- Next.js `16.2.2` with React `19.2.4` for operator console in `apps/operator-console/package.json`
- SQLx `0.8.6` with Postgres and `runtime-tokio-rustls` in workspace deps (`Cargo.toml`), used across `crates/persistence/src/postgres/*.rs`
- Rust built-in test harness via `cargo test` commands in `package.json`
- Node built-in test runner (`node --test`) in root scripts in `package.json`
- Cargo fmt/clippy/build/test via `rust:*` scripts in `package.json`
- ESLint 9 + `eslint-config-next` in `apps/operator-console/eslint.config.mjs`
- TypeScript `tsc --noEmit` in `apps/operator-console/package.json`
- Next build/dev via `next build` and `next dev` in `apps/operator-console/package.json`
## Key Dependencies
- `polymarket-client-sdk = 0.4.4` (CLOB + WS features) for market/user stream ingestion in `services/execution-engine/src/ingestion/mod.rs` and `services/execution-engine/src/ingestion/user_stream.rs`
- `sqlx = 0.8.6` for durable state and governance persistence in `services/*/src/main.rs` and `crates/persistence/src/postgres/*.rs`
- `axum = 0.8.8` for control/reporting HTTP surfaces in `services/control-api/src/routes/mod.rs` and `services/reporting-service/src/api.rs`
- `opentelemetry = 0.31.0` declared in workspace and `crates/common/Cargo.toml` (current direct usage is minimal in `crates/common/src/telemetry.rs`)
- `time = 0.3.44` for RFC3339 timestamps and scheduling logic across `services/*` and `crates/domain/*`
- Frontend runtime deps `next`, `react`, `react-dom` in `apps/operator-console/package.json`
## Configuration
- Runtime config is environment-variable driven in Rust binaries (`services/control-api/src/main.rs`, `services/execution-engine/src/main.rs`, `services/risk-engine/src/main.rs`, `services/reporting-service/src/main.rs`)
- Frontend API base URL is environment-variable driven in `apps/operator-console/src/lib/env.ts`
- `.env`, `.env.example`, and `tests/fixtures/valid/.env.example` exist but are not read here; verify runtime values there and in deployment secret stores
- Workspace configuration in `Cargo.toml` and `pnpm-workspace.yaml`
- Rust toolchain pin in `rust-toolchain.toml`
- Next/Turbopack config in `apps/operator-console/next.config.ts`
- TypeScript and lint config in `apps/operator-console/tsconfig.json`, `apps/operator-console/eslint.config.mjs`
## Platform Requirements
- Install Rust 1.91.0 with `clippy` and `rustfmt` (`rust-toolchain.toml`)
- Install Node `>=20.9.0` and pnpm `>=10 <11` (`package.json`)
- Provide Postgres connectivity through `DATABASE_URL` for durable service paths (`services/control-api/src/main.rs`, `services/execution-engine/src/main.rs`)
- Backend services are designed as Rust processes with Postgres dependency (`services/*/src/main.rs`, `crates/persistence/src/postgres/*.rs`)
- Operator UI deploy target is a Next.js app (`apps/operator-console/package.json`); concrete hosting platform is not detected in repo config and should be verified from deployment environment
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

## Naming Patterns
- Use `PascalCase.tsx` for React components in `apps/operator-console/src/components/**` (example: `apps/operator-console/src/components/risk/RiskCommandSurface.tsx`).
- Use `kebab-case.ts` for frontend utility modules in `apps/operator-console/src/lib/**` (example: `apps/operator-console/src/lib/risk/control-actions.ts`).
- Use `snake_case.rs` module files and `mod.rs` directories in Rust services/crates (examples: `services/governance-service/src/risk_limits/mod.rs`, `crates/domain/src/risk.rs`).
- Use `*.test.mjs` and `*.e2e.test.mjs` for Node test suites under `tests/**` (examples: `tests/api/story-3-2-safety-control-api.test.mjs`, `tests/e2e/story-3-2-risk-safety-rail.e2e.test.mjs`).
- Use `camelCase` in TypeScript/JavaScript (examples: `getOperatorConsoleEnv` in `apps/operator-console/src/lib/env.ts`, `resolveRiskPostureViewModel` in `apps/operator-console/src/lib/risk/posture.ts`).
- Use `snake_case` in Rust (examples: `validate_risk_limit_profile_version` in `crates/domain/src/risk.rs`, `risk_limit_service_error_response` in `services/control-api/src/routes/mod.rs`).
- Use descriptive test names as behavior sentences (examples: `critical_increase_without_approval_reference_returns_pending` in `services/governance-service/src/risk_limits/mod.rs`, `Story 3.2 API surfaces machine-readable errors with no success-shaped fallback` in `tests/api/story-3-2-safety-control-api.test.mjs`).
- Use `SCREAMING_SNAKE_CASE` constants for both Rust and TS (examples: `EMERGENCY_CONTROL_ACK_MAX_SECONDS` in `crates/domain/src/risk.rs`, `RECOVERY_READINESS_ENDPOINT` in `apps/operator-console/src/lib/risk/control-actions.ts`).
- Use `camelCase` locals/props in TS and JS (examples: `activePosture` in `apps/operator-console/src/components/risk/RiskCommandSurface.tsx`, `compiledOutputDir` in `tests/api/story-3-2-safety-control-api.test.mjs`).
- Use `PascalCase` for Rust structs/enums and TS types/interfaces (examples: `RiskLimitServiceError` in `services/governance-service/src/risk_limits/mod.rs`, `RecoveryReadinessDecision` in `apps/operator-console/src/lib/risk/control-actions.ts`).
## Code Style
- Rust formatting is enforced with `cargo fmt --all -- --check` via `rust:fmt` in `package.json`.
- Frontend formatting is lint-driven; no Prettier config was detected in repo root or `apps/operator-console/` (verify by checking for `.prettierrc*` and `prettier` scripts in `package.json` and `apps/operator-console/package.json`).
- TypeScript style aligns with ESLint + Next defaults from `apps/operator-console/eslint.config.mjs`.
- Rust linting uses `cargo clippy --workspace --all-targets -- -D warnings` via `rust:lint` in `package.json` (warnings are treated as errors).
- Web linting uses `eslint . --max-warnings=0` in `apps/operator-console/package.json`.
- ESLint extends Next core web vitals and TypeScript presets in `apps/operator-console/eslint.config.mjs`.
## Import Organization
- Use `@/*` alias for operator-console source imports, configured in `apps/operator-console/tsconfig.json`.
## Error Handling
- Use structured, machine-readable error payloads with `code`/`error_code`, `message`, and optional field-level issues (examples: `RiskLimitServiceError` in `services/governance-service/src/risk_limits/mod.rs`; `RiskLimitServiceErrorResponse` returned by `risk_limit_service_error_response` in `services/control-api/src/routes/mod.rs`).
- Map domain/service error codes to HTTP status explicitly with per-domain status mapper functions (example: `risk_limit_service_error_status` in `services/control-api/src/routes/mod.rs`).
- Use custom typed error classes on frontend API clients and throw early on contract validation failures (example: `EmergencyControlClientError` in `apps/operator-console/src/lib/risk/control-actions.ts`).
- Use `expect_err` / `assert.rejects` in tests to enforce negative-path contracts (examples: `services/governance-service/src/risk_limits/mod.rs`, `tests/api/story-3-2-safety-control-api.test.mjs`).
## Logging
- Emit structured telemetry events as serialized JSON in orchestrator modules (example: `emit_risk_limit_telemetry` in `services/governance-service/src/risk_limits/mod.rs`).
- Emit startup lifecycle logs in service entrypoints (example: `println!("control-api bootstrap ready...")` in `services/control-api/src/main.rs`).
- Frontend modules rely on explicit error surfaces instead of console logging (example: `apps/operator-console/src/lib/risk/control-actions.ts`).
## Comments
- Keep comments sparse and intent-focused; most code is self-descriptive through naming.
- Use comments mainly in config/override contexts (example: ignore override comments in `apps/operator-console/eslint.config.mjs`).
- Not detected in sampled TS/JS source (`apps/operator-console/src/**`) and test files (`tests/**`).
- Verify if new public APIs need docs in future modules; current baseline does not enforce docblock conventions.
## Function Design
- Prefer typed input structs/interfaces over many positional parameters (examples: `UpsertRiskLimitProfileInput` in `services/governance-service/src/risk_limits/mod.rs`, `RecoveryReadinessRequestInput` in `apps/operator-console/src/lib/risk/control-actions.ts`).
- Use `#[allow(clippy::too_many_arguments)]` only for constructor/wiring boundaries (example: `with_all_orchestrators*` in `services/control-api/src/middleware/mod.rs`).
- Return `Result<T, E>` in Rust service/domain layers and propagate typed errors (examples across `crates/domain/src/*.rs`, `services/governance-service/src/risk_limits/mod.rs`).
- Return typed DTOs or throw typed errors in TS API adapters (example: `invokeEmergencyControlAction` and `EmergencyControlClientError` in `apps/operator-console/src/lib/risk/control-actions.ts`).
## Module Design
- Rust modules expose domain/service surfaces through `pub mod` in crate roots (examples: `crates/domain/src/lib.rs`, `services/governance-service/src/lib.rs`).
- TS frontend favors named exports for shared logic and component functions, with Next route modules using `export default` page handlers (example: `apps/operator-console/src/app/(dashboard)/dashboard/page.tsx`).
- Rust crate roots act as explicit module barrels via `lib.rs`.
- TypeScript barrel `index.ts` patterns were not detected in `apps/operator-console/src/**`; import from concrete module paths directly.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

## Pattern Overview
- Keep pure contracts and decision logic in `crates/domain/src/*.rs`, then orchestrate use-cases in `services/*/src/**/mod.rs`.
- Use trait ports for boundaries (for example `ApprovalRepositoryPort` in `services/governance-service/src/approvals/mod.rs` and `MarketStreamStore` in `services/execution-engine/src/ingestion/mod.rs`).
- Bind PostgreSQL adapters in composition roots (`services/control-api/src/main.rs`, `services/risk-engine/src/main.rs`, `services/execution-engine/src/main.rs`) and pass them as `Arc<dyn ...>`.
## Layers
- Purpose: Own canonical entities, enums, validation, and reason-code contracts.
- Location: `crates/domain/src/`
- Contains: Modules like `risk.rs`, `governance.rs`, `reporting.rs`, `research.rs`, `allocation.rs`.
- Depends on: Primarily `serde` and standard library; no service-level dependencies detected.
- Used by: All service crates and persistence adapters.
- Purpose: Translate domain records to SQL and back.
- Location: `crates/persistence/src/postgres/`
- Contains: Per-capability modules such as `approvals.rs`, `market_stream.rs`, `risk_limits.rs`, `validation_runs.rs`.
- Depends on: `sqlx`, `domain`, `common`.
- Used by: Service orchestrators via repository/store ports in `services/*`.
- Purpose: Implement business workflows behind port traits.
- Location: `services/governance-service/src/**`, `services/reporting-service/src/**`, `services/research-gateway/src/**`, `services/risk-engine/src/**`, `services/execution-engine/src/**`, `services/portfolio-engine/src/**`.
- Contains: `*Service` structs, `*Orchestrator` traits, input/evidence DTOs, telemetry emission.
- Depends on: `domain` contracts and `persistence` adapters.
- Used by: `services/control-api` routes and service `main.rs` composition roots.
- Purpose: Expose HTTP control surface and reporting endpoints.
- Location: `services/control-api/src/routes/mod.rs`, `services/reporting-service/src/api.rs`.
- Contains: `axum::Router` construction, request parsing, auth middleware integration.
- Depends on: Orchestrator traits in service crates and middleware state in `services/control-api/src/middleware/mod.rs`.
- Used by: Runtime process entry points (`services/control-api/src/main.rs`, `services/reporting-service/src/main.rs`).
- Purpose: Operator shell and workflow surfaces.
- Location: `apps/operator-console/src/app/**`, `apps/operator-console/src/components/**`, `apps/operator-console/src/lib/**`.
- Contains: Next.js App Router pages, client components, typed API clients to `/control/*`.
- Depends on: Browser/server `fetch`, URL-derived shell read-model state, control-api contracts.
- Used by: Operator workflows in dashboard/governance/incidents routes.
## Data Flow
- Use fail-closed in-memory runtime state for live gating (`InMemoryRuntimePolicyState` in `services/risk-engine/src/gates/mod.rs`, `InMemoryRiskLimitState` in `services/risk-engine/src/limits/mod.rs`, `SharedFreshnessSignals` in `services/execution-engine/src/ingestion/freshness_gate.rs`) and hydrate from PostgreSQL on startup.
- Use immutable evidence records persisted in PostgreSQL via `crates/persistence/src/postgres/*`.
## Key Abstractions
- Purpose: Isolate domain workflow from storage and external transports.
- Examples: `services/governance-service/src/approvals/mod.rs`, `services/research-gateway/src/validation/workflow_runs.rs`, `services/execution-engine/src/orders/mod.rs`.
- Pattern: `trait ...Port/Repository/Orchestrator` + `in_memory()` + `postgres(pool)` implementations.
- Purpose: Runtime dependency graph for all control endpoints.
- Examples: `services/control-api/src/middleware/mod.rs` (`ControlApiState` and `with_*_orchestrator` builders), wired in `services/control-api/src/main.rs`.
- Pattern: Builder-style dependency injection using `Arc<dyn Trait>`.
- Purpose: Return machine-readable reason codes, correlation IDs, timestamps.
- Examples: `services/governance-service/src/recovery/mod.rs` (`RecoveryResumeExecutionEvidence`), `services/reporting-service/src/read_models/queries.rs` (`ReportingDatasetResponse`), `services/research-gateway/src/validation/gate_policies.rs`.
- Pattern: Service methods return typed evidence DTOs rather than primitive booleans.
## Entry Points
- Location: `services/control-api/src/main.rs`
- Triggers: Process startup.
- Responsibilities: Open `PgPool`, construct all orchestrators, assemble `ControlApiState`, build router through `routes::app_router`.
- Location: `services/execution-engine/src/main.rs`
- Triggers: Process startup.
- Responsibilities: Bootstrap market/user ingestion runtimes, attach freshness/order lifecycle seams, run async loops with fail-closed `tokio::select!`.
- Location: `services/risk-engine/src/main.rs`
- Triggers: Process startup.
- Responsibilities: Hydrate runtime policy/limit state from Postgres and env, evaluate bootstrap intent, enforce fail-closed containment defaults.
- Location: `apps/operator-console/src/app/layout.tsx` and route pages under `apps/operator-console/src/app/(dashboard|incidents|governance)/**`.
- Triggers: HTTP requests to Next.js app routes.
- Responsibilities: Compose shell layout, tabbed route content, and client components invoking control/reporting clients.
## Error Handling
- Create per-module errors (`ApprovalServiceError`, `MarketIngestionError`, `ReportingReadModelError`) and map lower-level errors (`map_persistence_error` style) in files like `services/governance-service/src/approvals/mod.rs` and `services/reporting-service/src/read_models/queries.rs`.
- Fail closed for runtime risk paths (explicit defaults and panic/deny behavior in `services/risk-engine/src/main.rs` and `services/execution-engine/src/main.rs`).
## Cross-Cutting Concerns
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

| Skill | Description | Path |
|-------|-------------|------|
| bmad-advanced-elicitation | 'Push the LLM to reconsider, refine, and improve its recent output. Use when user asks for deeper critique or mentions a known deeper critique method, e.g. socratic, first principles, pre-mortem, red team.' | `.github/skills/bmad-advanced-elicitation/SKILL.md` |
| bmad-agent-analyst | Strategic business analyst and requirements expert. Use when the user asks to talk to Mary or requests the business analyst. | `.github/skills/bmad-agent-analyst/SKILL.md` |
| bmad-agent-architect | System architect and technical design leader. Use when the user asks to talk to Winston or requests the architect. | `.github/skills/bmad-agent-architect/SKILL.md` |
| bmad-agent-dev | Senior software engineer for story execution and code implementation. Use when the user asks to talk to Amelia or requests the developer agent. | `.github/skills/bmad-agent-dev/SKILL.md` |
| bmad-agent-pm | Product manager for PRD creation and requirements discovery. Use when the user asks to talk to John or requests the product manager. | `.github/skills/bmad-agent-pm/SKILL.md` |
| bmad-agent-qa | QA engineer for test automation and coverage. Use when the user asks to talk to Quinn or requests the QA engineer. | `.github/skills/bmad-agent-qa/SKILL.md` |
| bmad-agent-quick-flow-solo-dev | Elite full-stack developer for rapid spec and implementation. Use when the user asks to talk to Barry or requests the quick flow solo dev. | `.github/skills/bmad-agent-quick-flow-solo-dev/SKILL.md` |
| bmad-agent-sm | Scrum master for sprint planning and story preparation. Use when the user asks to talk to Bob or requests the scrum master. | `.github/skills/bmad-agent-sm/SKILL.md` |
| bmad-agent-tech-writer | Technical documentation specialist and knowledge curator. Use when the user asks to talk to Paige or requests the tech writer. | `.github/skills/bmad-agent-tech-writer/SKILL.md` |
| bmad-agent-ux-designer | UX designer and UI specialist. Use when the user asks to talk to Sally or requests the UX designer. | `.github/skills/bmad-agent-ux-designer/SKILL.md` |
| bmad-brainstorming | 'Facilitate interactive brainstorming sessions using diverse creative techniques and ideation methods. Use when the user says help me brainstorm or help me ideate.' | `.github/skills/bmad-brainstorming/SKILL.md` |
| bmad-check-implementation-readiness | 'Validate PRD, UX, Architecture and Epics specs are complete. Use when the user says "check implementation readiness".' | `.github/skills/bmad-check-implementation-readiness/SKILL.md` |
| bmad-code-review | 'Review code changes adversarially using parallel review layers (Blind Hunter, Edge Case Hunter, Acceptance Auditor) with structured triage into actionable categories. Use when the user says "run code review" or "review this code"' | `.github/skills/bmad-code-review/SKILL.md` |
| bmad-correct-course | 'Manage significant changes during sprint execution. Use when the user says "correct course" or "propose sprint change"' | `.github/skills/bmad-correct-course/SKILL.md` |
| bmad-create-architecture | 'Create architecture solution design decisions for AI agent consistency. Use when the user says "lets create architecture" or "create technical architecture" or "create a solution design"' | `.github/skills/bmad-create-architecture/SKILL.md` |
| bmad-create-epics-and-stories | 'Break requirements into epics and user stories. Use when the user says "create the epics and stories list"' | `.github/skills/bmad-create-epics-and-stories/SKILL.md` |
| bmad-create-prd | 'Create a PRD from scratch. Use when the user says "lets create a product requirements document" or "I want to create a new PRD"' | `.github/skills/bmad-create-prd/SKILL.md` |
| bmad-create-story | 'Creates a dedicated story file with all the context the agent will need to implement it later. Use when the user says "create the next story" or "create story [story identifier]"' | `.github/skills/bmad-create-story/SKILL.md` |
| bmad-create-ux-design | 'Plan UX patterns and design specifications. Use when the user says "lets create UX design" or "create UX specifications" or "help me plan the UX"' | `.github/skills/bmad-create-ux-design/SKILL.md` |
| bmad-dev-story | 'Execute story implementation following a context filled story spec file. Use when the user says "dev this story [story file]" or "implement the next story in the sprint plan"' | `.github/skills/bmad-dev-story/SKILL.md` |
| bmad-distillator | Lossless LLM-optimized compression of source documents. Use when the user requests to 'distill documents' or 'create a distillate'. | `.github/skills/bmad-distillator/SKILL.md` |
| bmad-document-project | 'Document brownfield projects for AI context. Use when the user says "document this project" or "generate project docs"' | `.github/skills/bmad-document-project/SKILL.md` |
| bmad-domain-research | 'Conduct domain and industry research. Use when the user says wants to do domain research for a topic or industry' | `.github/skills/bmad-domain-research/SKILL.md` |
| bmad-edit-prd | 'Edit an existing PRD. Use when the user says "edit this PRD".' | `.github/skills/bmad-edit-prd/SKILL.md` |
| bmad-editorial-review-prose | 'Clinical copy-editor that reviews text for communication issues. Use when user says review for prose or improve the prose' | `.github/skills/bmad-editorial-review-prose/SKILL.md` |
| bmad-editorial-review-structure | 'Structural editor that proposes cuts, reorganization, and simplification while preserving comprehension. Use when user requests structural review or editorial review of structure' | `.github/skills/bmad-editorial-review-structure/SKILL.md` |
| bmad-generate-project-context | 'Create project-context.md with AI rules. Use when the user says "generate project context" or "create project context"' | `.github/skills/bmad-generate-project-context/SKILL.md` |
| bmad-help | 'Analyzes current state and user query to answer BMad questions or recommend the next skill(s) to use. Use when user asks for help, bmad help, what to do next, or what to start with in BMad.' | `.github/skills/bmad-help/SKILL.md` |
| bmad-index-docs | 'Generates or updates an index.md to reference all docs in the folder. Use if user requests to create or update an index of all files in a specific folder' | `.github/skills/bmad-index-docs/SKILL.md` |
| bmad-init | "Initialize BMad project configuration and load config variables. Use when any skill needs module-specific configuration values, or when setting up a new BMad project." | `.github/skills/bmad-init/SKILL.md` |
| bmad-market-research | 'Conduct market research on competition and customers. Use when the user says they need market research' | `.github/skills/bmad-market-research/SKILL.md` |
| bmad-party-mode | 'Orchestrates group discussions between all installed BMAD agents, enabling natural multi-agent conversations. Use when user requests party mode.' | `.github/skills/bmad-party-mode/SKILL.md` |
| bmad-product-brief | Create or update product briefs through guided or autonomous discovery. Use when the user requests to create or update a Product Brief. | `.github/skills/bmad-product-brief/SKILL.md` |
| bmad-qa-generate-e2e-tests | 'Generate end to end automated tests for existing features. Use when the user says "create qa automated tests for [feature]"' | `.github/skills/bmad-qa-generate-e2e-tests/SKILL.md` |
| bmad-quick-dev | 'Implements any user intent, requirement, story, bug fix or change request by producing clean working code artifacts that follow the project''s existing architecture, patterns and conventions. Use when the user wants to build, fix, tweak, refactor, add or modify any code, component or feature.' | `.github/skills/bmad-quick-dev/SKILL.md` |
| bmad-retrospective | 'Post-epic review to extract lessons and assess success. Use when the user says "run a retrospective" or "lets retro the epic [epic]"' | `.github/skills/bmad-retrospective/SKILL.md` |
| bmad-review-adversarial-general | 'Perform a Cynical Review and produce a findings report. Use when the user requests a critical review of something' | `.github/skills/bmad-review-adversarial-general/SKILL.md` |
| bmad-review-edge-case-hunter | 'Walk every branching path and boundary condition in content, report only unhandled edge cases. Orthogonal to adversarial review - method-driven not attitude-driven. Use when you need exhaustive edge-case analysis of code, specs, or diffs.' | `.github/skills/bmad-review-edge-case-hunter/SKILL.md` |
| bmad-shard-doc | 'Splits large markdown documents into smaller, organized files based on level 2 (default) sections. Use if the user says perform shard document' | `.github/skills/bmad-shard-doc/SKILL.md` |
| bmad-sprint-planning | 'Generate sprint status tracking from epics. Use when the user says "run sprint planning" or "generate sprint plan"' | `.github/skills/bmad-sprint-planning/SKILL.md` |
| bmad-sprint-status | 'Summarize sprint status and surface risks. Use when the user says "check sprint status" or "show sprint status"' | `.github/skills/bmad-sprint-status/SKILL.md` |
| bmad-technical-research | 'Conduct technical research on technologies and architecture. Use when the user says they would like to do or produce a technical research report' | `.github/skills/bmad-technical-research/SKILL.md` |
| bmad-validate-prd | 'Validate a PRD against standards. Use when the user says "validate this PRD" or "run PRD validation"' | `.github/skills/bmad-validate-prd/SKILL.md` |
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
