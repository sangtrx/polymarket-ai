export type ShellDataState =
  | "ready"
  | "loading"
  | "empty"
  | "error"
  | "unauthorized";

export type ShellSearchParams = Record<
  string,
  string | string[] | undefined
>;

export interface ShellFreshnessSnapshot {
  lastUpdatedIso: string;
  source: string;
  isStale: boolean;
}

export interface ShellEvidence {
  errorCode: string;
  message: string;
  timestampIso: string;
}

export interface ShellReadModelSnapshot {
  dataState: ShellDataState;
  freshness: ShellFreshnessSnapshot;
  p95TargetMs: number;
  evidence?: ShellEvidence;
}

const P95_QUERY_TARGET_MS = 2_000;

const DEFAULT_MESSAGES: Record<Exclude<ShellDataState, "ready">, string> = {
  loading:
    "Read model fetch in progress. If this state persists beyond p95 budget, treat as degraded.",
  empty:
    "Read model returned no records for this view. Validate upstream projection health before operating.",
  error:
    "Read model request failed. Route remains non-destructive until a healthy payload is available.",
  unauthorized:
    "Shell context denied for current role. Request approved role elevation through governance workflow.",
};

const DEFAULT_ERROR_CODES: Record<Exclude<ShellDataState, "ready">, string> = {
  loading: "SHELL_LOADING",
  empty: "SHELL_EMPTY_READ_MODEL",
  error: "SHELL_READ_MODEL_UNAVAILABLE",
  unauthorized: "SHELL_CONTEXT_UNAUTHORIZED",
};

function firstValue(value: string | string[] | undefined): string | undefined {
  if (Array.isArray(value)) {
    return value[0];
  }
  return value;
}

function normalizeState(value: string | undefined): ShellDataState {
  const normalized = value?.trim().toLowerCase();
  if (!normalized) {
    return "ready";
  }

  if (
    normalized === "ready" ||
    normalized === "loading" ||
    normalized === "empty" ||
    normalized === "error" ||
    normalized === "unauthorized"
  ) {
    return normalized;
  }

  return "error";
}

function isTruthyFlag(value: string | undefined): boolean {
  const normalized = value?.trim().toLowerCase();
  return normalized === "1" || normalized === "true" || normalized === "yes";
}

function normalizeOptionalValue(value: string | undefined): string | undefined {
  const normalized = value?.trim();
  return normalized ? normalized : undefined;
}

function resolveTimestamp(rawValue: string | undefined): string {
  if (rawValue) {
    const parsed = Date.parse(rawValue);
    if (!Number.isNaN(parsed)) {
      return new Date(parsed).toISOString();
    }
  }

  return new Date().toISOString();
}

function resolveSource(rawValue: string | undefined, fallback: string): string {
  if (!rawValue) {
    return fallback;
  }

  const normalized = rawValue.trim();
  if (
    normalized.length > 0 &&
    normalized.length <= 120 &&
    /^[a-z0-9._:-]+$/i.test(normalized)
  ) {
    return normalized;
  }

  return fallback;
}

export function resolveShellReadModel(
  searchParams: ShellSearchParams,
  defaultSource: string,
): ShellReadModelSnapshot {
  const state = normalizeState(firstValue(searchParams.shellState));
  const timestampIso = resolveTimestamp(
    normalizeOptionalValue(firstValue(searchParams.at)),
  );
  const source = resolveSource(firstValue(searchParams.source), defaultSource);
  const staleByInput = isTruthyFlag(firstValue(searchParams.stale));
  const isStale = staleByInput || state !== "ready";
  const errorCode = normalizeOptionalValue(firstValue(searchParams.errorCode));
  const message = normalizeOptionalValue(firstValue(searchParams.message));

  const baseSnapshot: ShellReadModelSnapshot = {
    dataState: state,
    freshness: {
      lastUpdatedIso: timestampIso,
      source,
      isStale,
    },
    p95TargetMs: P95_QUERY_TARGET_MS,
  };

  if (state === "ready") {
    return baseSnapshot;
  }

  return {
    ...baseSnapshot,
    evidence: {
      errorCode: errorCode ?? DEFAULT_ERROR_CODES[state],
      message: message ?? DEFAULT_MESSAGES[state],
      timestampIso,
    },
  };
}
