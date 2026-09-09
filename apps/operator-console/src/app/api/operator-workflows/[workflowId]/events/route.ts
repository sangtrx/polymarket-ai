import {
  OperatorBffHandoffError,
  streamWorkflowEvents,
} from "@/lib/workflows/server-handoff";

interface WorkflowEventsRouteContext {
  params: Promise<{ workflowId: string }>;
}

function jsonError(status: number, errorCode: string, message: string): Response {
  return Response.json(
    { error_code: errorCode, message, timestamp_utc: new Date().toISOString() },
    { status, headers: { "cache-control": "no-store" } },
  );
}

export async function GET(
  request: Request,
  context: WorkflowEventsRouteContext,
): Promise<Response> {
  if (request.headers.get("sec-fetch-site") === "cross-site") {
    return jsonError(
      403,
      "cross_site_operator_workflow_stream_denied",
      "Cross-site operator workflow streams are not allowed.",
    );
  }

  const { workflowId } = await context.params;
  try {
    const upstream = await streamWorkflowEvents(workflowId, request.signal);
    if (!upstream.ok) {
      return new Response(await upstream.text(), {
        status: upstream.status,
        headers: {
          "cache-control": "no-store",
          "content-type": upstream.headers.get("content-type") ?? "application/json",
        },
      });
    }
    if (!upstream.body) {
      return jsonError(
        502,
        "operator_bff_stream_missing",
        "Operator workflow stream returned no body.",
      );
    }

    return new Response(upstream.body, {
      status: 200,
      headers: {
        "cache-control": "no-cache, no-transform",
        connection: "keep-alive",
        "content-type": "text/event-stream; charset=utf-8",
        "x-accel-buffering": "no",
      },
    });
  } catch (error) {
    if (error instanceof OperatorBffHandoffError) {
      return jsonError(503, error.code, error.message);
    }
    return jsonError(
      503,
      "operator_bff_stream_unavailable",
      "Operator workflow stream is unavailable.",
    );
  }
}
