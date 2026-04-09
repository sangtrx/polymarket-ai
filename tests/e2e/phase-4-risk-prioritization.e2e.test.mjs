import assert from "node:assert/strict";
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

function createFixtureWorkspace() {
  const root = mkdtempSync(join(tmpdir(), "phase-4-e2e-"));
  const files = {
    ".planning/PRD.md":
      "# PRD\n- [ ] RISK-1 deterministic deployment risk ordering\n- [ ] RISK-2 contiguous priority ranks",
    "docs/architecture.md":
      "# Architecture\n- [ ] RISK-3 replayable risk snapshots",
    ".planning/stories/story-1.md":
      "# Story\n- [ ] RISK-4 prioritize-risk end-to-end command",
    ".planning/ROADMAP.md":
      "# Roadmap\n- [ ] RISK-5 risk output consumed by CI reporting",
    "services/research-gateway/src/placeholder-a.rs": "// placeholder-a",
    "services/research-gateway/src/placeholder-b.rs": "// placeholder-b",
    "tests/api/placeholder.test.mjs":
      "import assert from 'node:assert/strict'; assert.equal(1, 1);",
  };
  for (const [relativePath, body] of Object.entries(files)) {
    const absolute = join(root, relativePath);
    mkdirSync(resolve(absolute, ".."), { recursive: true });
    writeFileSync(absolute, body, "utf8");
  }
  return root;
}

function runPrioritizeRisk(repoRoot) {
  const cargoBin = process.env.CARGO || `${process.env.HOME}/.cargo/bin/cargo`;
  const command = spawnSync(
    cargoBin,
    [
      "run",
      "-q",
      "-p",
      "research-gateway",
      "--",
      "prioritize-risk",
      "--commit-sha",
      "abc123",
      "--generated-at-utc",
      "2026-04-09T00:00:00Z",
      "--repo-root",
      repoRoot,
    ],
    {
      cwd: resolve("."),
      encoding: "utf8",
    },
  );
  assert.equal(command.status, 0, command.stderr ?? command.error?.message);
  return (command.stdout ?? "").trim();
}

function severityRank(severity) {
  return (
    {
      critical: 4,
      high: 3,
      medium: 2,
      low: 1,
    }[severity] ?? 0
  );
}

test("RISK-02: prioritize-risk ordering and priority ranks are deterministic end-to-end", () => {
  const workspace = createFixtureWorkspace();
  try {
    const firstPayload = runPrioritizeRisk(workspace);
    const secondPayload = runPrioritizeRisk(workspace);
    assert.equal(firstPayload, secondPayload);

    const payload = JSON.parse(firstPayload);
    assert.ok(Array.isArray(payload.rows));
    assert.ok(payload.rows.length > 0);

    for (let i = 0; i < payload.rows.length; i += 1) {
      const current = payload.rows[i];
      assert.equal(current.priority_rank, i + 1);
      assert.ok(["critical", "high", "medium", "low"].includes(current.severity));

      if (i === 0) {
        continue;
      }
      const previous = payload.rows[i - 1];
      const previousSeverity = severityRank(previous.severity);
      const currentSeverity = severityRank(current.severity);
      assert.ok(
        previousSeverity >= currentSeverity,
        "severity must be sorted desc",
      );

      if (previousSeverity === currentSeverity) {
        assert.ok(
          previous.risk_score >= current.risk_score,
          "risk_score must be sorted desc when severity ties",
        );
      }

      if (
        previousSeverity === currentSeverity &&
        previous.risk_score === current.risk_score
      ) {
        assert.ok(
          previous.canonical_requirement_id <= current.canonical_requirement_id,
          "canonical_requirement_id must be sorted asc when severity and score tie",
        );
      }
    }
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
