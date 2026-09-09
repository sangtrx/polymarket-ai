import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import test from "node:test";

import { AuthenticationError, verifyOperatorBearerToken } from "../src/auth.js";

const SECRET = "0123456789abcdef0123456789abcdef";

function signToken(payload: Record<string, unknown>): string {
  const header = Buffer.from(JSON.stringify({ alg: "HS256", typ: "JWT" })).toString(
    "base64url",
  );
  const body = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const signature = createHmac("sha256", SECRET)
    .update(`${header}.${body}`)
    .digest("base64url");
  return `${header}.${body}.${signature}`;
}

test("verified operator JWT yields typed principal", () => {
  const token = signToken({ sub: "operator-7", role: "operator", exp: 2_000_000_000 });
  assert.deepEqual(
    verifyOperatorBearerToken(`Bearer ${token}`, SECRET, 1_900_000_000),
    { sub: "operator-7", role: "operator" },
  );
});

test("tampered operator JWT fails closed", () => {
  const token = signToken({ sub: "operator-7", role: "operator", exp: 2_000_000_000 });
  const [header, payload] = token.split(".");
  const tamperedPayload = Buffer.from(
    JSON.stringify({ sub: "operator-7", role: "admin", exp: 2_000_000_000 }),
  ).toString("base64url");

  assert.throws(
    () =>
      verifyOperatorBearerToken(
        `Bearer ${header}.${tamperedPayload}.${token.split(".")[2]}`,
        SECRET,
        1_900_000_000,
      ),
    (error: unknown) =>
      error instanceof AuthenticationError && error.code === "invalid_signature",
  );
  assert.notEqual(payload, tamperedPayload);
});

test("expired operator JWT is rejected", () => {
  const token = signToken({ sub: "operator-7", role: "operator", exp: 100 });
  assert.throws(
    () => verifyOperatorBearerToken(`Bearer ${token}`, SECRET, 101),
    (error: unknown) =>
      error instanceof AuthenticationError && error.code === "token_expired",
  );
});
