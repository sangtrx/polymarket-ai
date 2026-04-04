import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  BootstrapError,
  compareSemver,
  createFailurePayload,
  emitCheckLine,
  parseSemver,
  printMachineError,
  throwFailure,
  timestampUtc,
} from "./check-utils.mjs";

const MIN_NODE_VERSION = "20.9.0";
const PNPM_MAJOR = 10;

/**
 * @param {string} raw
 */
export function extractToolVersion(raw) {
  const match = raw.match(/(\d+\.\d+\.\d+)/);
  if (!match) {
    throw new Error(`Unable to parse version from: ${raw}`);
  }
  return match[1];
}

/**
 * @param {string} nodeVersion
 */
export function validateNodeVersion(nodeVersion) {
  if (
    compareSemver(parseSemver(nodeVersion), parseSemver(MIN_NODE_VERSION)) < 0
  ) {
    throwFailure(
      "BOOTSTRAP_TOOLCHAIN_MISMATCH",
      "node-version",
      `Use Node.js >= ${MIN_NODE_VERSION}`,
      { expected: `>=${MIN_NODE_VERSION}`, actual: nodeVersion },
    );
  }

  return { expected: `>=${MIN_NODE_VERSION}`, actual: nodeVersion };
}

/**
 * @param {string} pnpmVersion
 */
export function validatePnpmVersion(pnpmVersion) {
  const [major] = parseSemver(pnpmVersion);
  if (major !== PNPM_MAJOR) {
    throwFailure(
      "BOOTSTRAP_TOOLCHAIN_MISMATCH",
      "pnpm-version",
      `Install pnpm major ${PNPM_MAJOR} to match packageManager lock.`,
      { expected_major: PNPM_MAJOR, actual: pnpmVersion },
    );
  }

  return { expected_major: PNPM_MAJOR, actual: pnpmVersion };
}

/**
 * @param {string} rustToolchainContent
 */
export function parseRustToolchainChannel(rustToolchainContent) {
  const match = rustToolchainContent.match(/channel\s*=\s*"([^"]+)"/);
  if (!match) {
    throwFailure(
      "BOOTSTRAP_CONFIG_INVALID",
      "rust-toolchain-channel",
      "Set [toolchain].channel in rust-toolchain.toml (for example: 1.90.0).",
    );
  }
  return match[1];
}

/**
 * @param {string} expected
 * @param {string} actual
 * @param {"rustc" | "cargo"} binary
 */
export function validateRustVersion(expected, actual, binary) {
  if (expected !== actual) {
    throwFailure(
      "BOOTSTRAP_TOOLCHAIN_MISMATCH",
      `${binary}-version`,
      `Align ${binary} to rust-toolchain.toml channel ${expected}.`,
      { expected, actual },
    );
  }

  return { expected, actual };
}

/**
 * @param {string} repoRoot
 */
export function runPreflight(repoRoot) {
  const matrix = [];
  const runCheck = (check, fn) => {
    const details = fn();
    matrix.push({ check, status: "pass", details });
    emitCheckLine(check, "pass", details);
  };

  runCheck("node-version", () => validateNodeVersion(process.version));

  runCheck("pnpm-version", () => {
    const pnpmVersion = execFileSync("pnpm", ["--version"], {
      encoding: "utf8",
    }).trim();
    return validatePnpmVersion(pnpmVersion);
  });

  const rustToolchainContent = readFileSync(
    resolve(repoRoot, "rust-toolchain.toml"),
    "utf8",
  );
  const pinnedRustVersion = parseRustToolchainChannel(rustToolchainContent);

  runCheck("rustc-version", () => {
    const rustcOutput = execFileSync("rustc", ["--version"], {
      encoding: "utf8",
    }).trim();
    const rustcVersion = extractToolVersion(rustcOutput);
    return validateRustVersion(pinnedRustVersion, rustcVersion, "rustc");
  });

  runCheck("cargo-version", () => {
    const cargoOutput = execFileSync("cargo", ["--version"], {
      encoding: "utf8",
    }).trim();
    const cargoVersion = extractToolVersion(cargoOutput);
    return validateRustVersion(pinnedRustVersion, cargoVersion, "cargo");
  });

  return {
    timestamp_utc: timestampUtc(),
    checks: matrix,
  };
}

function main() {
  try {
    const summary = runPreflight(process.cwd());
    process.stdout.write(`${JSON.stringify(summary)}\n`);
  } catch (error) {
    if (error instanceof BootstrapError) {
      printMachineError(error);
      process.exitCode = 1;
      return;
    }

    const payload = createFailurePayload(
      "BOOTSTRAP_PRECHECK_CRASH",
      "preflight-execution",
      "Inspect stack trace and rerun `npm run preflight`.",
      {
        message: error instanceof Error ? error.message : String(error),
      },
    );
    printMachineError(payload);
    process.exitCode = 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
