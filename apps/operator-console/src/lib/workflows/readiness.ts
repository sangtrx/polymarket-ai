interface ResponseLike {
  ok: boolean;
  status: number;
  json: () => Promise<unknown>;
}

type FetchLike = (url: string, init: RequestInit) => Promise<ResponseLike>;

export interface OperatorBffReadiness {
  status: "ready" | "not_ready" | "unavailable";
  authConfigured: boolean;
  endpoint: string;
  observedAtUtc: string;
  errorCode?: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export async function queryOperatorBffReadiness(input: {
  baseUrl: string;
  fetchImpl?: FetchLike;
}): Promise<OperatorBffReadiness> {
  const endpoint = `${input.baseUrl.replace(/\/$/, "")}/readyz`;
  if (!input.baseUrl) {
    return {
      status: "unavailable",
      authConfigured: false,
      endpoint,
      observedAtUtc: new Date().toISOString(),
      errorCode: "operator_bff_base_url_missing",
    };
  }

  const fetchImpl = input.fetchImpl ?? fetch;
  try {
    const response = await fetchImpl(endpoint, {
      method: "GET",
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    const payload = await response.json();
    if (!isRecord(payload)) {
      throw new Error("readiness payload must be an object");
    }
    const status = payload.status;
    const authConfigured = payload.auth_configured;
    if (
      (status !== "ready" && status !== "not_ready") ||
      typeof authConfigured !== "boolean"
    ) {
      throw new Error("readiness payload contract mismatch");
    }
    return {
      status,
      authConfigured,
      endpoint,
      observedAtUtc: new Date().toISOString(),
      ...(response.ok ? {} : { errorCode: "operator_bff_not_ready" }),
    };
  } catch {
    return {
      status: "unavailable",
      authConfigured: false,
      endpoint,
      observedAtUtc: new Date().toISOString(),
      errorCode: "operator_bff_unavailable",
    };
  }
}
