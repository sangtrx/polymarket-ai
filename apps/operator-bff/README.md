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

Public process checks are `GET /healthz` and `GET /readyz`. Process health is independent of dependencies. Readiness fails closed unless the auth secret is configured **and** the PostgreSQL workflow table is reachable. An in-memory store remains available for tests/development but is reported as non-durable and cannot make `/readyz` return 200.

## Ownership boundary

The BFF owns only operator-facing workflow request metadata, idempotency, status, and audit-friendly result/error envelopes. Upstream Rust services remain the source of truth for domain state and decisions. An unavailable upstream becomes a failed BFF workflow; the BFF must never synthesize success or bypass Rust authorization/risk gates.

`PostgresWorkflowStore` persists orchestration state in `operator_workflow_requests`. Its idempotency key is the database unique constraint `(operator_id, workflow_kind, idempotency_key)`, and state transitions use compare-and-set semantics on the expected status. `migrations/001_operator_workflow_requests.sql` is the schema contract; apply it before starting a durable runtime.

The operator console interactive proof keeps credentials server-side. `POST /api/operator-workflows/readiness` mints a 60-second BFF JWT from server-only runtime configuration, and `/api/operator-workflows/:id/events` proxies the authenticated SSE stream. The browser receives only workflow identifiers/status/result data; it never receives `OPERATOR_BFF_AUTH_SECRET` or a bearer token. The mutation route additionally requires same-origin browser requests. This source slice assumes access to the operator console itself is separately restricted; production identity/session integration remains outside SAN-279.

## Commands

From the repository root after workspace dependencies are installed:

```bash
pnpm --filter operator-bff typecheck
pnpm --filter operator-bff test
pnpm --filter operator-bff build
```

The PostgreSQL integration smoke is intentionally destructive only against a database literally named `operator_bff_test`:

```bash
OPERATOR_BFF_TEST_DATABASE_URL='postgresql://operator_bff:<password>@127.0.0.1:5432/operator_bff_test' \
  pnpm --filter operator-bff test:postgres
```

CI provisions that dedicated database as a service container, applies `migrations/001_operator_workflow_requests.sql`, runs a real durable workflow, and verifies database-level idempotency.

## Reproducible local container path

No production deployment is authorized or implied. For a local proof, provide a non-production 32+ character secret and start the bounded BFF/PostgreSQL stack:

```bash
OPERATOR_BFF_AUTH_SECRET='local-only-secret-at-least-32-characters' \
CONTROL_API_BASE_URL='http://host.docker.internal:8080' \
docker compose -f compose.operator-bff.yml up --build
```

The image defaults remain fail-closed. Container execution sets `OPERATOR_BFF_BIND_ADDRESS=0.0.0.0` only inside the isolated compose network, publishes port `8090` to host loopback only, initializes the workflow migration in a dedicated PostgreSQL volume, and waits on durable `/readyz`. Remove the local volume before re-testing a changed bootstrap migration (`docker compose -f compose.operator-bff.yml down -v`).

For a direct non-container local run:

```bash
OPERATOR_BFF_AUTH_SECRET='<32+ char local test secret>' \
OPERATOR_BFF_DATABASE_URL='postgres://<local-user>:<local-password>@127.0.0.1:5432/<local-db>' \
CONTROL_API_BASE_URL='http://127.0.0.1:8080' \
node apps/operator-bff/dist/main.js
```

Do not commit real secrets. Production deployment, live trading mutations, and operator credential issuance are outside this Track A source slice and require separate authorization.
