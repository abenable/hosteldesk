/**
 * LOCAL-ONLY Bun dev server. Never deployed — this file is the only place a
 * `Bun.*` API is allowed to appear. It runs the same application core
 * (`src/app.ts`) as the deployed Worker, injecting the bun-flavor Prisma
 * client instead of the cloudflare-flavor one.
 *
 * Bun auto-loads `.env` from this directory. Run with `bun --hot dev.ts`
 * (npm run dev).
 */
import { createApp, type Env, type WaitUntilCtx } from "./src/app";
import { createPrisma } from "./src/db/prisma-bun";

const env: Env = {
  DATABASE_URL: process.env.DATABASE_URL ?? "",
  ALLOWED_ORIGINS: process.env.ALLOWED_ORIGINS ?? "",
  BETTER_AUTH_SECRET: process.env.BETTER_AUTH_SECRET ?? "",
};

// Stand-in for the Workers ExecutionContext in local dev: fire-and-forget.
const ctx: WaitUntilCtx = {
  waitUntil(promise: Promise<unknown>) {
    promise.catch(() => {});
  },
};

const app = createApp(env, createPrisma);
const port = Number(process.env.PORT ?? 3000);

Bun.serve({
  port,
  fetch: (request) => app.fetch(request, ctx),
});

console.log(`hosteldesk-backend dev: http://localhost:${port}`);
