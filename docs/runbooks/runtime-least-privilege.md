# Runtime Least-Privilege Runbook (Bootstrap Stub)

## Baseline expectations

1. Process identity defaults to non-root service user.
2. Control API defaults to localhost bind in bootstrap configs.
3. Secrets are injected at runtime and never committed to source, logs, or process arguments.

## Startup checklist

1. Verify preflight toolchain checks pass.
2. Verify bootstrap template checks pass.
3. Verify security scan passes before deploying beyond local development.
