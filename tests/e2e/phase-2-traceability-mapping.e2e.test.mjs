import assert from "node:assert/strict";
import { mkdtempSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

function createFixtureWorkspace() {
  const root = mkdtempSync(join(tmpdir(), "phase-2-e2e-"));
  const files = {
    ".planning/PRD.md": "# PRD\n- [ ] TRAC-1 deterministic evidence mapping",
    "docs/architecture.md": "# Architecture\n- [ ] TRAC-2 deterministic evidence mapping",
    ".planning/stories/story-1.md": "# Story\n- [ ] TRAC-3 deterministic evidence mapping",
    ".planning/ROADMAP.md": "# Roadmap\n- [ ] TRAC-4 deterministic evidence mapping",
    "services/research-gateway/src/placeholder-a.rs": "// placeholder",
    "services/research-gateway/src/placeholder-b.rs": "// placeholder",
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
  assert.equal(command.status, 0, command.stderr ?? command.error?.message);
  return JSON.parse((command.stdout ?? "").trim());
}

test("deterministic replay keeps output ordering and exposes ambiguous, missing, and stale outcomes", () => {
  const workspace = createFixtureWorkspace();
  try {
    const first = runTraceability(workspace);
    const firstRequirement = first.rows[0].canonical_requirement_id;
    const secondRequirement = first.rows[1]?.canonical_requirement_id ?? firstRequirement;

    writeFileSync(
      join(workspace, "services/research-gateway/src/feature-a.rs"),
      `// ${firstRequirement}\nfn feature_a() {}`,
      "utf8",
    );
    writeFileSync(
      join(workspace, "services/research-gateway/src/feature-b.rs"),
      `// ${firstRequirement}\nfn feature_b() {}`,
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

    const replayA = runTraceability(workspace);
    const replayB = runTraceability(workspace);

    assert.deepEqual(
      replayA.rows.map((row) => row.canonical_requirement_id),
      replayB.rows.map((row) => row.canonical_requirement_id),
    );
    assert.ok(replayA.rows.some((row) => row.outcome === "ambiguous"));
    assert.ok(replayA.rows.some((row) => row.outcome === "missing_evidence"));
    assert.ok(replayA.rows.some((row) => row.outcome === "stale_evidence"));
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
