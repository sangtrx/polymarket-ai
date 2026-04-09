import assert from "node:assert/strict";
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

function createFixtureWorkspace() {
  const root = mkdtempSync(join(tmpdir(), "phase-1-e2e-"));
  const files = {
    ".planning/PRD.md": "# PRD\n- [ ] REQ-1 canonical IDs remain stable",
    "docs/architecture.md":
      "# Architecture\n- [ ] ARCH-1 canonical IDs remain stable",
    ".planning/stories/story-1.md": "# Story\n- [ ] ST-1 deterministic replay",
    ".planning/ROADMAP.md": "# Roadmap\n- [ ] RM-1 include snapshot_digest",
  };
  for (const [relativePath, body] of Object.entries(files)) {
    const absolute = join(root, relativePath);
    mkdirSync(resolve(absolute, ".."), { recursive: true });
    writeFileSync(absolute, body, "utf8");
  }
  return root;
}

function runIngestion(repoRoot) {
  const cargoBin = process.env.CARGO || `${process.env.HOME}/.cargo/bin/cargo`;
  const command = spawnSync(
    cargoBin,
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
    ],
    {
      cwd: resolve("."),
      encoding: "utf8",
    },
  );

  assert.equal(command.status, 0, command.stderr ?? command.error?.message);
  return JSON.parse((command.stdout ?? "").trim());
}

test("ARTF-05: repeated full snapshot ingestion keeps canonical_requirement_ids and snapshot_digest stable", () => {
  const workspace = createFixtureWorkspace();
  try {
    const first = runIngestion(workspace);
    const second = runIngestion(workspace);

    assert.ok(first.snapshot_digest);
    assert.equal(first.snapshot_digest, second.snapshot_digest);
    assert.deepEqual(first.canonical_requirement_ids, second.canonical_requirement_ids);
    assert.ok(first.canonical_requirement_ids.length > 0);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
