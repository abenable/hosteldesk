/**
 * JSON response helpers and error mapping.
 *
 * Mirrors the desktop app's error semantics (crates/app/src/commands/support.rs):
 * `"unauthorized"` → 401, `"forbidden"` → 403, anything else is a 4xx/5xx
 * with the human-readable message.
 */

/** An error carrying an HTTP status and machine-readable code. */
export class HttpError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

/** Typed API error bodies: `{ error: { code, message } }`. */
export interface ErrorBody {
  error: { code: string; message: string };
}

/** Base headers on every JSON response (see security guidance: nosniff, CORP). */
const BASE_HEADERS: Record<string, string> = {
  "content-type": "application/json; charset=utf-8",
  "x-content-type-options": "nosniff",
  "cross-origin-resource-policy": "same-origin",
};

/** Serialize `data` as a JSON response with security headers applied. */
export function json(
  data: unknown,
  init: { status?: number; headers?: Record<string, string> } = {},
): Response {
  const headers = new Headers({ ...BASE_HEADERS, ...init.headers });
  return new Response(JSON.stringify(data), { status: init.status ?? 200, headers });
}

/** Build the JSON error response for any thrown value. */
export function errorResponse(error: unknown): Response {
  if (error instanceof HttpError) {
    const body: ErrorBody = { error: { code: error.code, message: error.message } };
    return json(body, { status: error.status });
  }
  // Unexpected errors are logged for observability but never leaked to clients.
  console.error("unhandled error:", error);
  const body: ErrorBody = {
    error: { code: "internal", message: "Something went wrong while processing your request." },
  };
  return json(body, { status: 500 });
}
