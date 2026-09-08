import { createHmac, timingSafeEqual } from "node:crypto";

import type { OperatorPrincipal, OperatorRole } from "./contracts.js";

interface JwtHeader {
  alg?: unknown;
  typ?: unknown;
}

interface JwtPayload {
  sub?: unknown;
  role?: unknown;
  exp?: unknown;
}

export class AuthenticationError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "AuthenticationError";
  }
}

function parseJsonPart<T>(part: string, label: string): T {
  try {
    return JSON.parse(Buffer.from(part, "base64url").toString("utf8")) as T;
  } catch {
    throw new AuthenticationError("invalid_token", `Invalid JWT ${label}.`);
  }
}

function isOperatorRole(value: unknown): value is OperatorRole {
  return value === "viewer" || value === "operator" || value === "admin";
}

export function verifyOperatorBearerToken(
  authorization: string | undefined,
  secret: string,
  nowSeconds = Math.floor(Date.now() / 1000),
): OperatorPrincipal {
  if (secret.length < 32) {
    throw new AuthenticationError(
      "auth_not_configured",
      "Operator BFF auth secret must be at least 32 characters.",
    );
  }
  if (!authorization?.startsWith("Bearer ")) {
    throw new AuthenticationError("missing_bearer_token", "Bearer token required.");
  }

  const token = authorization.slice("Bearer ".length).trim();
  const parts = token.split(".");
  if (parts.length !== 3) {
    throw new AuthenticationError("invalid_token", "JWT must have three parts.");
  }
  const [encodedHeader, encodedPayload, encodedSignature] = parts;
  if (!encodedHeader || !encodedPayload || !encodedSignature) {
    throw new AuthenticationError("invalid_token", "JWT contains an empty part.");
  }

  const header = parseJsonPart<JwtHeader>(encodedHeader, "header");
  if (header.alg !== "HS256") {
    throw new AuthenticationError("unsupported_alg", "Only HS256 is accepted.");
  }

  const expected = createHmac("sha256", secret)
    .update(`${encodedHeader}.${encodedPayload}`)
    .digest();
  let supplied: Buffer;
  try {
    supplied = Buffer.from(encodedSignature, "base64url");
  } catch {
    throw new AuthenticationError("invalid_token", "JWT signature is malformed.");
  }
  if (supplied.length !== expected.length || !timingSafeEqual(supplied, expected)) {
    throw new AuthenticationError("invalid_signature", "JWT signature is invalid.");
  }

  const payload = parseJsonPart<JwtPayload>(encodedPayload, "payload");
  if (typeof payload.sub !== "string" || payload.sub.length === 0) {
    throw new AuthenticationError("invalid_subject", "JWT subject is required.");
  }
  if (!isOperatorRole(payload.role)) {
    throw new AuthenticationError("invalid_role", "JWT role is not allowed.");
  }
  if (typeof payload.exp !== "number" || !Number.isFinite(payload.exp)) {
    throw new AuthenticationError("invalid_expiry", "JWT expiry is required.");
  }
  if (payload.exp <= nowSeconds) {
    throw new AuthenticationError("token_expired", "JWT is expired.");
  }

  return { sub: payload.sub, role: payload.role };
}
