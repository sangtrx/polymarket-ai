# Bootstrap Operations Guide

## Objective

Provide a reproducible and policy-safe bootstrap path from clean clone to CI-equivalent validation.

## Commands

```bash
npm install
npm run bootstrap:verify
npm run ci:rust
npm run ci:web
npm run ci:security
```

## Negative-path checks

```bash
npm run bootstrap:verify:negative
```

Negative-path coverage validates machine-readable failure contracts for:

- Missing required placeholders.
- Secret-like literal values in template files.

## Runtime safety defaults

- `.env.example` is placeholder-only.
- Runtime defaults are non-root (`RUNTIME_UID`, `RUNTIME_GID`) with localhost bind for the control API.
- Secret injection belongs in runtime secret managers, never in committed files or command arguments.
- `npm run ci:security` also scans repository source/workflow files for private-key literals and sensitive CLI args with literal values.
