---
phase: 02
slug: evidence-traceability-mapping
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 02 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness + Node built-in test runner |
| **Config file** | none (command-driven scripts) |
| **Quick run command** | `cargo test -p research-gateway traceability:: && node --test tests/api/phase-2-traceability.test.mjs -t` |
| **Full suite command** | `cargo test -p research-gateway traceability:: && node --test tests/api/phase-2-traceability.test.mjs tests/e2e/phase-2-traceability-mapping.e2e.test.mjs` |
| **Estimated runtime** | ~180 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p research-gateway traceability::` plus relevant `node --test` phase-2 file.
- **After every plan wave:** Run `cargo test -p research-gateway traceability:: && node --test tests/api/phase-2-traceability.test.mjs tests/e2e/phase-2-traceability-mapping.e2e.test.mjs`.
- **Before `/gsd-verify-work`:** Full suite must be green.
- **Max feedback latency:** 180 seconds.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | TRAC-01 | T-02-01 | Deterministic mapping to code evidence; no silent winner selection | unit + API | `cargo test -p research-gateway traceability::tests::maps_requirement_to_code_evidence` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | TRAC-02 | T-02-02 | Test evidence links recorded when automated assertions exist | unit + API | `cargo test -p research-gateway traceability::tests::maps_requirement_to_test_evidence` | ❌ W0 | ⬜ pending |
| 02-01-03 | 01 | 1 | TRAC-03 | T-02-03 | Rationale/confidence mandatory and machine-readable | unit + API + e2e | `cargo test -p domain traceability::tests::rejects_empty_rationale` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/domain/src/traceability.rs` — contract tests for rationale/confidence validation
- [ ] `crates/persistence/src/postgres/traceability.rs` — adapter + migration contract tests
- [ ] `services/research-gateway/src/traceability/service.rs` — deterministic/fallback/ambiguity tests
- [ ] `tests/api/phase-2-traceability.test.mjs` — API behavior checks
- [ ] `tests/e2e/phase-2-traceability-mapping.e2e.test.mjs` — end-to-end determinism checks
- [ ] `package.json` script `qa:test:phase-2` to run full phase verification suite

---

## Manual-Only Verifications

All phase behaviors are expected to be automated. No manual-only checks planned.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all missing references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
