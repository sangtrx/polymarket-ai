import type {
  ShellDataState,
  ShellReadModelSnapshot,
} from "@/lib/shell/read-models";

const STATE_TITLES: Record<Exclude<ShellDataState, "ready">, string> = {
  loading: "Loading read model evidence",
  empty: "Read model returned no results",
  error: "Read model unavailable",
  unauthorized: "Unauthorized shell context",
};

export function ShellStatePanel({
  stateModel,
}: {
  stateModel: ShellReadModelSnapshot;
}) {
  if (stateModel.dataState === "ready") {
    return null;
  }

  const evidence = stateModel.evidence;

  return (
    <section
      className="shell-state-panel"
      data-state={stateModel.dataState}
      role={stateModel.dataState === "loading" ? "status" : "alert"}
    >
      <p className="type-eyebrow">Shell fallback state</p>
      <h2 className="type-heading-l">{STATE_TITLES[stateModel.dataState]}</h2>
      <p className="type-body text-muted">
        {evidence?.message ??
          "State evidence unavailable. Treat shell as degraded until trace is restored."}
      </p>

      {evidence ? (
        <dl className="shell-state-evidence">
          <div>
            <dt className="type-metadata text-muted">Error code</dt>
            <dd className="type-mono">
              <code>{evidence.errorCode}</code>
            </dd>
          </div>
          <div>
            <dt className="type-metadata text-muted">Timestamp</dt>
            <dd className="type-mono">
              <time dateTime={evidence.timestampIso}>
                {evidence.timestampIso}
              </time>
            </dd>
          </div>
          <div>
            <dt className="type-metadata text-muted">Source</dt>
            <dd className="type-mono">{stateModel.freshness.source}</dd>
          </div>
        </dl>
      ) : null}
    </section>
  );
}
