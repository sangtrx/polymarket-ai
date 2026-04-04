<#
.SYNOPSIS
    BMAD Auto Loop — GitHub Copilot CLI Edition (Windows PowerShell)
.DESCRIPTION
    Executes the BMAD workflow in a loop using GitHub Copilot CLI:
    create-story (CS) → validate-story (VS) → dev-story (DS) → code-review (CR)
    → qa-generate-e2e-tests (QA, optional) → sprint-status (SS) → commit
    Each step runs as a NEW Copilot CLI session (fresh context window).
    Continues until all stories reach 'done' state in sprint-status.yaml.

    Model: GPT-5.3-Codex with xhigh reasoning (configurable via params or env vars)
.PARAMETER MaxIterations
    Safety limit for maximum loop iterations (default: 250)
.PARAMETER Model
    AI model to use (default: gpt-5.3-codex)
.PARAMETER ReasoningEffort
    Reasoning effort to use (default: xhigh)
.PARAMETER Verbose
    Enable detailed debug logging
.PARAMETER DryRun
    Show what would happen without actually executing
.EXAMPLE
    .\bmad-loop.ps1
    .\bmad-loop.ps1 -MaxIterations 100 -Verbose
    .\bmad-loop.ps1 -DryRun -Verbose
.NOTES
    Prerequisites:
    - GitHub Copilot CLI installed: npm install -g @github/copilot
    - Authenticated: run `copilot` and use /login
    - BMAD installed with .github/agents/ (sm, dev agents)
    - Sprint planning completed (sprint-status.yaml exists)
#>

param(
    [int]$MaxIterations = 250,
    [string]$Model = "",
    [string]$ReasoningEffort = "",
    [switch]$Verbose,
    [switch]$DryRun
)

$ErrorActionPreference = "Continue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# ============================================================
# CONFIGURATION
# ============================================================

$ConfigPath = Join-Path $ScriptDir "bmad-auto-config.yaml"
$ProgressLog = Join-Path $ScriptDir "bmad-progress.log"
$PromptDir = Join-Path $ScriptDir "prompts"

# Model: prefer parameter > env var > default
if (-not $Model) {
    $Model = $env:COPILOT_MODEL
}
if (-not $Model) {
    $Model = "gpt-5.3-codex"
}

# Reasoning effort: prefer parameter > env var > default
if (-not $ReasoningEffort) {
    $ReasoningEffort = $env:COPILOT_REASONING_EFFORT
}
if (-not $ReasoningEffort) {
    $ReasoningEffort = "xhigh"
}

# Delay between iterations (seconds) with enforced minimum
$IterationDelay = if ($env:ITERATION_DELAY) { [int]$env:ITERATION_DELAY } else { 60 }
if ($IterationDelay -lt 60) {
    $IterationDelay = 60
}

# Canonical loop controls
$EnableValidateGate = if ($env:ENABLE_VALIDATE_GATE) { $env:ENABLE_VALIDATE_GATE } else { "true" }
$EnableQAStep = if ($env:ENABLE_QA_STEP) { $env:ENABLE_QA_STEP } else { "true" }
$EnableSprintStatusStep = if ($env:ENABLE_SPRINT_STATUS_STEP) { $env:ENABLE_SPRINT_STATUS_STEP } else { "true" }

function Test-Enabled {
    param([string]$Value)
    if (-not $Value) { return $false }
    $v = $Value.ToLowerInvariant()
    return $v -eq "true" -or $v -eq "1" -or $v -eq "yes" -or $v -eq "y"
}

# ============================================================
# CONFIGURATION LOADING
# ============================================================

if (-not (Test-Path $ConfigPath)) {
    Write-Host @"
[ERROR] Configuration file not found: $ConfigPath

Re-install: npx bmad-auto-copilot install
"@ -ForegroundColor Red
    exit 1
}

$config = @{}
Get-Content $ConfigPath | ForEach-Object {
    if ($_ -match '^([^#]\w+):\s*"?([^"]+)"?\s*$') {
        $config[$Matches[1].Trim()] = $Matches[2].Trim()
    }
}

