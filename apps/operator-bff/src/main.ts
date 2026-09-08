import { buildServer } from "./server.js";

const DEFAULT_PORT = 8090;
const port = Number.parseInt(process.env.OPERATOR_BFF_PORT ?? `${DEFAULT_PORT}`, 10);
const server = buildServer();

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(
    `${JSON.stringify({ event: "operator_bff_started", port, timestamp_utc: new Date().toISOString() })}\n`,
  );
});
