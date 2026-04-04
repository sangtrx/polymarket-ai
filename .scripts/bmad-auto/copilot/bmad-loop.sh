#!/bin/bash
# ============================================================
# BMAD Auto Loop - GitHub Copilot CLI Edition
# ============================================================
#
# DESCRIPTION:
#   Executes the BMAD workflow in a loop using GitHub Copilot CLI:
#     create-story (CS) → validate-story (VS) → dev-story (DS) → code-review (CR)
#     → qa-generate-e2e-tests (QA, optional) → sprint-status (SS) → commit
#   Each step runs as a NEW Copilot CLI session (fresh context window).
#   Continues until all stories reach 'done' state in sprint-status.yaml.
#
# USAGE:
#   ./bmad-loop.sh                      # Run with default 250 max iterations
#   ./bmad-loop.sh 100                  # Run with 100 max iterations
#   ./bmad-loop.sh 250 --verbose        # Run with verbose logging
#   ./bmad-loop.sh 250 --verbose --dry  # Dry run (show actions without executing)
#
# PREREQUISITES:
#   - GitHub Copilot CLI installed: npm install -g @github/copilot
#     or: brew install copilot-cli
#   - Authenticated: run `copilot` and use /login
#   - BMAD installed with .github/agents/ directory (sm, dev agents)
#   - Sprint planning completed (sprint-status.yaml exists)
#
# MODEL: GPT-5.3-Codex with xhigh reasoning (configurable via env vars)
#
# ============================================================

set -e  # Exit on error

# ============================================================
# ARGUMENT PARSING
# ============================================================

MAX_ITERATIONS=${1:-250}
VERBOSE=false
DRY_RUN=false

for arg in "$@"; do
    case $arg in
        --verbose|-v) VERBOSE=true ;;
        --dry|-d) DRY_RUN=true ;;
    esac
done

# ============================================================
# CONFIGURATION
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_PATH="$SCRIPT_DIR/bmad-auto-config.yaml"
PROGRESS_LOG="$SCRIPT_DIR/bmad-progress.log"
PROMPT_DIR="$SCRIPT_DIR/prompts"

# Model configuration - GPT-5.3-Codex + xhigh reasoning effort
COPILOT_MODEL="${COPILOT_MODEL:-gpt-5.3-codex}"
COPILOT_REASONING_EFFORT="${COPILOT_REASONING_EFFORT:-xhigh}"

# Delay between workflow steps/iterations (seconds)
# Minimum enforced delay is 60s to reduce rate-limit/ban risk.
ITERATION_DELAY="${ITERATION_DELAY:-60}"
if ! [[ "$ITERATION_DELAY" =~ ^[0-9]+$ ]] || [[ "$ITERATION_DELAY" -lt 60 ]]; then
    ITERATION_DELAY=60
fi

# Canonical loop controls
# VS gate runs after CS. QA and SS run after CR when story reaches done.
ENABLE_VALIDATE_GATE="${ENABLE_VALIDATE_GATE:-true}"
ENABLE_QA_STEP="${ENABLE_QA_STEP:-true}"
ENABLE_SPRINT_STATUS_STEP="${ENABLE_SPRINT_STATUS_STEP:-true}"

is_enabled() {
    local value
    value=$(echo "$1" | tr '[:upper:]' '[:lower:]')
    [[ "$value" == "true" || "$value" == "1" || "$value" == "yes" || "$value" == "y" ]]
}

# ============================================================
# CONFIGURATION LOADING
# ============================================================

if [[ ! -f "$CONFIG_PATH" ]]; then
    echo "[ERROR] Configuration file not found: $CONFIG_PATH"
    echo ""
    echo "Re-install: npx bmad-auto-copilot install"
    exit 1
fi

