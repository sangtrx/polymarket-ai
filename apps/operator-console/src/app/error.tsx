"use client";

import { useMemo } from "react";

interface ErrorPageProps {
  error: Error & { digest?: string };
  reset: () => void;
}

export default function Error({ error, reset }: ErrorPageProps) {
  const timestampIso = useMemo(() => new Date().toISOString(), []);
  const errorCode = error.digest ?? "SHELL_ROUTE_ERROR";
  const safeMessage =
    process.env.NODE_ENV === "production"
      ? "Route render failed. Retry after read-model health is restored."
      : error.message;

  return (
    <main className="shell-frame">
      <section
        className="shell-state-panel shell-state-panel--full"
        data-state="error"
        role="alert"
      >
        <p className="type-eyebrow">Shell fallback state</p>
        <h1 className="type-heading-l">Route-level shell error</h1>
        <p className="type-body text-muted">
          The shell could not render this route. The UI remains non-destructive
          until a healthy read-model context is restored.
        </p>
        <dl className="shell-state-evidence">
          <div>
            <dt className="type-metadata text-muted">Error code</dt>
            <dd className="type-mono">
              <code>{errorCode}</code>
            </dd>
          </div>
          <div>
            <dt className="type-metadata text-muted">Message</dt>
            <dd className="type-mono">{safeMessage}</dd>
          </div>
          <div>
            <dt className="type-metadata text-muted">Timestamp</dt>
            <dd className="type-mono">
              <time dateTime={timestampIso}>{timestampIso}</time>
            </dd>
          </div>
        </dl>
        <button className="shell-tab" onClick={() => reset()} type="button">
          Retry shell route
        </button>
      </section>
    </main>
  );
}
