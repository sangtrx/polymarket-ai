"use client";

import { useEffect, useRef, useState } from "react";

type WorkflowStatus = "queued" | "running" | "succeeded" | "failed";

interface WorkflowEnvelope {
  workflow_id: string;
  status: WorkflowStatus;
  updated_at_utc: string;
  result?: {
    target?: string;
    status?: number;
    ok?: boolean;
    latencyMs?: number;
    observedAtUtc?: string;
  } | null;
  error_code?: string | null;
}

function asWorkflowEnvelope(value: unknown): WorkflowEnvelope | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const record = value as Record<string, unknown>;
  if (
    typeof record.workflow_id !== "string" ||
    (record.status !== "queued" &&
      record.status !== "running" &&
      record.status !== "succeeded" &&
      record.status !== "failed") ||
    typeof record.updated_at_utc !== "string"
  ) {
    return null;
  }
  return value as WorkflowEnvelope;
}

function errorMessage(value: unknown): string {
  if (typeof value === "object" && value !== null) {
    const record = value as Record<string, unknown>;
    if (typeof record.message === "string") {
      return record.message;
    }
    if (typeof record.error_code === "string") {
      return record.error_code;
    }
  }
  return "Operator workflow request failed.";
}

export function OperatorWorkflowRunCard() {
  const [workflow, setWorkflow] = useState<WorkflowEnvelope | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sourceRef = useRef<EventSource | null>(null);

  useEffect(
    () => () => {
      sourceRef.current?.close();
    },
    [],
  );

  async function startWorkflow() {
    sourceRef.current?.close();
    sourceRef.current = null;
    setError(null);
    setIsStarting(true);

    try {
      const response = await fetch("/api/operator-workflows/readiness", {
        method: "POST",
        cache: "no-store",
        headers: { accept: "application/json" },
      });
      const payload: unknown = await response.json();
      if (!response.ok) {
        throw new Error(errorMessage(payload));
      }
      const initial = asWorkflowEnvelope(payload);
      if (!initial) {
        throw new Error("Operator workflow response contract mismatch.");
      }
      setWorkflow(initial);

      const source = new EventSource(
        `/api/operator-workflows/${encodeURIComponent(initial.workflow_id)}/events`,
      );
      sourceRef.current = source;
      source.addEventListener("workflow", (event) => {
        try {
          const next = asWorkflowEnvelope(
            JSON.parse((event as MessageEvent<string>).data) as unknown,
          );
          if (!next) {
            throw new Error("stream contract mismatch");
          }
          setWorkflow(next);
          if (next.status === "succeeded" || next.status === "failed") {
            source.close();
            sourceRef.current = null;
          }
        } catch {
          setError("Operator workflow stream returned invalid data.");
          source.close();
          sourceRef.current = null;
        }
      });
      source.onerror = () => {
        if (sourceRef.current === source) {
          setError("Operator workflow stream disconnected.");
          source.close();
          sourceRef.current = null;
        }
      };
    } catch (startError) {
      setError(
        startError instanceof Error
          ? startError.message
          : "Operator workflow request failed.",
      );
    } finally {
      setIsStarting(false);
    }
  }

  return (
    <article className="shell-panel shell-panel-grid-item">
      <p className="type-eyebrow">Authenticated workflow proof</p>
      <h2 className="type-heading-m">Run readiness snapshot</h2>
      <p className="type-body text-muted">
        Starts a read-only workflow through a same-origin server handoff. The browser
        never receives the BFF signing secret or bearer token.
      </p>

      <button
        className="allocation-form-submit"
        disabled={isStarting || workflow?.status === "running" || workflow?.status === "queued"}
        onClick={() => void startWorkflow()}
        type="button"
      >
        {isStarting ? "Starting…" : "Run readiness snapshot"}
      </button>

      <div aria-live="polite">
        {workflow ? (
          <dl className="shell-state-evidence">
            <div>
              <dt className="type-metadata text-muted">Workflow state</dt>
              <dd className="type-mono">{workflow.status}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Workflow id</dt>
              <dd className="type-mono">{workflow.workflow_id}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Probe latency</dt>
              <dd className="type-mono">
                {typeof workflow.result?.latencyMs === "number"
                  ? `${workflow.result.latencyMs} ms`
                  : "pending"}
              </dd>
            </div>
          </dl>
        ) : null}
        {workflow?.error_code ? (
          <p className="type-metadata text-muted">{workflow.error_code}</p>
        ) : null}
        {error ? <p className="type-metadata text-muted">{error}</p> : null}
      </div>
    </article>
  );
}
