---
status: complete
phase: 04-deployment-risk-prioritization
source:
  - 04-deployment-risk-prioritization-01-SUMMARY.md
  - 04-deployment-risk-prioritization-02-SUMMARY.md
  - 04-deployment-risk-prioritization-03-SUMMARY.md
started: 2026-04-09T18:47:41Z
updated: 2026-04-09T18:47:41Z
---

## Current Test

[testing complete]

## Tests

### 1. Cold Start Smoke Test
expected: Running `prioritize-risk` from a fresh temporary workspace completes without runtime errors and returns non-empty risk rows.
result: pass
checks: Verified by `npm run -s qa:test:phase-4` API/E2E fixture execution (`RISK-01`, `RISK-02`) that runs `cargo run -q -p research-gateway -- prioritize-risk ...`.

### 2. Covered Requirements Are Excluded from Ranked Output
expected: Ranked rows exclude coverage_class `covered` while unresolved rows remain with severity, reason_code, risk_score, and priority_rank.
result: pass
checks: Verified by `RISK-01: prioritize-risk excludes covered rows...` test in `tests/api/phase-4-risk-prioritization.test.mjs`.

### 3. Unknown Reason Codes Fail Closed to High Severity
expected: Unknown `reason_code` maps to high severity with machine code `risk_weight_unmapped_reason`.
result: pass
checks: Verified by `risk::tests::guardrails_unknown_reason_still_maps_to_high_severity` and Node API fallback test in the phase-4 QA run.

### 4. Risk Ordering Is Deterministic and Comparator-Stable
expected: Ordering is severity desc, risk_score desc, then canonical_requirement_id asc, producing byte-equivalent output for identical input.
result: pass
checks: Verified by `risk::tests::ranking_sorts_by_severity_desc_then_risk_score_desc_then_canonical_requirement_id_asc`, `guardrails_identical_input_returns_byte_equivalent_json_order`, and `RISK-02` E2E test.

### 5. Priority Ranks Are Contiguous and 1-Based
expected: Output rows assign contiguous `priority_rank` values starting from 1 with no gaps.
result: pass
checks: Verified by `risk::tests::ranking_assigns_contiguous_one_based_priority_ranks` and `RISK-02` E2E assertions.

### 6. CLI Contract Is JSON-Only and Fail-Closed
expected: `prioritize-risk` emits JSON payload fields (`priority_rank`, `risk_score`, `severity`, identifiers) and returns machine-readable error codes for invalid payloads.
result: pass
checks: Verified by `main::tests::prioritize_risk_command_returns_json_payload_with_priority_rank_and_risk_score`, `prioritize_risk_command_fails_closed_with_machine_readable_code`, and `prioritize_risk_command_requires_commit_sha`.

### 7. Risk Snapshot Persistence Is Immutable with Typed Error Mapping
expected: Persistence schema rejects mutation operations and maps persistence failures to typed machine codes.
result: pass
checks: Verified by persistence tests `migration_contract_enforces_immutable_snapshot_tables` and `query_errors_map_to_machine_readable_codes`.

### 8. Phase QA Aggregator Executes Required Rust and Node Verification
expected: `qa:test:phase-4` runs the required persistence, risk-domain/service, CLI, API, and E2E verification checks successfully.
result: pass
checks: Verified by successful unattended run of `npm run -s qa:test:phase-4`.

## Summary

total: 8
passed: 8
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none]

## Unresolved Items

- none (all checkpoints were resolved via automated artifact checks in unattended mode)