# Parse YAML config (simple key: value format)
PROJECT_ROOT=$(grep "^project_root:" "$CONFIG_PATH" | sed 's/project_root: *"\{0,1\}\([^"]*\)"\{0,1\}/\1/' | tr -d '\r')
IMPL_ARTIFACTS=$(grep "^implementation_artifacts:" "$CONFIG_PATH" | sed 's/implementation_artifacts: *"\{0,1\}\([^"]*\)"\{0,1\}/\1/' | tr -d '\r')

if [[ -z "$PROJECT_ROOT" ]]; then
    echo "[ERROR] 'project_root' not found in config file"
    exit 1
fi

if [[ -z "$IMPL_ARTIFACTS" ]]; then
    IMPL_ARTIFACTS="_bmad-output/implementation-artifacts"
fi

SPRINT_STATUS_PATH="$PROJECT_ROOT/$IMPL_ARTIFACTS/sprint-status.yaml"

# ============================================================
# PREREQUISITE CHECKS
# ============================================================

check_prerequisites() {
    # Check Copilot CLI
    if ! command -v copilot &> /dev/null; then
        echo "[ERROR] GitHub Copilot CLI not found in PATH"
        echo ""
        echo "Install Copilot CLI:"
        echo "  macOS:    brew install copilot-cli"
        echo "  npm:      npm install -g @github/copilot"
        echo "  script:   curl -fsSL https://gh.io/copilot-install | bash"
        echo ""
        echo "Then authenticate: copilot (and use /login)"
        exit 1
    fi

    # Check sprint-status.yaml
    if [[ ! -f "$SPRINT_STATUS_PATH" ]]; then
        echo "[ERROR] sprint-status.yaml not found: $SPRINT_STATUS_PATH"
        echo ""
        echo "Run sprint planning first:"
        echo "  copilot --agent sm -p 'Execute the sprint-planning workflow'"
        exit 1
    fi

    # Check .github/agents directory
    if [[ ! -d "$PROJECT_ROOT/.github/agents" ]]; then
        echo "[WARNING] .github/agents directory not found"
        echo "BMAD custom agents may not be available to Copilot CLI"
    fi

    # Check prompt files
    if [[ ! -d "$PROMPT_DIR" ]]; then
        echo "[ERROR] Prompts directory not found: $PROMPT_DIR"
        exit 1
    fi
}

# ============================================================
# LOGGING
# ============================================================

log() {
    local message="$1"
    local color="${2:-white}"
    local timestamp=$(date "+%Y-%m-%d %H:%M:%S")

    case $color in
        red)    color_code="\033[0;31m" ;;
        green)  color_code="\033[0;32m" ;;
        yellow) color_code="\033[0;33m" ;;
        cyan)   color_code="\033[0;36m" ;;
        gray)   color_code="\033[0;90m" ;;
        magenta) color_code="\033[0;35m" ;;
        *)      color_code="\033[0m" ;;
    esac

    echo -e "${color_code}${message}\033[0m"
    echo "[$timestamp] $message" >> "$PROGRESS_LOG"
}

# ============================================================
# GET NEXT ACTION
# ============================================================

