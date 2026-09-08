import {
  OperatorBffHandoffError,
  startReadinessWorkflow,
} from "@/lib/workflows/server-handoff";

function jsonError(status: number, errorCode: string, message: string): Response {
  return Response.json(
    { error_code: errorCode, message, timestamp_utc: new Date().toISOString() },
    { status, headers: { "cache-control": "no-store" } },
  );
}

function isSameOriginMutation(request: Request): boolean {
  const origin = request.headers.get("origin");
  if (!origin) {
    return false;
  }
  return origin === new URL(request.url).origin;
}

export async function POST(request: Request): Promise<Response> {
  if (!isSameOriginMutation(request)) {
    return jsonError(
      403,
      "cross_origin_operator_workflow_denied",
      "Operator workflow mutations require a same-origin browser request.",
    );
  }

  try {
    const upstream = await startReadinessWorkflow();
    const body = await upstream.text();
    return new Response(body, {
      status: upstream.status,
      headers: {
        "cache-control": "no-store",
        "content-type": upstream.headers.get("content-type") ?? "application/json",
      },
    });
  } catch (error) {
    if (error instanceof OperatorBffHandoffError) {
      return jsonError(503, error.code, error.message);
    }
    return jsonError(
      503,
      "operator_bff_handoff_unavailable",
      "Operator workflow handoff is unavailable.",
    );
  }
}