$ProjectRoot = $config['project_root']
if (-not $ProjectRoot) {
    Write-Host "[ERROR] 'project_root' not found in config file" -ForegroundColor Red
    exit 1
}

$ImplementationArtifacts = $config['implementation_artifacts']
if (-not $ImplementationArtifacts) {
    $ImplementationArtifacts = "_bmad-output/implementation-artifacts"
}

$SprintStatusPath = Join-Path $ProjectRoot $ImplementationArtifacts "sprint-status.yaml"

# ============================================================
# PREREQUISITE CHECKS
# ============================================================

function Test-Prerequisites {
    if (-not (Get-Command "copilot" -ErrorAction SilentlyContinue)) {
        Write-Host @"
[ERROR] GitHub Copilot CLI not found in PATH

Install Copilot CLI:
  npm:      npm install -g @github/copilot
  winget:   winget install GitHub.Copilot

Then authenticate: copilot (and use /login)
"@ -ForegroundColor Red
        exit 1
    }

    if (-not (Test-Path $SprintStatusPath)) {
        Write-Host @"
[ERROR] sprint-status.yaml not found: $SprintStatusPath

Run sprint planning first:
  copilot --agent sm -p 'Execute the sprint-planning workflow'
"@ -ForegroundColor Red
        exit 1
    }

    $agentsDir = Join-Path $ProjectRoot ".github" "agents"
    if (-not (Test-Path $agentsDir)) {
        Write-Host "[WARNING] .github/agents directory not found" -ForegroundColor Yellow
    }

    if (-not (Test-Path $PromptDir)) {
        Write-Host "[ERROR] Prompts directory not found: $PromptDir" -ForegroundColor Red
        exit 1
    }
}

# ============================================================
# LOGGING
# ============================================================

function Write-Log {
    param(
        [string]$Message,
        [string]$Color = "White"
    )
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host $Message -ForegroundColor $Color
    Add-Content -Path $ProgressLog -Value "[$timestamp] $Message"
}

# ============================================================
# GET NEXT ACTION
# ============================================================

function Get-NextAction {
    if (-not (Test-Path $SprintStatusPath)) {
        Write-Log "[ERROR] sprint-status.yaml not found: $SprintStatusPath" "Red"
        return "error"
    }

    $content = Get-Content $SprintStatusPath -Raw
    $lines = $content -split "`n"

    $storyLines = $lines | Where-Object {
        $_ -match '^\s*[0-9]+-[0-9]+-' -and $_ -notmatch 'retrospective'
    }

    if ($Verbose) {
        Write-Log "[DEBUG] Found $($storyLines.Count) story lines" "Gray"
    }

    # Strip inline YAML comments so status matching works with annotated lines
    $storyLines = $storyLines | ForEach-Object { $_ -replace '\s*#.*$', '' }
    $reviewCount = ($storyLines | Where-Object { $_ -match ':\s*review\s*$' }).Count
    $readyCount = ($storyLines | Where-Object { $_ -match ':\s*ready-for-dev\s*$' }).Count
    $backlogCount = ($storyLines | Where-Object { $_ -match ':\s*backlog\s*$' }).Count
    $doneCount = ($storyLines | Where-Object { $_ -match ':\s*done\s*$' }).Count
    $inProgressCount = ($storyLines | Where-Object { $_ -match ':\s*in-progress\s*$' }).Count
    $totalCount = $storyLines.Count

    if ($Verbose) {
        Write-Log "[DEBUG] Review: $reviewCount | Ready: $readyCount | Backlog: $backlogCount | In-Progress: $inProgressCount | Done: $doneCount / $totalCount" "Gray"
    }

    if ($reviewCount -gt 0) {
        Write-Log "[NEXT] Found story in REVIEW state -> code-review (Dev agent)" "Green"
        return "code-review"
    }
    if ($inProgressCount -gt 0) {
        Write-Log "[NEXT] Found story in IN-PROGRESS state -> dev-story resume (Dev agent)" "Green"
        return "dev-story"
    }
    if ($readyCount -gt 0) {
        Write-Log "[NEXT] Found story in READY-FOR-DEV state -> dev-story (Dev agent)" "Green"
        return "dev-story"
    }
    if ($backlogCount -gt 0) {
        Write-Log "[NEXT] Found story in BACKLOG state -> create-story (SM agent)" "Green"
        return "create-story"
    }

    if ($doneCount -eq $totalCount -and $totalCount -gt 0) {
        return "complete"
    }

    return "wait"
}

