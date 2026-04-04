# polymarket-ai

Story 1.1 bootstrap baseline for the Rust + Next.js monorepo.

## Prerequisites

- Node.js `>=20.9.0`
- pnpm `>=10 <11`
- Rust toolchain pinned by `rust-toolchain.toml`

## Bootstrap verification commands

```bash
npm run bootstrap:verify
npm run bootstrap:verify:negative
```

`bootstrap:verify` performs:

1. Toolchain preflight checks.
2. Template/policy checks (no plaintext secrets, required placeholders, reproducible entrypoint hash).
3. Machine-readable failure output with `error_code`, `failed_check`, `remediation_hint`, and `timestamp_utc`.

`ci:security` extends this with lockfile validation plus source/workflow scans for private-key literals and secret-like CLI argument leakage.

## Local CI-equivalent commands

```bash
npm run ci:rust
npm run ci:web
npm run ci:security
```

## Least-privilege runtime defaults

- Services default to non-root (`RUNTIME_RUN_AS_NON_ROOT=true`) and localhost bind (`CONTROL_API_BIND_ADDRESS=127.0.0.1`).
- Secrets are placeholder-only in `.env.example`; inject real values only at runtime through approved secret management.
