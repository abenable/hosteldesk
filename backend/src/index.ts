/**
 * Cloudflare Workers entry point.
 *
 * Only Web-standard APIs here, plus the cloudflare-flavor Prisma client —
 * this module and its imports form the entire Worker bundle. The identical
 * application core runs locally under Bun via `dev.ts` with the bun-flavor
 * client injected instead.
 */
import { createApp, type Env, type WaitUntilCtx } from "./app";
import { createPrisma } from "./db/prisma-cf";

export type { Env } from "./app";

export default {
  fetch: (request: Request, env: Env, ctx?: WaitUntilCtx) =>
    createApp(env, createPrisma).fetch(request, ctx),
};
