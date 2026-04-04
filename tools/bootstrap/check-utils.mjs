import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

export const FAILURE_FIELDS = [
  "error_code",
  "failed_check",
  "remediation_hint",
  "timestamp_utc",
];

export class BootstrapError extends Error {
  /**
   * @param {Record<string, unknown>} payload
   */
  constructor(payload) {
    super(String(payload.failed_check ?? "bootstrap-check-failed"));
    this.name = "BootstrapError";
    this.payload = payload;
  }
}

export function timestampUtc() {
  return new Date().toISOString();
}

/**
 * @param {string} errorCode
 * @param {string} failedCheck
 * @param {string} remediationHint
 * @param {Record<string, unknown> | undefined} details
 */
export function createFailurePayload(
  errorCode,
  failedCheck,
  remediationHint,
  details = undefined,
) {
  const payload = {
    error_code: errorCode,
    failed_check: failedCheck,
    remediation_hint: remediationHint,
    timestamp_utc: timestampUtc(),
  };

  if (details !== undefined) {
    payload.details = details;
  }

  return payload;
}

/**
 * @param {string} errorCode
 * @param {string} failedCheck
 * @param {string} remediationHint
 * @param {Record<string, unknown> | undefined} details
 */
export function throwFailure(
  errorCode,
  failedCheck,
  remediationHint,
  details = undefined,
) {
  throw new BootstrapError(
    createFailurePayload(errorCode, failedCheck, remediationHint, details),
  );
}

/**
 * @param {BootstrapError | Record<string, unknown>} error
 */
export function printMachineError(error) {
  const payload = error instanceof BootstrapError ? error.payload : error;
  process.stderr.write(`${JSON.stringify(payload)}\n`);
}

/**
 * @param {string} check
 * @param {"pass" | "fail"} status
 * @param {Record<string, unknown>} details
 */
export function emitCheckLine(check, status, details) {
  process.stdout.write(
    `${JSON.stringify({
      timestamp_utc: timestampUtc(),
      check,
      status,
      details,
    })}\n`,
  );
}

/**
 * @param {string} evidenceFile
 * @param {Record<string, unknown>} payload
 */
export function writeEvidenceFile(evidenceFile, payload) {
  mkdirSync(dirname(evidenceFile), { recursive: true });
  writeFileSync(evidenceFile, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
}

/**
 * @param {string} rawVersion
 */
export function parseSemver(rawVersion) {
  const normalized = rawVersion
    .trim()
    .replace(/^v/, "")
    .split(/[+-]/)[0];
  const [majorRaw, minorRaw = "0", patchRaw = "0"] = normalized.split(".");
  const major = Number.parseInt(majorRaw, 10);
  const minor = Number.parseInt(minorRaw, 10);
  const patch = Number.parseInt(patchRaw, 10);

  if ([major, minor, patch].some(Number.isNaN)) {
    throw new Error(`Invalid semver: ${rawVersion}`);
  }

  return [major, minor, patch];
}

/**
 * @param {[number, number, number]} left
 * @param {[number, number, number]} right
 */
export function compareSemver(left, right) {
  for (let i = 0; i < 3; i += 1) {
    if (left[i] > right[i]) {
      return 1;
    }
    if (left[i] < right[i]) {
      return -1;
    }
  }

  return 0;
}
