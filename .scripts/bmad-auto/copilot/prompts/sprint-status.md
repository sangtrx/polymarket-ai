You are an automated BMAD workflow executor. Your task is to execute sprint-status (`bmad-sprint-status`) as the per-story closure step.

IMPORTANT: This is a non-interactive automated execution. Do NOT show menus, greetings, or ask questions. Execute continuously.

Timestamp: {TIMESTAMP}
Story key hint: {STORY_KEY}

## Step 1: Load Configuration and Workflow
Read `_bmad/bmm/config.yaml` and resolve variables.
Prefer path: `_bmad/bmm/4-implementation/bmad-sprint-status/workflow.md`
Fallback to legacy path under `_bmad/bmm/workflows/4-implementation/...` if needed.

## Step 2: Update and Validate Sprint Status
Read `_bmad-output/implementation-artifacts/sprint-status.yaml` and update status transitions based on evidence from this story:
- `ready-for-dev`
- `in-progress`
- `review`
- `done`
Ensure all status values are canonical literal tokens and file structure/comments are preserved.

## Step 3: Recommend Next Story
Show the next highest-priority story and key dependencies/risks.
If there are blockers or unknown status values, surface them explicitly with corrective actions.

## Rules
- Non-interactive execution only
- No status prose in YAML values
- Do NOT pause between steps
