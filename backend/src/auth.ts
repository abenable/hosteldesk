/**
 * better-auth instance factory.
 *
 * Created per request together with the request-scoped Prisma client (the
 * recommended pattern for edge/serverless runtimes). Uses the Prisma adapter
 * against whichever client flavor the active entry point injected.
 *
 * Cookie policy: better-auth defaults (httpOnly; Secure in production). We
 * deliberately do NOT force `__Host-` prefixed secure-only cookies: the
 * desktop Tauri webview origin is `http://tauri.localhost` on Windows, which
 * would reject Secure cookies and break desktop login.
 */
import { betterAuth } from "better-auth";
import { prismaAdapter } from "@better-auth/prisma-adapter";
import type { Env } from "./app";
import type { DbClient } from "./db/prisma-cf";
import { parseAllowlist } from "./lib/cors";

/** Create a better-auth instance bound to `db` and `env`. */
export function createAuth(db: DbClient, env: Env) {
  return betterAuth({
    database: prismaAdapter(db as never, { provider: "postgresql" }),
    secret: env.BETTER_AUTH_SECRET,
    trustedOrigins: parseAllowlist(env.ALLOWED_ORIGINS),
    // The API is reached on different hosts across environments (local dev
    // server, workers.dev, a future custom domain), so the base URL is
    // resolved per request and validated against an allowlist.
    baseURL: {
      // Patterns match the full Host string, ports included ("localhost" does
      // NOT match "localhost:3000").
      allowedHosts: ["localhost", "localhost:*", "127.0.0.1", "127.0.0.1:*", "*.workers.dev"],
    },
    advanced: {
      // Session/account joins — relations exist on User/Session/Account.
      database: { joins: true },
    },
    emailAndPassword: {
      enabled: true,
      // Mirrors the desktop app's server-side password rule.
      minPasswordLength: 8,
    },
    user: {
      additionalFields: {
        // HostelDesk-specific user data. `input: false` — clients can never
        // set these via sign-up; they're managed by our user-management
        // endpoints (and by onboarding) directly against the database,
        // which supplies defaults ("manager", NULL hostel).
        hostelId: {
          type: "number",
          required: false,
          input: false,
        },
        role: {
          type: "string",
          required: false,
          input: false,
        },
      },
    },
  });
}

export type Auth = ReturnType<typeof createAuth>;