# ============================================================
# INVOKE COPILOT CLI (NEW SESSION PER CALL)
# ============================================================

function Invoke-CopilotSession {
    param(
        [string]$Agent,
        [string]$PromptFile,
        [string]$ActionLabel,
        [string]$StoryKey = ""
    )

    if (-not (Test-Path $PromptFile)) {
        Write-Log "[ERROR] Prompt file not found: $PromptFile" "Red"
        return $false
    }

    $prompt = Get-Content $PromptFile -Raw
    $prompt = $prompt -replace '\{TIMESTAMP\}', (Get-Date -Format "yyyy-MM-dd HH:mm:ss")
    $prompt = $prompt -replace '\{STORY_KEY\}', $StoryKey

    Write-Log "[SESSION] New Copilot CLI session -> Agent: $Agent | Model: $Model | Reasoning: $ReasoningEffort | Action: $ActionLabel" "Magenta"

    if ($DryRun) {
        Write-Log "[DRY RUN] Would execute: copilot --model $Model --reasoning-effort $ReasoningEffort -p '...' --allow-all --no-ask-user" "Yellow"
        return $true
    }

    $tempPrompt = [System.IO.Path]::GetTempFileName()
    try {
        Set-Content -Path $tempPrompt -Value $prompt

        # NOTE: We do NOT use --agent to avoid the agent's menu/activation loop.
        # The prompt itself contains all workflow instructions.
        copilot `
            --model $Model `
            --reasoning-effort $ReasoningEffort `
            --allow-all `
            --no-ask-user `
            -p $prompt

        $success = $LASTEXITCODE -eq 0

        if ($success) {
            Write-Log "[SUCCESS] Session completed: $ActionLabel" "Green"
        } else {
            Write-Log "[ERROR] Session failed (exit $LASTEXITCODE): $ActionLabel" "Red"
        }

        return $success
    }
    finally {
        Remove-Item $tempPrompt -Force -ErrorAction SilentlyContinue
    }
}

# ============================================================
# GET CURRENT STORY KEY
# ============================================================

function Get-StoryKeyForState {
    param([string]$State)
    $lines = Get-Content $SprintStatusPath | ForEach-Object { $_ -replace '\s*#.*$', '' } | Where-Object { $_ -match '^\s*\d+-\d+-' -and $_ -notmatch 'retrospective' -and $_ -match ": *${State} *$" }
    if ($lines) {
        $first = ($lines | Select-Object -First 1).Trim()
        return ($first -split ':')[0]
    }
    return "unknown"
}

function Get-StoryStateByKey {
    param([string]$StoryKey)

    if (-not $StoryKey) { return "" }

    $line = Get-Content $SprintStatusPath |
        Where-Object { $_ -match "^\s*${StoryKey}:" } |
        Select-Object -First 1

    if (-not $line) { return "" }

    $line = $line -replace '\s*#.*$', ''
    $line = $line -replace '^[^:]*:\s*', ''
    return $line.Trim()
}

function Get-StoryStateToken {
    param([string]$RawState)
    if (-not $RawState) { return "" }
    if ($RawState -match '^\s*([A-Za-z-]+)') {
        return $Matches[1].ToLowerInvariant()
    }
    return $RawState.Trim().ToLowerInvariant()
}

# ============================================================
# GIT COMMIT
# ============================================================

function Invoke-GitCommit {
    param([string]$StoryKey = "unknown")
    Write-Log "[GIT] Checking for changes to commit..." "Yellow"

    if ($DryRun) {
        Write-Log "[DRY RUN] Would check git status and commit" "Yellow"
        return $true
    }

    Push-Location $ProjectRoot
    try {
        $status = git status --porcelain

        if (-not $status) {
            Write-Log "[INFO] No changes to commit" "Gray"
            return $true
        }

        $changedFiles = ($status -split "`n").Count
        Write-Log "[GIT] Committing $changedFiles changed file(s)..." "Yellow"

        git add -A
        git commit -m "feat(${StoryKey}): BMAD auto-complete [copilot-cli]"

        if ($LASTEXITCODE -eq 0) {
            Write-Log "[SUCCESS] Changes committed successfully" "Green"
            return $true
        } else {
            Write-Log "[ERROR] Git commit failed" "Red"
            return $false
        }
    }
    finally {
        Pop-Location
    }
}

