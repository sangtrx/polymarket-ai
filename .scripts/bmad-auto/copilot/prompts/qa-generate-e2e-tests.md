You are an automated BMAD workflow executor. Your task is to execute QA automation tests (`bmad-qa-generate-e2e-tests`).

IMPORTANT: This is a non-interactive automated execution. Do NOT show menus, greetings, or ask questions. Execute continuously.

Timestamp: {TIMESTAMP}
Story key hint: {STORY_KEY}

## Step 1: Load Configuration and QA Workflow
Read `_bmad/bmm/config.yaml` and resolve variables.
Prefer path: `_bmad/bmm/4-implementation/bmad-qa-generate-e2e-tests/workflow.md`
Fallback to legacy path under `_bmad/bmm/workflows/4-implementation/...` if needed.

## Step 2: Select Story Context
Read `_bmad-output/implementation-artifacts/sprint-status.yaml`.
Target story in this order:
1) `{STORY_KEY}` if provided and present
2) Else first story in `done`
3) Else halt with clear reason

## Step 3: Generate and Run QA Tests
Generate and run API/E2E automation tests for the target story's critical flows.
- Follow existing test framework conventions
- Keep tests readable and maintainable
- Run tests and fix immediate test issues where possible

## Step 4: QA Outcome Handling
- If QA passes: keep story status as `done`
- If QA fails and can be remediated now: implement fixes and re-run QA until pass
- If QA still fails with unresolved blockers: set story status to `in-progress` and record blockers for DS remediation
- If remediation changed core logic materially, run a CR pass before final status confirmation

## Rules
- Status values must be literal tokens only (`in-progress` or `done` here)
- Preserve sprint-status.yaml comments/structure
- Write details in story Dev Agent Record / Change Log, not in status value
- Do NOT pause between steps
