/**
 * Route table. All endpoint registrations live here; shared by the deployed
 * Worker and the local Bun dev server. Handlers receive `ctx.getDb()` for
 * database access (lazily created per request) and `ctx.env`.
 */
import type { AppContext } from "../app";
import { Router } from "../lib/router";
import { json } from "../lib/http";
import { handleAuth } from "./auth";

export function buildRouter(): Router<AppContext> {
  const router = new Router<AppContext>();

  // Liveness probe — deliberately DB-free.
  router.add("GET", "/health", () => json({ ok: true }));

  // better-auth catch-all: /api/auth/* (sign-up, sign-in, sign-out,
  // get-session, etc.). Must be registered before any more specific
  // /api/auth/... routes if we add custom ones later.
  for (const method of ["GET", "POST", "PUT", "DELETE"] as const) {
    router.add(method, "/api/auth/:path*", handleAuth);
  }

  return router;
}