# ============================================================
# STATUS SUMMARY
# ============================================================

function Write-Summary {
    if (-not (Test-Path $SprintStatusPath)) { return }

    $content = Get-Content $SprintStatusPath -Raw
    $lines = $content -split "`n"
    $storyLines = $lines | Where-Object {
        $_ -match '^\s*[0-9]+-[0-9]+-' -and $_ -notmatch 'retrospective'
    } | ForEach-Object { $_ -replace '\s*#.*$', '' }
    $done = ($storyLines | Where-Object { $_ -match ':\s*done\s*$' }).Count
    $total = $storyLines.Count
    $pct = if ($total -gt 0) { [math]::Round($done * 100 / $total) } else { 0 }

    Write-Log "[PROGRESS] $done/$total stories done ($pct%)" "Cyan"
}

# ============================================================
# MAIN LOOP
# ============================================================

Test-Prerequisites

Write-Host ""
Write-Log "================================================================" "Cyan"
Write-Log "    BMAD Auto Loop - GitHub Copilot CLI Edition" "Cyan"
Write-Log "    Model: $Model" "Cyan"
Write-Log "================================================================" "Cyan"
Write-Host ""
Write-Log "[START] BMAD Auto Loop Started" "Green"
Write-Log "Max iterations: $MaxIterations" "Gray"
Write-Log "Model: $Model" "Gray"
Write-Log "Reasoning effort: $ReasoningEffort" "Gray"
Write-Log "Delay between steps: ${IterationDelay}s" "Gray"
Write-Log "Validate gate enabled: $EnableValidateGate" "Gray"
Write-Log "QA step enabled: $EnableQAStep" "Gray"
Write-Log "Sprint-status step enabled: $EnableSprintStatusStep" "Gray"
Write-Log "Project root: $ProjectRoot" "Gray"
Write-Log "Sprint status: $SprintStatusPath" "Gray"
if ($DryRun) {
    Write-Log "[DRY RUN MODE] No actions will be executed" "Yellow"
}
Write-Host ""

Write-Summary
Write-Host ""

