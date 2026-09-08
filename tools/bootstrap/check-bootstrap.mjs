import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  BootstrapError,
  createFailurePayload,
  emitCheckLine,
  printMachineError,
  throwFailure,
  timestampUtc,
  writeEvidenceFile,
} from "./check-utils.mjs";

export const REQUIRED_PATHS = [
  "Cargo.toml",
  "Cargo.lock",
  "package.json",
  "pnpm-workspace.yaml",
  "rust-toolchain.toml",
  ".env.example",
  "apps/operator-console/package.json",
  ".github/workflows/ci-rust.yml",
  ".github/workflows/ci-web.yml",
  ".github/workflows/security.yml",
];

export const REQUIRED_ENV_KEYS = [
  "RUNTIME_RUN_AS_NON_ROOT",
  "RUNTIME_UID",
  "RUNTIME_GID",
  "CONTROL_API_BIND_ADDRESS",
  "CONTROL_API_PORT",
  "CONTROL_API_JWT_ISSUER",
  "CONTROL_API_JWT_AUDIENCE",
  "CONTROL_API_JWKS_URL",
  "EXECUTION_POLYMARKET_API_BASE_URL",
  "EXECUTION_POLYMARKET_PRIVATE_KEY",
  "EXECUTION_POLYMARKET_API_KEY",
  "EXECUTION_POLYMARKET_CHAIN_ID",
  "RISK_DEFAULT_MODE",
  "RISK_MAX_ORDER_NOTIONAL_USD",
  "OPERATOR_CONSOLE_PUBLIC_API_BASE_URL",
  "OPERATOR_CONSOLE_SENTRY_DSN",
  "OPERATOR_CONSOLE_FEATURE_FLAGS",
];

const SENSITIVE_ENV_KEY_PATTERN = /(SECRET|TOKEN|PRIVATE_KEY|API_KEY|PASSWORD|DSN)$/;
const RUNTIME_PLACEHOLDER_PREFIX = "REPLACE_AT_RUNTIME";
const REQUIRED_ROOT_SCRIPTS = [
  "bootstrap:verify",
  "ci:rust",
  "ci:web",
  "ci:security",
  "rust:test",
  "web:build",
];

/**
 * @param {string} envContent
 */
export function parseEnvEntries(envContent) {
  const entries = new Map();
  for (const rawLine of envContent.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }
    const match = line.match(/^([A-Z0-9_]+)=(.*)$/);
    if (!match) {
      continue;
    }
    entries.set(match[1], match[2].trim());
  }
  return entries;
}

/**
 * @param {string} value
 */
export function isRuntimePlaceholder(value) {
  return value.startsWith(RUNTIME_PLACEHOLDER_PREFIX);
}

/**
 * @param {Map<string, string>} envEntries
 */
export function validateEnvironmentTemplate(envEntries) {
  const missingKeys = REQUIRED_ENV_KEYS.filter((key) => !envEntries.has(key));
  if (missingKeys.length > 0) {
    throwFailure(
      "BOOTSTRAP_MISSING_PLACEHOLDER",
      "environment_placeholders",
      "Add all required placeholder keys to .env.example.",
      { missing_keys: missingKeys },
    );
  }

  for (const [key, value] of envEntries.entries()) {
    if (SENSITIVE_ENV_KEY_PATTERN.test(key) && !isRuntimePlaceholder(value)) {
      throwFailure(
        "BOOTSTRAP_PLAINTEXT_SECRET",
        "environment_placeholders",
        `Replace ${key} with a REPLACE_AT_RUNTIME_* placeholder.`,
        { key, value_preview: value.slice(0, 24) },
      );
    }
  }

  if (envEntries.get("RUNTIME_RUN_AS_NON_ROOT") !== "true") {
    throwFailure(
      "BOOTSTRAP_LEAST_PRIVILEGE",
      "least-privilege-defaults",
      "Set RUNTIME_RUN_AS_NON_ROOT=true in .env.example.",
      { actual: envEntries.get("RUNTIME_RUN_AS_NON_ROOT") ?? null },
    );
  }

  if (envEntries.get("CONTROL_API_BIND_ADDRESS") !== "127.0.0.1") {
    throwFailure(
      "BOOTSTRAP_LEAST_PRIVILEGE",
      "least-privilege-defaults",
      "Set CONTROL_API_BIND_ADDRESS=127.0.0.1 for local-safe default.",
      { actual: envEntries.get("CONTROL_API_BIND_ADDRESS") ?? null },
    );
  }

  const runtimeUid = envEntries.get("RUNTIME_UID");
  const runtimeGid = envEntries.get("RUNTIME_GID");
  if (runtimeUid === "0" || runtimeGid === "0") {
    throwFailure(
      "BOOTSTRAP_LEAST_PRIVILEGE",
      "least-privilege-defaults",
      "Use non-root UID/GID defaults in .env.example.",
      { runtime_uid: runtimeUid, runtime_gid: runtimeGid },
    );
  }
}

/**
 * @param {string} repoRoot
 */