get_next_action() {
    if [[ ! -f "$SPRINT_STATUS_PATH" ]]; then
        echo "[ERROR] sprint-status.yaml not found: $SPRINT_STATUS_PATH" >&2
        echo "error"
        return
    fi

    # Filter story lines (pattern: digits-digits-*) excluding retrospectives
    # Strip inline YAML comments (# ...) so status matching works with annotated lines
    local story_lines=$(grep -E '^[[:space:]]*[0-9]+-[0-9]+-' "$SPRINT_STATUS_PATH" | grep -v "retrospective" | sed 's/ *#.*//')

    if $VERBOSE; then
        local count=$(echo "$story_lines" | wc -l | tr -d ' ')
        echo "[DEBUG] Found $count story lines" >&2
    fi

    # Count stories in each state
    local review_count=$(echo "$story_lines" | grep -c ': *review *$' || true)
    local ready_count=$(echo "$story_lines" | grep -c ': *ready-for-dev *$' || true)
    local backlog_count=$(echo "$story_lines" | grep -c ': *backlog *$' || true)
    local done_count=$(echo "$story_lines" | grep -c ': *done *$' || true)
    local in_progress_count=$(echo "$story_lines" | grep -c ': *in-progress *$' || true)
    local total_count=$(echo "$story_lines" | grep -c '.' || true)

    if $VERBOSE; then
        echo "[DEBUG] Review: $review_count | Ready: $ready_count | Backlog: $backlog_count | In-Progress: $in_progress_count | Done: $done_count / $total_count" >&2
    fi

    # Priority order: review > in-progress > ready-for-dev > backlog
    if [[ $review_count -gt 0 ]]; then
        log "[NEXT] Found story in REVIEW state → code-review (Dev agent)" "green" >&2
        echo "code-review"
        return
    fi

    if [[ $in_progress_count -gt 0 ]]; then
        log "[NEXT] Found story in IN-PROGRESS state → dev-story resume (Dev agent)" "green" >&2
        echo "dev-story"
        return
    fi

    if [[ $ready_count -gt 0 ]]; then
        log "[NEXT] Found story in READY-FOR-DEV state → dev-story (Dev agent)" "green" >&2
        echo "dev-story"
        return
    fi

    if [[ $backlog_count -gt 0 ]]; then
        log "[NEXT] Found story in BACKLOG state → create-story (SM agent)" "green" >&2
        echo "create-story"
        return
    fi

    # Check if all done
    if [[ $done_count -eq $total_count && $total_count -gt 0 ]]; then
        echo "complete"
        return
    fi

    echo "wait"
}

# ============================================================
# INVOKE COPILOT CLI (NEW SESSION PER CALL)
# ============================================================

invoke_copilot() {
    local agent="$1"
    local prompt_file="$2"
    local action_label="$3"
    local story_key="${4:-}"

    if [[ ! -f "$prompt_file" ]]; then
        log "[ERROR] Prompt file not found: $prompt_file" "red"
        return 1
    fi

    # Read prompt and substitute timestamp
    local prompt=$(cat "$prompt_file")
    prompt="${prompt//\{TIMESTAMP\}/$(date "+%Y-%m-%d %H:%M:%S")}"
    prompt="${prompt//\{STORY_KEY\}/$story_key}"

    log "[SESSION] New Copilot CLI session → Model: $COPILOT_MODEL | Reasoning: $COPILOT_REASONING_EFFORT | Action: $action_label" "magenta"

    if $DRY_RUN; then
        log "[DRY RUN] Would execute: copilot --model $COPILOT_MODEL --reasoning-effort $COPILOT_REASONING_EFFORT -p '...' --allow-all --no-ask-user" "yellow"
        return 0
    fi

    # Execute Copilot CLI in programmatic mode (each -p call = new session)
    # NOTE: We do NOT use --agent to avoid the agent's menu/activation loop.
    # The prompt itself contains all workflow instructions.
    if copilot \
        --model "$COPILOT_MODEL" \
        --reasoning-effort "$COPILOT_REASONING_EFFORT" \
        --allow-all \
        --no-ask-user \
        -p "$prompt" \
        2>&1; then
        log "[SUCCESS] Session completed: $action_label" "green"
        return 0
    else
        local exit_code=$?
        log "[ERROR] Session failed (exit $exit_code): $action_label" "red"
        return 1
    fi
}

# ============================================================
# GET CURRENT STORY KEY
# ============================================================

get_story_key_for_state() {
    local state="$1"
    grep -E '^[[:space:]]*[0-9]+-[0-9]+-' "$SPRINT_STATUS_PATH" | grep -v "retrospective" | sed 's/ *#.*//' | grep ": *${state} *$" | head -1 | sed 's/^[[:space:]]*//' | cut -d: -f1
}

get_story_state_by_key() {
    local story_key="$1"

    if [[ -z "$story_key" ]]; then
        echo ""
        return
    fi

    grep -E "^[[:space:]]*${story_key}:" "$SPRINT_STATUS_PATH" \
        | head -1 \
        | sed 's/ *#.*//' \
        | sed 's/^[^:]*:[[:space:]]*//' \
        | sed 's/[[:space:]]*$//'
}

