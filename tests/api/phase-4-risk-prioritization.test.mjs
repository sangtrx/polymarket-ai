import assert from "node:assert/strict";
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

function createFixtureWorkspace(prefix) {
  const root = mkdtempSync(join(tmpdir(), prefix));
  const files = {
    ".planning/PRD.md":
      "# PRD\n- [ ] RISK-1 prioritize unresolved deployment risks\n- [ ] RISK-2 deterministic fix ordering",
    "docs/architecture.md":
      "# Architecture\n- [ ] RISK-3 replayable risk output",
    ".planning/stories/story-1.md":
      "# Story\n- [ ] RISK-4 prioritize-risk CLI support",
    ".planning/ROADMAP.md":
      "# Roadmap\n- [ ] RISK-5 deployment risk matrix",
    "services/research-gateway/src/placeholder.rs": "// phase-4 placeholder",
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
  return {
    status: command.status ?? 1,
    stdout: (command.stdout ?? "").trim(),
    stderr: (command.stderr ?? command.error?.message ?? "").trim(),
  };
}

test("RISK-01: prioritize-risk excludes covered rows from output while keeping unresolved rows ranked", () => {
  const workspace = createFixtureWorkspace("phase-4-api-");
  try {
    const baseline = runPrioritizeRisk(workspace);
    assert.equal(baseline.status, 0, baseline.stderr);
    const baselinePayload = JSON.parse(baseline.stdout);
    assert.ok(Array.isArray(baselinePayload.rows));
    assert.ok(baselinePayload.rows.length > 0);

    const promotedRequirement = baselinePayload.rows[0].canonical_requirement_id;
    writeFileSync(
      join(workspace, "services/research-gateway/src/covered.rs"),
      `// ${promotedRequirement}\nfn covered_requirement() {}`,
      "utf8",
    );
    writeFileSync(
      join(workspace, "tests/api/covered.test.mjs"),
      `import assert from "node:assert/strict";\nassert.equal(1,1); // ${promotedRequirement}`,
      "utf8",
    );

    const result = runPrioritizeRisk(workspace);
    assert.equal(result.status, 0, result.stderr);
    const payload = JSON.parse(result.stdout);
    assert.ok(Array.isArray(payload.rows));
    assert.ok(payload.rows.length > 0);
    assert.ok(payload.rows.length <= baselinePayload.rows.length);

    for (const row of payload.rows) {
      assert.notEqual(row.coverage_class, "covered");
      assert.ok(["critical", "high", "medium", "low"].includes(row.severity));
      assert.equal(typeof row.risk_score, "number");
      assert.equal(typeof row.priority_rank, "number");
      assert.equal(typeof row.reason_code, "string");
      assert.ok(row.reason_code.length > 0);
    }
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});

test("RISK-01: unknown reason fallback emits risk_weight_unmapped_reason with high severity", () => {
  const cargoBin = process.env.CARGO || `${process.env.HOME}/.cargo/bin/cargo`;
  const command = spawnSync(
    cargoBin,
    [
      "test",
      "-q",
      "-p",
      "research-gateway",
      "risk::tests::guardrails_unknown_reason_still_maps_to_high_severity",
    ],
    {
      cwd: resolve("."),
      encoding: "utf8",
    },
  );
  assert.equal(command.status, 0, command.stderr ?? command.error?.message);
});
