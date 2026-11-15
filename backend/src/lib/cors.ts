/**
 * CORS origin validation.
 *
 * Per the validate-origins guidance: compare origins structurally, never with
 * `startsWith`/substring matching, and always reject the opaque `"null"`
 * origin string. The `Origin` API is browser-only, so this uses the endorsed
 * `new URL()` fallback: parse, then compare the normalized serialized origin
 * for exact equality against the allowlist.
 */

/** Headers returned on successful CORS preflights (never for disallowed origins). */
export const PREFLIGHT_HEADERS: Record<string, string> = {
  "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
  "access-control-allow-headers": "content-type, authorization",
  "access-control-max-age": "86400",
};

/** Split the comma-separated `ALLOWED_ORIGINS` var into a trimmed list. */
export function parseAllowlist(raw: string | undefined): string[] {
  return (raw ?? "")
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

/**
 * Return the allowed origin matching `origin`, or `null`.
 *
 * Exact `(scheme, host, port)` match only — subdomain prefix spoofs like
 * `https://app.example.com.attacker.example`, opaque `"null"` origins, and
 * unparseable values are all rejected.
 */
export function resolveAllowedOrigin(origin: string | null, allowlist: string[]): string | null {
  if (!origin || origin === "null") {
    return null;
  }
  let parsed: URL;
  try {
    parsed = new URL(origin);
  } catch {
    return null;
  }
  return allowlist.includes(parsed.origin) ? parsed.origin : null;
}