for ($iteration = 1; $iteration -le $MaxIterations; $iteration++) {
    Write-Log "=========================================" "Cyan"
    Write-Log "  Iteration $iteration / $MaxIterations" "Cyan"
    Write-Log "=========================================" "Cyan"

    $action = Get-NextAction

    switch ($action) {
        "create-story" {
            Write-Log "[ACTION] CREATE STORY (SM Agent -> new session)" "Yellow"
            Invoke-CopilotSession -Agent "bmad-agent-bmm-sm" -PromptFile (Join-Path $PromptDir "create-story.md") -ActionLabel "create-story"

            if (Test-Enabled $EnableValidateGate) {
                Write-Log "[ACTION] VALIDATE STORY READINESS (VS gate)" "Yellow"
                $validateOk = Invoke-CopilotSession -Agent "bmad-agent-bmm-sm" -PromptFile (Join-Path $PromptDir "validate-story.md") -ActionLabel "validate-story"
                if (-not $validateOk) {
                    Write-Log "[WARN] Validate-story step failed; will retry canonical CS->VS cycle next iteration" "Yellow"
                }
            }
        }
        "dev-story" {
            Write-Log "[ACTION] DEVELOP STORY (Dev Agent -> new session)" "Yellow"
            Invoke-CopilotSession -Agent "bmad-agent-bmm-dev" -PromptFile (Join-Path $PromptDir "dev-story.md") -ActionLabel "dev-story"
        }
        "code-review" {
            Write-Log "[ACTION] CODE REVIEW (Dev Agent -> new session)" "Yellow"
            # Capture story key before review (it's in 'review' state now)
            $reviewStoryKey = Get-StoryKeyForState -State "review"
            Invoke-CopilotSession -Agent "bmad-agent-bmm-dev" -PromptFile (Join-Path $PromptDir "code-review.md") -ActionLabel "code-review"

            # Only commit if the story actually moved to 'done'
            if (-not $reviewStoryKey -or $reviewStoryKey -eq "unknown") {
                Write-Log "[WARN] Could not determine review story key; skipping auto-commit this iteration" "Yellow"
            } else {
                $currentState = Get-StoryStateByKey -StoryKey $reviewStoryKey
                $currentStateToken = Get-StoryStateToken -RawState $currentState

                if ($currentStateToken -ne $currentState) {
                    Write-Log "[WARN] Non-canonical sprint status value for $reviewStoryKey: '$currentState' (parsed token: '$currentStateToken')" "Yellow"
                }

                if ($currentStateToken -eq "done") {
                    Write-Log "[VERIFIED] Story $reviewStoryKey confirmed DONE" "Green"

                    $qaPassed = $true
                    if (Test-Enabled $EnableQAStep) {
                        Write-Log "[ACTION] QA GENERATE E2E TESTS (recommended)" "Yellow"
                        $qaPassed = Invoke-CopilotSession -Agent "bmad-agent-bmm-qa" -PromptFile (Join-Path $PromptDir "qa-generate-e2e-tests.md") -ActionLabel "qa-generate-e2e-tests" -StoryKey $reviewStoryKey
                        if (-not $qaPassed) {
                            Write-Log "[WARN] QA step failed; skipping commit and continuing loop for remediation" "Yellow"
                        }
                    }

                    if ($qaPassed -and (Test-Enabled $EnableSprintStatusStep)) {
                        Write-Log "[ACTION] SPRINT STATUS UPDATE + NEXT STORY" "Yellow"
                        $ssOk = Invoke-CopilotSession -Agent "bmad-agent-bmm-sm" -PromptFile (Join-Path $PromptDir "sprint-status.md") -ActionLabel "sprint-status" -StoryKey $reviewStoryKey
                        if (-not $ssOk) {
                            Write-Log "[WARN] Sprint-status step failed; continuing to commit because story is done" "Yellow"
                        }
                    }

                    if ($qaPassed) {
                        Invoke-GitCommit -StoryKey $reviewStoryKey
                    }
                } else {
                    Write-Log "[RETRY] Story $reviewStoryKey still in '$currentState' (token: '$currentStateToken', not done) - will retry next iteration" "Yellow"
                }
            }
        }
        "complete" {
            Write-Host ""
            Write-Log "================================================================" "Green"
            Write-Log "    ALL STORIES COMPLETED!" "Green"
            Write-Log "================================================================" "Green"
            Write-Host ""
            Write-Log "Sprint status: All stories are DONE" "Green"
            Write-Log "Total iterations used: $iteration" "Gray"
            Write-Summary
            exit 0
        }
        "wait" {
            Write-Log "[WAIT] No actionable stories (may be in-progress)" "Yellow"
            Write-Log "Skipping this iteration..." "Gray"
        }
        "error" {
            Write-Log "[ERROR] Error state - stopping loop" "Red"
            exit 1
        }
    }

    Write-Summary
    Write-Host ""
    Start-Sleep -Seconds $IterationDelay
}

Write-Log "[TIMEOUT] Max iterations ($MaxIterations) reached without completion" "Yellow"
Write-Log "Check sprint-status.yaml for current state" "Gray"
Write-Summary
exit 1
