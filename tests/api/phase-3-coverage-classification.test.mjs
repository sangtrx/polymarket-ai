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
      "# PRD\n- [ ] COVR-1 classify every scoped requirement\n- [ ] COVR-2 keep rationale payloads",
    "docs/architecture.md":
      "# Architecture\n- [ ] COVR-3 deterministic matrix output",
    ".planning/stories/story-1.md":
      "# Story\n- [ ] COVR-4 expose classify coverage command",
    ".planning/ROADMAP.md":
      "# Roadmap\n- [ ] COVR-5 complete matrix with provenance",
    "services/research-gateway/src/placeholder.rs": "// phase-3 placeholder",
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
  return {
    status: command.status ?? 1,
    stdout: (command.stdout ?? "").trim(),
    stderr: (command.stderr ?? command.error?.message ?? "").trim(),
  };
}

test("COVR-01: classify-coverage emits one covered|partial|missing row per canonical requirement", () => {
  const workspace = createFixtureWorkspace("phase-3-api-");
  try {
    const first = runCoverageClassification(workspace);
    assert.equal(first.status, 0, first.stderr);
    const firstPayload = JSON.parse(first.stdout);
    const targetRequirement = firstPayload.rows[0].canonical_requirement_id;

    writeFileSync(
      join(workspace, "services/research-gateway/src/feature.rs"),
      `// ${targetRequirement}\nfn mapped_feature() {}`,
      "utf8",
    );
    writeFileSync(
      join(workspace, "tests/api/feature.test.mjs"),
      `import assert from "node:assert/strict";\nassert.equal(1,1); // ${targetRequirement}`,
      "utf8",
    );

    const second = runCoverageClassification(workspace);
    assert.equal(second.status, 0, second.stderr);
    const payload = JSON.parse(second.stdout);
    assert.equal(typeof payload.snapshot_id, "string");
    assert.equal(typeof payload.commit_sha, "string");
    assert.equal(typeof payload.generated_at_utc, "string");
    assert.ok(Array.isArray(payload.rows));
    assert.ok(payload.rows.length > 0);

    const ids = payload.rows.map((row) => row.canonical_requirement_id);
    const uniqueIds = new Set(ids);
    assert.equal(uniqueIds.size, ids.length);
    const sortedIds = [...ids].sort();
    assert.deepEqual(ids, sortedIds);

    for (const row of payload.rows) {
      assert.equal(typeof row.canonical_requirement_id, "string");
      assert.ok(["covered", "partial", "missing"].includes(row.class));
      assert.ok(Array.isArray(row.code_anchors));
      assert.ok(Array.isArray(row.test_anchors));
      assert.ok(Array.isArray(row.ambiguous_candidates));
    }
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});

test("COVR-02: partial/missing rows always include explicit reason_code and rationale lineage", () => {
  const workspace = createFixtureWorkspace("phase-3-api-reason-");
  try {
    const result = runCoverageClassification(workspace);
    assert.equal(result.status, 0, result.stderr);
    const payload = JSON.parse(result.stdout);
    const lineageCodes = new Set([
      "deterministic_anchor_match",
      "semantic_fallback_used",
      "stale_link_invalidated",
      "missing_evidence",
      "ambiguous_multiple_candidates",
    ]);

    const rowsNeedingReason = payload.rows.filter(
      (row) => row.class === "partial" || row.class === "missing",
    );
    assert.ok(rowsNeedingReason.length > 0);

    for (const row of rowsNeedingReason) {
      assert.equal(typeof row.reason_code, "string");
      assert.ok(row.reason_code.length > 0);
      assert.equal(typeof row.rationale, "string");
      assert.ok(row.rationale.length > 0);
      assert.ok(lineageCodes.has(row.reason_code));
      assert.ok(Array.isArray(row.code_anchors));
      assert.ok(Array.isArray(row.test_anchors));
      assert.ok(Array.isArray(row.ambiguous_candidates));
    }
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
