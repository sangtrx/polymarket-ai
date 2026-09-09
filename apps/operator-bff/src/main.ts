import { buildServer } from "./server.js";

const DEFAULT_PORT = 8090;
const DEFAULT_BIND_ADDRESS = "127.0.0.1";
const port = Number.parseInt(process.env.OPERATOR_BFF_PORT ?? `${DEFAULT_PORT}`, 10);
const bindAddress = process.env.OPERATOR_BFF_BIND_ADDRESS ?? DEFAULT_BIND_ADDRESS;
const server = buildServer();

server.listen(port, bindAddress, () => {
  process.stdout.write(
    `${JSON.stringify({ event: "operator_bff_started", bind_address: bindAddress, port, timestamp_utc: new Date().toISOString() })}\n`,
  );
});
