import assert from "node:assert/strict";
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

function createFixtureWorkspace() {
  const root = mkdtempSync(join(tmpdir(), "phase-3-e2e-"));
  const files = {
    ".planning/PRD.md": "# PRD\n- [ ] COVR-1 deterministic coverage matrix",
    "docs/architecture.md": "# Architecture\n- [ ] COVR-2 explainable output rows",
    ".planning/stories/story-1.md": "# Story\n- [ ] COVR-3 replayable matrix",
    ".planning/ROADMAP.md": "# Roadmap\n- [ ] COVR-4 full baseline coverage",
    "services/research-gateway/src/placeholder-a.rs": "// placeholder",
    "services/research-gateway/src/placeholder-b.rs": "// placeholder",
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

function runCoverageClassification(repoRoot) {
  const cargoBin = process.env.CARGO || `${process.env.HOME}/.cargo/bin/cargo`;
  const command = spawnSync(
    cargoBin,
    [
      "run",
      "-q",
      "-p",
      "research-gateway",
      "--",
      "classify-coverage",
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
  return JSON.parse((command.stdout ?? "").trim());
}

test("COVR-03: classify-coverage produces deterministic, complete matrix with provenance on replay", () => {
  const workspace = createFixtureWorkspace();
  try {
    const first = runCoverageClassification(workspace);
    const firstRequirement = first.rows[0].canonical_requirement_id;
    const secondRequirement =
      first.rows[1]?.canonical_requirement_id ?? firstRequirement;

    writeFileSync(
      join(workspace, "services/research-gateway/src/feature-a.rs"),
      `// ${firstRequirement}\nfn feature_a() {}`,
      "utf8",
    );
    writeFileSync(
      join(workspace, "tests/api/feature-a.test.mjs"),
      `import assert from "node:assert/strict";\nassert.equal(1,1); // ${firstRequirement}`,
      "utf8",
    );
    writeFileSync(
      join(workspace, ".traceability-history.json"),
      JSON.stringify({
        [secondRequirement]: [
          {
            snapshot_id: "traceability_snapshot_0",
            file_path: "services/research-gateway/src/deleted.rs",
            symbol: "deleted_symbol",
            line_start: 1,
            line_end: 2,
          },
        ],
      }),
      "utf8",
    );

    const replayA = runCoverageClassification(workspace);
    const replayB = runCoverageClassification(workspace);

    assert.equal(replayA.snapshot_id, replayB.snapshot_id);
    assert.equal(replayA.commit_sha, replayB.commit_sha);
    assert.equal(replayA.generated_at_utc, replayB.generated_at_utc);
    assert.ok(Array.isArray(replayA.rows));
    assert.ok(replayA.rows.length > 0);

    const replayAIds = replayA.rows.map((row) => row.canonical_requirement_id);
    const replayBIds = replayB.rows.map((row) => row.canonical_requirement_id);
    assert.deepEqual(replayAIds, replayBIds);

    const sortedIds = [...replayAIds].sort();
    assert.deepEqual(replayAIds, sortedIds);
    assert.equal(new Set(replayAIds).size, replayA.rows.length);

    assert.ok(replayA.rows.some((row) => row.class === "covered"));
    assert.ok(replayA.rows.some((row) => row.class === "partial"));
    assert.ok(replayA.rows.some((row) => row.class === "missing"));

    for (const row of replayA.rows) {
      assert.equal(typeof row.canonical_requirement_id, "string");
      assert.equal(typeof row.reason_code, "string");
      assert.equal(typeof row.rationale, "string");
      assert.equal(typeof row.provenance, "string");
      assert.ok(Array.isArray(row.code_anchors));
      assert.ok(Array.isArray(row.test_anchors));
      assert.ok(Array.isArray(row.ambiguous_candidates));
    }
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