get_story_state_token() {
    local raw_state="$1"

    # Extract leading canonical token (letters/hyphen), normalize to lowercase.
    echo "$raw_state" | sed -E 's/^[[:space:]]*([A-Za-z-]+).*$/\1/' | tr '[:upper:]' '[:lower:]'
}

# ============================================================
# GIT COMMIT
# ============================================================

invoke_git_commit() {
    local story_key="${1:-unknown}"
    log "[GIT] Checking for changes to commit..." "yellow"

    if $DRY_RUN; then
        log "[DRY RUN] Would check git status and commit" "yellow"
        return 0
    fi

    cd "$PROJECT_ROOT"
    local status=$(git status --porcelain)

    if [[ -z "$status" ]]; then
        log "[INFO] No changes to commit" "gray"
        return 0
    fi

    local changed_files=$(echo "$status" | wc -l | tr -d ' ')
    log "[GIT] Committing $changed_files changed file(s)..." "yellow"

    if git add -A && git commit -m "feat(${story_key}): BMAD auto-complete [copilot-cli]"; then
        log "[SUCCESS] Changes committed successfully" "green"
        return 0
    else
        log "[ERROR] Git commit failed" "red"
        return 1
    fi
}

# ============================================================
# PRINT STATUS SUMMARY
# ============================================================

print_summary() {
    if [[ ! -f "$SPRINT_STATUS_PATH" ]]; then
        return
    fi

    local story_lines=$(grep -E '^[[:space:]]*[0-9]+-[0-9]+-' "$SPRINT_STATUS_PATH" | grep -v "retrospective" | sed 's/ *#.*//')
    local done=$(echo "$story_lines" | grep -c ': *done *$' || true)
    local total=$(echo "$story_lines" | grep -c '.' || true)
    local pct=0
    if [[ $total -gt 0 ]]; then
        pct=$((done * 100 / total))
    fi

    log "[PROGRESS] $done/$total stories done ($pct%)" "cyan"
}

# ============================================================
# MAIN LOOP
# ============================================================

check_prerequisites

log "" "white"
log "╔══════════════════════════════════════════════════════════╗" "cyan"
log "║    BMAD Auto Loop — GitHub Copilot CLI Edition          ║" "cyan"
log "║    Model: $COPILOT_MODEL                        ║" "cyan"
log "╚══════════════════════════════════════════════════════════╝" "cyan"
log "" "white"
log "[START] BMAD Auto Loop Started" "green"
log "Max iterations: $MAX_ITERATIONS" "gray"
log "Model: $COPILOT_MODEL" "gray"
log "Reasoning effort: $COPILOT_REASONING_EFFORT" "gray"
log "Delay between steps: ${ITERATION_DELAY}s" "gray"
log "Validate gate enabled: $ENABLE_VALIDATE_GATE" "gray"
log "QA step enabled: $ENABLE_QA_STEP" "gray"
log "Sprint-status step enabled: $ENABLE_SPRINT_STATUS_STEP" "gray"
log "Project root: $PROJECT_ROOT" "gray"
log "Sprint status: $SPRINT_STATUS_PATH" "gray"
if $DRY_RUN; then
    log "[DRY RUN MODE] No actions will be executed" "yellow"
fi
echo ""

print_summary
echo ""

