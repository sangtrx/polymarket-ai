import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runBootstrapChecks } from "./check-bootstrap.mjs";
import {
  BootstrapError,
  createFailurePayload,
  emitCheckLine,
  printMachineError,
  throwFailure,
  timestampUtc,
  writeEvidenceFile,
} from "./check-utils.mjs";

const PRIVATE_KEY_PATTERN = /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/;
const SENSITIVE_CLI_ARG_PATTERN =
  /--(?:token|password|secret|api-key|private-key)(?:=|\s+)(?!\$\{\{\s*secrets\.)\S+/i;
const MAX_SCAN_FILE_BYTES = 512 * 1024;
const REPOSITORY_SCAN_ROOTS = [
  "apps",
  "services",
  "crates",
  "tools",
  "tests",
  "infra",
  "docs",
  ".github",
];
const EXCLUDED_SCAN_DIRS = new Set([
  ".git",
  ".next",
  ".cursor",
  ".windsurf",
  ".claude",
  "_bmad",
  "_bmad-output",
  "node_modules",
  "target",
  "dist",
  "out",
  "coverage",
  "bootstrap-evidence",
]);

/**
 * @param {string} repoRoot
 */
function readRootAndWorkflowContent(repoRoot) {
  const rootPackage = JSON.parse(readFileSync(resolve(repoRoot, "package.json"), "utf8"));
  const workflows = [
    ".github/workflows/ci-rust.yml",
    ".github/workflows/ci-web.yml",
    ".github/workflows/security.yml",
  ].map((path) => ({
    path,
    content: readFileSync(resolve(repoRoot, path), "utf8"),
  }));

  return { rootPackage, workflows };
}

/**
 * @param {string} absolutePath
 */
function readTextContentForScan(absolutePath) {
  const content = readFileSync(absolutePath);
  if (content.length === 0 || content.length > MAX_SCAN_FILE_BYTES) {
    return undefined;
  }
  if (content.includes(0)) {
    return undefined;
  }

  return content.toString("utf8");
}

/**
 * @param {string} repoRoot
 */
function collectRepositorySourceCandidates(repoRoot) {
  /** @type {Array<{ source: string; content: string }>} */
  const candidates = [];
  /** @type {Array<string>} */
  const pendingDirs = REPOSITORY_SCAN_ROOTS.filter((rootDir) =>
    existsSync(resolve(repoRoot, rootDir)),
  );

  while (pendingDirs.length > 0) {
    const relativeDir = pendingDirs.pop();
    if (relativeDir === undefined) {
      continue;
    }

    const absoluteDir = resolve(repoRoot, relativeDir);
    const entries = readdirSync(absoluteDir, { withFileTypes: true });
    for (const entry of entries) {
      const relativePath = `${relativeDir}/${entry.name}`;
      const absolutePath = resolve(repoRoot, relativePath);

      if (entry.isDirectory()) {
        if (EXCLUDED_SCAN_DIRS.has(entry.name)) {
          continue;
        }
        pendingDirs.push(relativePath);
        continue;
      }

      if (!entry.isFile()) {
        continue;
      }

      const content = readTextContentForScan(absolutePath);
      if (content !== undefined) {
        candidates.push({ source: relativePath, content });
      }
    }
  }

  return candidates;
}

/**
 * @param {string} repoRoot
 */
function checkLockfiles(repoRoot) {
  const required = ["Cargo.lock", "pnpm-lock.yaml"];
  const missing = required.filter((path) => !existsSync(resolve(repoRoot, path)));
  if (missing.length > 0) {
    throwFailure(
      "SECURITY_LOCKFILE_MISSING",
      "dependency-lockfiles",
      "Generate and commit all dependency lockfiles.",
      { missing_paths: missing },
    );
  }

  return { checked: required };
}

/**
 * @param {string} repoRoot
 * @param {{ rootPackage: Record<string, unknown>; workflows: Array<{path: string; content: string}> }} data
 */
function checkNoEmbeddedSecrets(repoRoot, data) {
  const envTemplate = readFileSync(resolve(repoRoot, ".env.example"), "utf8");
  const candidateStrings = [
    { source: ".env.example", content: envTemplate },
    { source: "package.json scripts", content: JSON.stringify(data.rootPackage.scripts ?? {}) },
    ...data.workflows.map(({ path, content }) => ({ source: path, content })),
    ...collectRepositorySourceCandidates(repoRoot),
  ];
  const uniqueCandidates = Array.from(
    new Map(candidateStrings.map((candidate) => [candidate.source, candidate])).values(),
  ).sort((left, right) => left.source.localeCompare(right.source));

  for (const candidate of uniqueCandidates) {
    if (PRIVATE_KEY_PATTERN.test(candidate.content)) {
      throwFailure(
        "SECURITY_PRIVATE_KEY_LITERAL",
        "secret-material-scan",
        "Remove committed private key material and use runtime secret injection.",
        { source: candidate.source },
      );
    }
    if (SENSITIVE_CLI_ARG_PATTERN.test(candidate.content)) {
      throwFailure(
        "SECURITY_SENSITIVE_CLI_ARG",
        "sensitive-cli-args",
        "Do not pass literal secrets in CLI args; use secret stores/environment variables.",
        { source: candidate.source },
      );
    }
  }

  return {
    scanned_source_count: uniqueCandidates.length,
    scanned_sources: uniqueCandidates.map((item) => item.source),
  };
}

/**
 * @param {string} repoRoot
 * @param {string | undefined} evidenceFile
 */
export function runSecurityScan(repoRoot, evidenceFile = undefined) {
  const matrix = [];
  const runCheck = (name, fn) => {
    const details = fn();
    matrix.push({ check: name, status: "pass", details });
    emitCheckLine(name, "pass", details);
    return details;
  };

  const bootstrapSummary = runBootstrapChecks({ repoRoot });
  runCheck("dependency_lockfiles", () => checkLockfiles(repoRoot));
  const sourceData = readRootAndWorkflowContent(repoRoot);
  runCheck("secret_material_scan", () => checkNoEmbeddedSecrets(repoRoot, sourceData));

  const summary = {
    timestamp_utc: timestampUtc(),
    baseline_entrypoint_hash: bootstrapSummary.entrypoint_hash,
    check_matrix: matrix,
  };

  if (evidenceFile) {
    writeEvidenceFile(resolve(repoRoot, evidenceFile), summary);
  }

  return summary;
}

function parseCliArgs() {
  const parsed = { repoRoot: process.cwd(), evidenceFile: undefined };
  for (let index = 2; index < process.argv.length; index += 1) {
    const arg = process.argv[index];
    if (arg === "--root") {
      parsed.repoRoot = resolve(process.argv[index + 1]);
      index += 1;
      continue;
    }
    if (arg === "--evidence-file") {
      parsed.evidenceFile = process.argv[index + 1];
      index += 1;
    }
  }
  return parsed;
}

function main() {
  try {
    const args = parseCliArgs();
    const summary = runSecurityScan(args.repoRoot, args.evidenceFile);
    process.stdout.write(`${JSON.stringify(summary)}\n`);
  } catch (error) {
    if (error instanceof BootstrapError) {
      printMachineError(error);
      process.exitCode = 1;
      return;
    }

    const payload = createFailurePayload(
      "SECURITY_SCAN_CRASH",
      "security-scan-runner",
      "Inspect stack trace and rerun `npm run ci:security`.",
      { message: error instanceof Error ? error.message : String(error) },
    );
    printMachineError(payload);
    process.exitCode = 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
