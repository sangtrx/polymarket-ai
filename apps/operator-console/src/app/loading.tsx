const LOADING_CODE = "SHELL_LOADING";

export default function Loading() {
  const timestampIso = new Date().toISOString();

  return (
    <main className="shell-frame">
      <section
        className="shell-state-panel shell-state-panel--full"
        data-state="loading"
        role="status"
      >
        <p className="type-eyebrow">Shell fallback state</p>
        <h1 className="type-heading-l">Loading read model evidence</h1>
        <p className="type-body text-muted">
          Dashboard shell is waiting for read-model payloads. Controls remain
          non-destructive until evidence resolves.
        </p>
        <dl className="shell-state-evidence">
          <div>
            <dt className="type-metadata text-muted">Error code</dt>
            <dd className="type-mono">
              <code>{LOADING_CODE}</code>
            </dd>
          </div>
          <div>
            <dt className="type-metadata text-muted">Timestamp</dt>
            <dd className="type-mono">
              <time dateTime={timestampIso}>{timestampIso}</time>
            </dd>
          </div>
        </dl>
      </section>
    </main>
  );
}