for ((iteration=1; iteration<=MAX_ITERATIONS; iteration++)); do
    log "═══════════════════════════════════════════" "cyan"
    log "  Iteration $iteration / $MAX_ITERATIONS" "cyan"
    log "═══════════════════════════════════════════" "cyan"

    action=$(get_next_action)

    case $action in
        "create-story")
            log "[ACTION] CREATE STORY (SM Agent → new session)" "yellow"
            invoke_copilot "bmad-agent-bmm-sm" "$PROMPT_DIR/create-story.md" "create-story"

            if is_enabled "$ENABLE_VALIDATE_GATE"; then
                log "[ACTION] VALIDATE STORY READINESS (VS gate)" "yellow"
                if ! invoke_copilot "bmad-agent-bmm-sm" "$PROMPT_DIR/validate-story.md" "validate-story"; then
                    log "[WARN] Validate-story step failed; will retry canonical CS→VS cycle next iteration" "yellow"
                fi
            fi
            ;;
        "dev-story")
            log "[ACTION] DEVELOP STORY (Dev Agent → new session)" "yellow"
            invoke_copilot "bmad-agent-bmm-dev" "$PROMPT_DIR/dev-story.md" "dev-story"
            ;;
        "code-review")
            log "[ACTION] CODE REVIEW (Dev Agent → new session)" "yellow"
            # Capture story key before review (it's in 'review' state now)
            review_story_key=$(get_story_key_for_state "review")
            invoke_copilot "bmad-agent-bmm-dev" "$PROMPT_DIR/code-review.md" "code-review"

            # Only commit if the story actually moved to 'done'
            if [[ -z "$review_story_key" ]]; then
                log "[WARN] Could not determine review story key; skipping auto-commit this iteration" "yellow"
            else
                current_state=$(get_story_state_by_key "$review_story_key")
                current_state_token=$(get_story_state_token "$current_state")

                if [[ "$current_state_token" != "$current_state" ]]; then
                    log "[WARN] Non-canonical sprint status value for $review_story_key: '$current_state' (parsed token: '$current_state_token')" "yellow"
                fi

                if [[ "$current_state_token" == "done" ]]; then
                    log "[VERIFIED] Story $review_story_key confirmed DONE" "green"

                    qa_passed=true
                    if is_enabled "$ENABLE_QA_STEP"; then
                        log "[ACTION] QA GENERATE E2E TESTS (recommended)" "yellow"
                        if ! invoke_copilot "bmad-agent-bmm-qa" "$PROMPT_DIR/qa-generate-e2e-tests.md" "qa-generate-e2e-tests" "$review_story_key"; then
                            qa_passed=false
                            log "[WARN] QA step failed; skipping commit and continuing loop for remediation" "yellow"
                        fi
                    fi

                    if $qa_passed && is_enabled "$ENABLE_SPRINT_STATUS_STEP"; then
                        log "[ACTION] SPRINT STATUS UPDATE + NEXT STORY" "yellow"
                        if ! invoke_copilot "bmad-agent-bmm-sm" "$PROMPT_DIR/sprint-status.md" "sprint-status" "$review_story_key"; then
                            log "[WARN] Sprint-status step failed; continuing to commit because story is done" "yellow"
                        fi
                    fi

                    if $qa_passed; then
                        invoke_git_commit "$review_story_key"
                    fi
                else
                    log "[RETRY] Story $review_story_key still in '$current_state' (token: '$current_state_token', not done) — will retry next iteration" "yellow"
                fi
            fi
            ;;
        "complete")
            log "" "white"
            log "╔══════════════════════════════════════════════════════════╗" "green"
            log "║    🎉  ALL STORIES COMPLETED!                           ║" "green"
            log "╚══════════════════════════════════════════════════════════╝" "green"
            log "" "white"
            log "Sprint status: All stories are DONE" "green"
            log "Total iterations used: $iteration" "gray"
            print_summary
            exit 0
            ;;
        "wait")
            log "[WAIT] No actionable stories (may be in-progress)" "yellow"
            log "Skipping this iteration..." "gray"
            ;;
        "error")
            log "[ERROR] Error state — stopping loop" "red"
            exit 1
            ;;
    esac

    print_summary
    echo ""
    sleep "$ITERATION_DELAY"
done

log "[TIMEOUT] Max iterations ($MAX_ITERATIONS) reached without completion" "yellow"
log "Check sprint-status.yaml for current state" "gray"
print_summary
exit 1