export function validateRequiredPaths(repoRoot) {
  const missingPaths = REQUIRED_PATHS.filter(
    (path) => !existsSync(resolve(repoRoot, path)),
  );

  if (missingPaths.length > 0) {
    throwFailure(
      "BOOTSTRAP_REQUIRED_PATH_MISSING",
      "required_paths",
      "Create all baseline scaffold files required by Story 1.1.",
      { missing_paths: missingPaths },
    );
  }

  return { required_path_count: REQUIRED_PATHS.length };
}

/**
 * @param {string} repoRoot
 */
export function validateReproducibility(repoRoot) {
  const rootPackageJson = JSON.parse(
    readFileSync(resolve(repoRoot, "package.json"), "utf8"),
  );
  const appPackageJson = JSON.parse(
    readFileSync(resolve(repoRoot, "apps/operator-console/package.json"), "utf8"),
  );
  const rustToolchain = readFileSync(
    resolve(repoRoot, "rust-toolchain.toml"),
    "utf8",
  );
  const rustChannelMatch = rustToolchain.match(/channel\s*=\s*"([^"]+)"/);
  if (!rustChannelMatch) {
    throwFailure(
      "BOOTSTRAP_REPRODUCIBILITY_GAP",
      "reproducible-entrypoints",
      "Define a pinned rust toolchain channel in rust-toolchain.toml.",
    );
  }

  if (appPackageJson.dependencies?.next !== "16.2.6") {
    throwFailure(
      "BOOTSTRAP_REPRODUCIBILITY_GAP",
      "web-version-pin",
      "Pin apps/operator-console Next.js dependency to 16.2.6.",
      { actual: appPackageJson.dependencies?.next ?? null },
    );
  }

  if (
    typeof rootPackageJson.packageManager !== "string" ||
    !rootPackageJson.packageManager.startsWith("pnpm@")
  ) {
    throwFailure(
      "BOOTSTRAP_REPRODUCIBILITY_GAP",
      "package-manager-pin",
      "Set packageManager to an explicit pnpm version in root package.json.",
      { actual: rootPackageJson.packageManager ?? null },
    );
  }

  const missingScripts = REQUIRED_ROOT_SCRIPTS.filter(
    (scriptName) =>
      typeof rootPackageJson.scripts?.[scriptName] !== "string" ||
      rootPackageJson.scripts[scriptName].length === 0,
  );
  if (missingScripts.length > 0) {
    throwFailure(
      "BOOTSTRAP_REPRODUCIBILITY_GAP",
      "reproducible-entrypoints",
      "Add required root scripts for deterministic bootstrap/build flow.",
      { missing_scripts: missingScripts },
    );
  }

  const reproducibilityDescriptor = {
    rust_toolchain: rustChannelMatch[1],
    package_manager: rootPackageJson.packageManager,
    next_version: appPackageJson.dependencies.next,
    scripts: Object.fromEntries(
      REQUIRED_ROOT_SCRIPTS.map((scriptName) => [
        scriptName,
        rootPackageJson.scripts[scriptName],
      ]),
    ),
  };
  const entrypointHash = createHash("sha256")
    .update(JSON.stringify(reproducibilityDescriptor))
    .digest("hex");

  return {
    rust_toolchain: rustChannelMatch[1],
    package_manager: rootPackageJson.packageManager,
    next_version: appPackageJson.dependencies.next,
    entrypoint_hash: entrypointHash,
  };
}

/**
 * @param {{ repoRoot: string; evidenceFile?: string }} args
 */
export function runBootstrapChecks({ repoRoot, evidenceFile = undefined }) {
  const matrix = [];
  const runCheck = (checkName, fn) => {
    const details = fn();
    matrix.push({ check: checkName, status: "pass", details });
    emitCheckLine(checkName, "pass", details);
    return details;
  };

  runCheck("required_paths", () => validateRequiredPaths(repoRoot));

  const envContent = readFileSync(resolve(repoRoot, ".env.example"), "utf8");
  runCheck("environment_template", () => {
    const entries = parseEnvEntries(envContent);
    validateEnvironmentTemplate(entries);
    return { checked_keys: REQUIRED_ENV_KEYS.length };
  });

  const reproducibility = runCheck("reproducible_entrypoints", () =>
    validateReproducibility(repoRoot),
  );

  const summary = {
    timestamp_utc: timestampUtc(),
    artifact_flow: ["ci:rust", "ci:web", "ci:security"],
    entrypoint_hash: reproducibility.entrypoint_hash,
    check_matrix: matrix,
  };

  if (evidenceFile) {
    writeEvidenceFile(resolve(repoRoot, evidenceFile), summary);
  }

  return summary;
}

function parseCliArgs() {
  /** @type {{ repoRoot: string; evidenceFile?: string }} */
  const parsed = { repoRoot: process.cwd() };
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
    const summary = runBootstrapChecks(args);
    process.stdout.write(`${JSON.stringify(summary)}\n`);
  } catch (error) {
    if (error instanceof BootstrapError) {
      printMachineError(error);
      process.exitCode = 1;
      return;
    }

    const payload = createFailurePayload(
      "BOOTSTRAP_UNEXPECTED_ERROR",
      "bootstrap-check-runner",
      "Inspect stack trace and rerun bootstrap checks.",
      { message: error instanceof Error ? error.message : String(error) },
    );
    printMachineError(payload);
    process.exitCode = 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
