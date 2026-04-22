import assert from "node:assert/strict";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  mkdirSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

function createFixtureWorkspace() {
  const root = mkdtempSync(join(tmpdir(), "phase-5-e2e-"));
  const files = {
    ".planning/PRD.md":
      "# PRD\n- [ ] GATE-01 deterministic readiness signal\n- [ ] RPTG-01 readiness exports",
    "docs/architecture.md":
      "# Architecture\n- [ ] GATE-03 waiver governance and deterministic report generation",
    ".planning/stories/story-1.md":
      "# Story\n- [ ] RPTG-02 readiness markdown recommendation rationale",
    ".planning/ROADMAP.md":
      "# Roadmap\n- [ ] Phase 5 readiness signal and reporting",
    "services/research-gateway/src/placeholder.rs": "// phase-5 placeholder",
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

function runPhase5Chain(repoRoot) {
  const cargoBin = process.env.CARGO || `${process.env.HOME}/.cargo/bin/cargo`;
  const command = spawnSync(
    cargoBin,
    [
      "run",
      "-q",
      "-p",
      "research-gateway",
      "--",
      "run-phase5-chain",
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
  const lines = (command.stdout ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const payloadLine = [...lines]
    .reverse()
    .find((line) => line.startsWith("{") && line.includes("\"readiness\""));
  assert.ok(payloadLine, "phase5 chain should emit final readiness JSON payload");
  return payloadLine;
}

test("RPTG-01/RPTG-02: run-phase5-chain output is replay-stable with dual readiness artifacts", () => {
  const workspace = createFixtureWorkspace();
  try {
    const firstPayload = runPhase5Chain(workspace);
    const secondPayload = runPhase5Chain(workspace);
    assert.equal(firstPayload, secondPayload);

    const payload = JSON.parse(firstPayload);
    assert.equal(payload.readiness.snapshot_id, "readiness_abc123_2026-04-09T00:00:00Z");
    assert.ok(Array.isArray(payload.artifacts));
    assert.equal(payload.artifacts.length, 2);
    const artifactTypes = payload.artifacts
      .map((artifact) => artifact.artifact_type)
      .sort();
    assert.deepEqual(artifactTypes, [
      "readiness-report.json",
      "readiness-report.md",
    ]);
    for (const artifact of payload.artifacts) {
      assert.equal(typeof artifact.checksum, "string");
      assert.equal(artifact.checksum.length, 64);
    }
    const markdownArtifact = payload.artifacts.find(
      (artifact) => artifact.artifact_type === "readiness-report.md",
    );
    assert.ok(markdownArtifact?.path);
    const markdown = readFileSync(markdownArtifact.path, "utf8");
    assert.match(markdown, /## Recommendation Rationale/);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
