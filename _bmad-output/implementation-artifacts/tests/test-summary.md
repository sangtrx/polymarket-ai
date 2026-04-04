# Test Automation Summary

## Story
- 1-1-set-up-initial-project-from-starter-template

## Generated Tests

### API Tests
- [x] `tests/api/bootstrap-cli-api.test.mjs` - Verifies bootstrap check CLI success/failure contracts (exit behavior + machine-readable payload shape).

### E2E Tests
- [x] `tests/e2e/bootstrap-baseline.e2e.test.mjs` - Verifies baseline bootstrap evidence generation flow and secret-leakage blocker behavior.

## Coverage
- API contracts covered: 2/2 critical check-bootstrap CLI flows (success summary, failure envelope).
- E2E workflows covered: 2/2 critical bootstrap-security flows (evidence hash chaining, secret leak rejection).
- Story-critical controls covered: machine-readable failure fields, deterministic entrypoint hash propagation, and retained evidence payload generation.

## Execution Result
- `npm run qa:test:story-1-1` ✅
- `npm run bootstrap:test` ✅
- `npm run ci:rust && npm run ci:web && npm run ci:security` ✅
- `npm test` ✅
