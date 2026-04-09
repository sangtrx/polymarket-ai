import assert from "node:assert/strict";
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

function createFixtureWorkspace(prefix) {
  const root = mkdtempSync(join(tmpdir(), prefix));
  const files = {
    ".planning/PRD.md": "# PRD\n- [ ] TRAC-1 deterministic evidence mapping",
    "docs/architecture.md": "# Architecture\n- [ ] TRAC-2 deterministic evidence mapping",
    ".planning/stories/story-1.md": "# Story\n- [ ] TRAC-3 deterministic evidence mapping",
    ".planning/ROADMAP.md": "# Roadmap\n- [ ] TRAC-4 deterministic evidence mapping",
    "services/research-gateway/src/placeholder.rs": "// phase-2 placeholder",
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

function runTraceability(repoRoot) {
  const cargoBin = process.env.CARGO || `${process.env.HOME}/.cargo/bin/cargo`;
  const command = spawnSync(
    cargoBin,
    [
      "run",
      "-q",
      "-p",
      "research-gateway",
      "--",
      "trace-evidence",
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

test("TRAC-01 and TRAC-03: each row includes requirement id, rationale, confidence, reason code, and code evidence", () => {
  const workspace = createFixtureWorkspace("phase-2-api-");
  try {
    const first = runTraceability(workspace);
    assert.equal(first.status, 0, first.stderr);
    const firstPayload = JSON.parse(first.stdout);
    const targetRequirement = firstPayload.rows[0].canonical_requirement_id;
    writeFileSync(
      join(workspace, "docs/traceability.md"),
      `# mapped doc\n- ${targetRequirement}`,
      "utf8",
    );
    const second = runTraceability(workspace);
    assert.equal(second.status, 0, second.stderr);
    const payload = JSON.parse(second.stdout);
    assert.ok(Array.isArray(payload.rows));
    assert.ok(payload.rows.length > 0);
    for (const row of payload.rows) {
      assert.equal(typeof row.canonical_requirement_id, "string");
      assert.ok(row.canonical_requirement_id.length > 0);
      assert.equal(typeof row.rationale, "string");
      assert.ok(row.rationale.length > 0);
      assert.ok(["high", "medium", "low"].includes(row.confidence));
      assert.equal(typeof row.reason_code, "string");
      assert.ok(Array.isArray(row.code_anchors));
      for (const anchor of row.code_anchors) {
        assert.equal(typeof anchor.file_path, "string");
        assert.ok(!anchor.file_path.endsWith(".md"));
      }
      if (row.outcome !== "missing_evidence") {
        assert.ok(row.code_anchors.length > 0);
      }
    }
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});

test("TRAC-02: explicit automated assertions produce test evidence anchors", () => {
  const workspace = createFixtureWorkspace("phase-2-api-tests-");
  try {
    const first = runTraceability(workspace);
    assert.equal(first.status, 0, first.stderr);
    const payload = JSON.parse(first.stdout);
    const firstRequirement = payload.rows[0].canonical_requirement_id;

    writeFileSync(
      join(workspace, "services/research-gateway/src/feature.rs"),
      `// ${firstRequirement}\nfn mapped_feature() {}`,
      "utf8",
    );
    writeFileSync(
      join(workspace, "tests/api/feature.test.mjs"),
      `import assert from "node:assert/strict";\nassert.equal(1,1); // ${firstRequirement}`,
      "utf8",
    );

    const second = runTraceability(workspace);
    assert.equal(second.status, 0, second.stderr);
    const nextPayload = JSON.parse(second.stdout);
    const mappedRow = nextPayload.rows.find(
      (row) => row.canonical_requirement_id === firstRequirement,
    );
    assert.ok(mappedRow);
    assert.ok(mappedRow.test_anchors.length > 0);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
