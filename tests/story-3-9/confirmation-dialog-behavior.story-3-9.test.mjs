import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { rmSync, mkdtempSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const PROJECT_ROOT = resolve(".");
const OPERATOR_CONSOLE_FILTER = "operator-console";
const MODULE_SOURCE_PATH = "src/lib/risk/confirmation-dialog.ts";

let compiledOutputDir;
let confirmationDialogModule;

function findFileRecursively(rootDir, fileName) {
  const queue = [rootDir];
  while (queue.length > 0) {
    const nextDir = queue.shift();
    const entries = readdirSync(nextDir, { withFileTypes: true });
    for (const entry of entries) {
      const entryPath = join(nextDir, entry.name);
      if (entry.isDirectory()) {
        queue.push(entryPath);
      } else if (entry.isFile() && entry.name === fileName) {
        return entryPath;
      }
    }
  }
  return undefined;
}

function loadCompiledModule() {
  if (confirmationDialogModule) {
    return confirmationDialogModule;
  }

  compiledOutputDir = mkdtempSync(join(tmpdir(), "story-3-9-confirmation-dialog-"));
  const compileResult = spawnSync(
    "pnpm",
    [
      "--filter",
      OPERATOR_CONSOLE_FILTER,
      "exec",
      "tsc",
      "--pretty",
      "false",
      "--module",
      "commonjs",
      "--target",
      "es2022",
      "--outDir",
      compiledOutputDir,
      MODULE_SOURCE_PATH,
    ],
    { cwd: PROJECT_ROOT, encoding: "utf8" },
  );
  assert.equal(compileResult.status, 0, compileResult.stderr || compileResult.stdout);

  const compiledPath = findFileRecursively(compiledOutputDir, "confirmation-dialog.js");
  assert.ok(compiledPath, "expected compiled confirmation-dialog.js output");

  const require = createRequire(import.meta.url);
  confirmationDialogModule = require(compiledPath);
  return confirmationDialogModule;
}

test.after(() => {
  if (compiledOutputDir) {
    rmSync(compiledOutputDir, { recursive: true, force: true });
  }
});

test("Story 3.9 confirmation dialog traps Tab focus between confirm/cancel controls", () => {
  const { resolveConfirmationTabLoop } = loadCompiledModule();

  assert.equal(
    resolveConfirmationTabLoop({
      key: "Tab",
      shiftKey: false,
      activeControl: "cancel",
    }),
    "confirm",
  );
  assert.equal(
    resolveConfirmationTabLoop({
      key: "Tab",
      shiftKey: true,
      activeControl: "confirm",
    }),
    "cancel",
  );
  assert.equal(
    resolveConfirmationTabLoop({
      key: "Tab",
      shiftKey: false,
      activeControl: "confirm",
    }),
    null,
  );
});

test("Story 3.9 confirmation dialog only dismisses on Escape when actions are not busy", () => {
  const { shouldDismissDangerConfirmation } = loadCompiledModule();

  assert.equal(shouldDismissDangerConfirmation({ key: "Escape", isBusy: false }), true);
  assert.equal(shouldDismissDangerConfirmation({ key: "Escape", isBusy: true }), false);
  assert.equal(shouldDismissDangerConfirmation({ key: "Enter", isBusy: false }), false);
});
