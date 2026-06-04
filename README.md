# polymarket-ai

<p align="center">
	<img src="./banner.png" alt="polymarket-ai banner" style="max-width:100%; height:auto;" />
</p>

A monorepo providing the polymarket-ai baseline: a Rust backend ecosystem with web frontends (Next.js) and supporting services. This repository contains server crates, web apps, tooling, infrastructure manifests and CI helpers for local development and CI.

Badges: [CI] [Docs] [License] (replace with your project badges)

## Table of contents

- [Quick overview](#quick-overview)
- [Prerequisites](#prerequisites)
- [Getting started (local development)](#getting-started-local-development)
- [Common commands](#common-commands)
- [Repository layout](#repository-layout)
- [Contributing](#contributing)
- [Architecture notes](#architecture-notes)
- [License](#license)

## Quick overview

This repo is organized as a polyglot monorepo combining:

- Rust crates (core services, domain logic, persistence)
- Node/Next.js apps (operator consoles, web UIs)
- Multiple backend services (see `services/`)
- CI, infra and dev tooling under `infra/`, `tools/`, and root scripts

The goal is to make it straightforward for a contributor or maintainer to boot the toolchain, run checks, and iterate on code locally.

## Prerequisites

- Node.js >= 20.9.0
- pnpm >= 10 (workspace-aware)
- Rust toolchain (pinned via `rust-toolchain.toml`)
- Cargo (for building/running Rust crates)

Make sure the toolchain versions above are available on your PATH. Use your platform's package manager or rustup / nvm / fnm to manage versions.

## Getting started (local development)

1. Install workspace dependencies

```bash
pnpm install --frozen-lockfile
```

2. Run the repository bootstrap checks (verifies toolchain & policies)

```bash
npm run bootstrap:verify
```

3. Run CI-equivalent checks locally (examples)

```bash
npm run ci:rust    # Rust tests / linters
npm run ci:web     # Web app linting / typecheck
npm run ci:security
```

4. Develop services or apps

- Rust crate (example)

```bash
cd crates/common
cargo build
cargo test
```

- Web app (example)

```bash
cd apps/operator-console
pnpm install
pnpm dev
```

Notes: exact dev commands for each app/service live in that package's README. When in doubt, open the specific folder under `apps/`, `crates/` or `services/` and follow its local README.

## Common commands

- Bootstrap checks: `npm run bootstrap:verify`
- CI checks locally: `npm run ci:rust`, `npm run ci:web`, `npm run ci:security`
- Run tests: `cargo test` inside a crate, or the test runner for a given app

Add more granular scripts in the relevant package when adding new services or apps.

## Repository layout

- `apps/` — frontend and full-stack applications (Next.js, operator consoles)
- `crates/` — Rust crates grouped by domain (common, persistence, domain, etc.)
- `services/` — independent backend services and APIs
- `infra/` — Docker, Terraform, and deployment support
- `tests/` — integration, e2e, and contract tests
- `_bmad/` and `_bmad-output/` — internal planning and generated artifacts

## Contributing

We welcome contributions. A good pull request includes:

- A clear description of the change and why it is needed
- Tests or a description of how the change was tested locally
- Any required migration or configuration notes

Please follow the coding conventions and run the relevant linters and tests before opening a PR. If your change touches public APIs, add a note to the changelog or release notes.

## Architecture notes

High-level:

- Services strive for least-privilege runtime defaults (non-root user, localhost binds) — see repo helper scripts and environment templates.
- Secrets are not stored in the repo. Use `.env.example` as a template; inject real values via secret management in production.

For more detailed architecture documentation, check `docs/architecture/` and the planning artifacts under `_bmad-output/implementation-artifacts/`.

## License

See `LICENSE` in the repository root for license terms.

## Maintainers / Contact

If you need help, open an issue or reach out to the maintainers listed in the repository metadata.

---

If you'd like, I can:

- Add repository badges (CI, coverage, license) with exact URLs
- Create or update per-package READMEs under `apps/` and `crates/`
- Run a lightweight reader-test pass and iterate the outline if you want a more targeted README (e.g., contributor-focused or user-focused)

