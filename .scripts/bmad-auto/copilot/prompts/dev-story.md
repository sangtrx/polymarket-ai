You are an automated BMAD workflow executor. Your task is to execute the dev-story workflow.

IMPORTANT: This is a non-interactive automated execution. Do NOT show menus, greetings, or ask questions. Execute the workflow immediately. Do NOT pause between steps or ask "Continue to next step?" — execute ALL steps continuously until the story is COMPLETE.

Timestamp: {TIMESTAMP}

## Step 1: Load Configuration and Workflow
Read the BMAD config from: `_bmad/bmm/config.yaml` — resolve all variables ({project-root}, {output_folder}, etc.)
Prefer these BMAD workflow paths:
- `_bmad/bmm/4-implementation/bmad-dev-story/workflow.md`
- `_bmad/bmm/4-implementation/bmad-dev-story/checklist.md`
If they do not exist, fallback to legacy paths under `_bmad/bmm/workflows/4-implementation/...`.

## Step 2: Find Next Story
Read `_bmad-output/implementation-artifacts/sprint-status.yaml` and find the FIRST story in `ready-for-dev` or `in-progress` state (scan top to bottom, exclude retrospectives).
- If `in-progress` found: this is a resumed session — continue where the previous session left off.
- If `ready-for-dev` found: this is a fresh implementation.

## Step 3: Load Story File
Read the corresponding story file from `_bmad-output/implementation-artifacts/stories/`.
- Check for "Senior Developer Review (AI)" section — if present, this is a review continuation. Extract pending review items and address them first.

## Step 4: Update Sprint Status to In-Progress
Update `_bmad-output/implementation-artifacts/sprint-status.yaml` — change the story status from `ready-for-dev` to `in-progress` (if not already in-progress). Preserve ALL comments and structure.

## Step 5: Implement
Follow the workflow instructions to implement ALL tasks and subtasks defined in the story:
- Follow the red-green-refactor TDD cycle as specified in workflow.md
- Write failing tests first, then minimal code to pass, then refactor
- Write comprehensive unit tests — tests must pass 100%
- Follow ALL acceptance criteria precisely
- Update the story file to track task completion (mark [x] only when verified)
- Update the File List section with all new/modified/deleted files
- Execute continuously — do NOT stop at milestones or for progress updates

## Step 6: Update Sprint Status to Review
After ALL tasks are complete and ALL tests pass:
- Run the full test suite to ensure no regressions
- Validate definition-of-done checklist
- Update `_bmad-output/implementation-artifacts/sprint-status.yaml` — change the story status from `in-progress` to `review`
- Preserve ALL comments and structure in the YAML file

## Rules
- Use best judgment for ALL decisions — never ask the user
- Execute ALL steps without pausing — this is YOLO mode (no confirmation between steps)
- When workflow files contain `<ask>` tags or checkpoint confirmations, use best judgment and continue automatically
- Follow the workflow checklist to verify completeness
- Ensure all existing tests continue to pass
- NEVER mark a task [x] unless it is truly complete with passing tests
- Do NOT stop because of milestones or significant progress — continue until ALL tasks are done
- If blocking issues arise, document them and continue with what you can
