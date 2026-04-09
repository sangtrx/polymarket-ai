import assert from "node:assert/strict";
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

function createFixtureWorkspace(prefix, files) {
  const root = mkdtempSync(join(tmpdir(), prefix));
  for (const [relativePath, body] of Object.entries(files)) {
    const absolute = join(root, relativePath);
    mkdirSync(resolve(absolute, ".."), { recursive: true });
    writeFileSync(absolute, body, "utf8");
  }
  return root;
}

function runIngestion(repoRoot, args = []) {
  const command = spawnSync(
    "cargo",
    [
      "run",
      "-q",
      "-p",
      "research-gateway",
      "--",
      "ingest-artifacts",
      "--commit-sha",
      "abc123",
      "--ingested-at-utc",
      "2026-04-09T00:00:00Z",
      "--repo-root",
      repoRoot,
      ...args,
    ],
    {
      cwd: resolve("."),
      encoding: "utf8",
    },
  );
  return {
    status: command.status ?? 1,
    stdout: command.stdout.trim(),
    stderr: command.stderr.trim(),
  };
}

test("ARTF-01 ARTF-02 ARTF-03 ARTF-04: ingestion produces canonical coverage counts by artifact class", () => {
  const workspace = createFixtureWorkspace("phase-1-api-", {
    ".planning/PRD.md": "# PRD\n- [ ] REQ-1 canonical ids are deterministic",
    "docs/architecture.md":
      "# Architecture\n- [ ] ARCH-1 source provenance remains explicit",
    ".planning/stories/story-1.md":
      "# Story\n- [ ] ST-1 story artifacts ingest into canonical dataset",
    ".planning/ROADMAP.md":
      "# Roadmap\n- [ ] RM-1 roadmap requirements are preserved",
  });
  try {
    const result = runIngestion(workspace);
    assert.equal(result.status, 0, result.stderr);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.commit_sha, "abc123");
    assert.equal(payload.ingested_at_utc, "2026-04-09T00:00:00Z");
    assert.ok(payload.snapshot_digest);
    assert.equal(payload.counts_by_artifact_type.prd, 1);
    assert.equal(payload.counts_by_artifact_type.architecture, 1);
    assert.equal(payload.counts_by_artifact_type.story, 1);
    assert.equal(payload.counts_by_artifact_type.roadmap, 1);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});

test("schema violations fail closed with machine-readable canonical_artifact_invalid_payload code", () => {
  const workspace = createFixtureWorkspace("phase-1-api-invalid-", {
    ".planning/PRD.md":
      "# PRD\n| ID | Acceptance |\n| --- |\n| ACC-1 | malformed acceptance row |",
    "docs/architecture.md": "# Architecture\n- [ ] ARCH-1",
    ".planning/stories/story-1.md": "# Story\n- [ ] ST-1",
    ".planning/ROADMAP.md": "# Roadmap\n- [ ] RM-1",
  });
  try {
    const result = runIngestion(workspace);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /canonical_artifact_invalid_payload/);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});

test("valid ingestion emits unresolved conflict findings while succeeding", () => {
  const workspace = createFixtureWorkspace("phase-1-api-conflicts-", {
    ".planning/PRD.md": "# PRD\n- [ ] REQ-2 Execution must require manual approval",
    "docs/architecture.md":
      "# Architecture\n- [ ] ARCH-2 Execution must not require manual approval",
    ".planning/stories/story-1.md":
      "# Story\n- [ ] ST-1 Execution must require manual approval",
    ".planning/ROADMAP.md": "# Roadmap\n- [ ] RM-1 Include conflict evidence",
  });
  try {
    const result = runIngestion(workspace);
    assert.equal(result.status, 0, result.stderr);
    const payload = JSON.parse(result.stdout);
    assert.ok(Array.isArray(payload.unresolved_conflicts));
    assert.ok(payload.unresolved_conflicts.length >= 1);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
