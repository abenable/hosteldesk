/**
 * Shared application core — runtime-agnostic.
 *
 * Contains everything both entry points (the deployed Worker in
 * `src/index.ts` and the local Bun server in `dev.ts`) have in common:
 * CORS/preflight handling, routing, error mapping, and request-scoped
 * database access. The only per-runtime difference is which Prisma client
 * factory gets injected.
 */
import type { DbClient } from "./db/prisma-cf";
import { errorResponse, json } from "./lib/http";
import { PREFLIGHT_HEADERS, parseAllowlist, resolveAllowedOrigin } from "./lib/cors";
import { buildRouter } from "./routes";

/**
 * Worker bindings/vars. Secrets (`DATABASE_URL`, `BETTER_AUTH_SECRET`) are
 * set via `npx wrangler secret put`; `ALLOWED_ORIGINS` comes from
 * wrangler.jsonc.
 */
export interface Env {
  DATABASE_URL: string;
  ALLOWED_ORIGINS?: string;
  BETTER_AUTH_SECRET: string;
}

/** Request-scoped context handed to route handlers. */
export interface AppContext {
  /** Lazily-created per-request Prisma client. Handlers call this when they need the DB. */
  getDb(): Promise<DbClient>;
  /** Worker env (secrets + vars). */
  env: Env;
}

/** Structurally compatible with Workers' `ExecutionContext`. */
export interface WaitUntilCtx {
  waitUntil(promise: Promise<unknown>): void;
}

/** Builds the route table. Shared by both runtimes. */
export function createApp(env: Env, createDb: (env: Env) => DbClient) {
  const router = buildRouter();

  return {
    async fetch(request: Request, ctx?: WaitUntilCtx): Promise<Response> {
      const allowlist = parseAllowlist(env.ALLOWED_ORIGINS);
      const allowedOrigin = resolveAllowedOrigin(request.headers.get("origin"), allowlist);

      // Preflight: answer before any DB work or auth runs, only for allowed origins.
      if (request.method === "OPTIONS") {
        const headers = allowedOrigin
          ? {
              ...PREFLIGHT_HEADERS,
              "access-control-allow-origin": allowedOrigin,
              "access-control-allow-credentials": "true",
              vary: "Origin",
            }
          : { vary: "Origin" };
        return new Response(null, { status: 204, headers });
      }

      // Per-request Prisma client, created lazily so routes like /health and
      // unmatched 404s never open a connection. Released via waitUntil.
      const dbRef: { promise?: Promise<DbClient> } = {};
      const getDb = (): Promise<DbClient> =>
        (dbRef.promise ??= Promise.resolve(createDb(env)));

      try {
        const matched = await router.dispatch(request, { getDb, env });
        const response =
          matched ??
          json({ error: { code: "not_found", message: "route not found" } }, { status: 404 });
        if (dbRef.promise) {
          ctx?.waitUntil(
            dbRef.promise.then((db: DbClient) => db.$disconnect()).catch(reportDisconnect),
          );
        }
        return withCors(response, allowedOrigin);
      } catch (error) {
        if (dbRef.promise) {
          ctx?.waitUntil(
            dbRef.promise.then((db: DbClient) => db.$disconnect()).catch(reportDisconnect),
          );
        }
        return withCors(errorResponse(error), allowedOrigin);
      }
    },
  };
}

function reportDisconnect(error: unknown): void {
  console.error("prisma disconnect failed:", error);
}

/** Stamp CORS headers onto a response when the request origin is allowed. */
function withCors(response: Response, allowedOrigin: string | null): Response {
  const headers = new Headers(response.headers);
  headers.set("vary", "Origin");
  if (allowedOrigin) {
    headers.set("access-control-allow-origin", allowedOrigin);
    headers.set("access-control-allow-credentials", "true");
  }
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}
