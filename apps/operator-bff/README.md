# Operator BFF (Track A proof)

`operator-bff` is a bounded TypeScript/Node orchestration layer for the existing Polymarket operator console. It does **not** own trading, risk, research, governance, or execution decisions; the Rust services remain authoritative for those domains.

## First vertical slice

The initial workflow is a read-only control-plane readiness snapshot:

1. authenticate an operator with an HS256 bearer JWT (`sub`, `role`, `exp`);
2. require `operator` or `admin` to start a workflow;
3. accept an `Idempotency-Key` and create one workflow per operator/kind/key;
4. move through explicit `queued -> running -> succeeded|failed` states;
5. probe the existing control API `GET /health` without mutating trading state;
6. expose current workflow state and server-sent events (SSE).

Public process checks are `GET /healthz` and `GET /readyz`. Readiness fails closed until `OPERATOR_BFF_AUTH_SECRET` contains at least 32 characters.

## Ownership boundary

The BFF owns only operator-facing workflow request metadata, idempotency, status, and audit-friendly result/error envelopes. Upstream Rust services remain the source of truth for domain state and decisions. An unavailable upstream becomes a failed BFF workflow; the BFF must never synthesize success or bypass Rust authorization/risk gates.

The current implementation intentionally starts with `InMemoryWorkflowStore` so the auth/state/SSE contract can be tested without changing dependency resolution. `migrations/001_operator_workflow_requests.sql` defines the durable PostgreSQL schema and uniqueness contract. The next accepted slice must add the PostgreSQL adapter and switch runtime readiness to require durable storage before this service can be described as production-ready.

## Commands

From the repository root after workspace dependencies are installed:

```bash
pnpm --filter operator-bff typecheck
pnpm --filter operator-bff test
pnpm --filter operator-bff build
```

Run the compiled service only with explicit local configuration; no deployment is implied by repository acceptance:

```bash
OPERATOR_BFF_AUTH_SECRET='<32+ char local test secret>' \
CONTROL_API_BASE_URL='http://127.0.0.1:8080' \
node apps/operator-bff/dist/server.js
```

Do not commit real secrets. Production deployment, live trading mutations, and operator credential issuance are outside this Track A source slice and require separate authorization.
