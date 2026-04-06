import type { AttributionBreakdownRow } from "@/lib/portfolio/attribution";

function formatUsd(value: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value);
}

export function PnlAttributionBreakdownTable({
  rows,
}: {
  rows: AttributionBreakdownRow[];
}) {
  return (
    <div className="attribution-table-wrapper">
      <table className="attribution-table">
        <caption className="type-metadata text-muted">
          Cost-aware attribution rows sorted by period window, then market/alpha keys.
        </caption>
        <thead>
          <tr>
            <th scope="col">Market</th>
            <th scope="col">Alpha</th>
            <th scope="col">Realized</th>
            <th scope="col">Unrealized</th>
            <th scope="col">Gross</th>
            <th scope="col">Net</th>
            <th scope="col">Fees</th>
            <th scope="col">Rebates</th>
            <th scope="col">Incentives</th>
            <th scope="col">Net cost</th>
            <th scope="col">Reason</th>
            <th scope="col">Correlation</th>
            <th scope="col">Snapshot</th>
            <th scope="col">Run</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={`${row.marketId}:${row.alphaId}:${row.periodEndUtc}:${row.correlationId}`}>
              <td className="type-metadata">{row.marketId}</td>
              <td className="type-metadata">{row.alphaId}</td>
              <td className="type-mono">{formatUsd(row.realizedPnlUsd)}</td>
              <td className="type-mono">{formatUsd(row.unrealizedPnlUsd)}</td>
              <td className="type-mono">{formatUsd(row.grossPnlUsd)}</td>
              <td className="type-mono">{formatUsd(row.netPnlUsd)}</td>
              <td className="type-mono">{formatUsd(row.feesUsd)}</td>
              <td className="type-mono">{formatUsd(row.rebatesUsd)}</td>
              <td className="type-mono">{formatUsd(row.incentivesUsd)}</td>
              <td className="type-mono">{formatUsd(row.netCostImpactUsd)}</td>
              <td className="type-metadata">
                <code>{row.reasonCode}</code>
              </td>
              <td className="type-metadata">
                <code>{row.correlationId}</code>
              </td>
              <td className="type-metadata">
                <code>{row.snapshotId ?? "n/a"}</code>
              </td>
              <td className="type-metadata">
                <code>{row.runId ?? "n/a"}</code>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
