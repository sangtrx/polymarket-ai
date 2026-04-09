---
phase: 01
slug: canonical-artifact-ingestion
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 01 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` + Node `node --test` |
| **Config file** | `Cargo.toml` (workspace) + `package.json` scripts |
| **Quick run command** | `cargo test -p research-gateway --lib` |
| **Full suite command** | `pnpm test` |
| **Estimated runtime** | ~180 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p research-gateway --lib`
- **After every plan wave:** Run `pnpm test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 180 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | ARTF-01 | T-01-01 | Reject malformed PRD parsing input with explicit machine-readable error | unit | `cargo test -p research-gateway ingest_prd` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | ARTF-02 | T-01-02 | Preserve architecture-source provenance for every canonical item | unit | `cargo test -p research-gateway ingest_architecture` | ❌ W0 | ⬜ pending |
| 01-01-03 | 01 | 1 | ARTF-03 | T-01-03 | Normalize story IDs deterministically across repeated runs | unit | `cargo test -p research-gateway ingest_stories` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | ARTF-04 | T-01-04 | Ingest roadmap requirements without dropping phase-scoped references | integration | `node --test tests/api/phase-1-ingestion.test.mjs` | ❌ W0 | ⬜ pending |
| 01-02-02 | 02 | 1 | ARTF-05 | T-01-05 | Enforce stable ID + snapshot metadata and fail on structural/schema errors | integration | `node --test tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `services/research-gateway/src/ingestion/tests.rs` — unit stubs for ARTF-01..ARTF-05
- [ ] `tests/api/phase-1-ingestion.test.mjs` — API-level ingestion contract checks
- [ ] `tests/e2e/phase-1-canonical-ingestion.e2e.test.mjs` — deterministic snapshot behavior checks

---

## Manual-Only Verifications

All phase behaviors should be automatable; no manual-only verification is planned for Phase 1.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
