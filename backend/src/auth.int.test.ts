/**
 * better-auth integration tests: full round trip through the shared app
 * (createApp + bun-flavor Prisma) against the local Postgres database.
 */
import { describe, expect, test } from "bun:test";
import { createApp, type Env } from "./app";
import { createPrisma } from "./db/prisma-bun";

const hasDb = Boolean(process.env.DATABASE_URL);

const env: Env = {
  DATABASE_URL: process.env.DATABASE_URL ?? "",
  ALLOWED_ORIGINS: "http://localhost:1420",
  BETTER_AUTH_SECRET: "test-secret-for-better-auth-signing-min-32-chars",
};

const app = createApp(env, createPrisma);

const ORIGIN = "http://localhost:1420";

function cookieHeader(res: Response): string {
  return res.headers
    .getSetCookie()
    .map((cookie) => cookie.split(";")[0])
    .join("; ");
}

async function req(
  method: string,
  path: string,
  body?: unknown,
  cookie?: string,
): Promise<Response> {
  return app.fetch(
    new Request(`http://localhost:3000${path}`, {
      method,
      headers: {
        "content-type": "application/json",
        origin: ORIGIN,
        ...(cookie ? { cookie } : {}),
      },
      body: body === undefined ? undefined : JSON.stringify(body),
    }),
  );
}

describe("better-auth round trip", () => {
  test.skipIf(!hasDb)("sign-up → session → sign-out", async () => {
    const email = `__test__${crypto.randomUUID()}@example.com`;

    // Weak passwords are rejected server-side.
    const weak = await req("POST", "/api/auth/sign-up/email", {
      email,
      password: "short",
      name: "Weak Password",
    });
    expect(weak.ok).toBe(false);

    // Sign-up succeeds and sets a session cookie.
    const signup = await req("POST", "/api/auth/sign-up/email", {
      email,
      password: "password123",
      name: "Test User",
    });
    expect(signup.status).toBe(200);
    const cookie = cookieHeader(signup);
    expect(cookie).toContain("better-auth.session_token");

    // The user row carries HostelDesk defaults: role manager, no hostel yet.
    const db = createPrisma(env);
    const user = await db.user.findUniqueOrThrow({ where: { email } });
    expect(user.role).toBe("manager");
    expect(user.hostelId).toBeNull();
    await db.$disconnect();

    // Session is readable with the cookie, including additional fields.
    const session = await req("GET", "/api/auth/get-session", undefined, cookie);
    expect(session.status).toBe(200);
    const sessionBody = (await session.json()) as {
      user?: { email: string; role: string; hostelId: number | null } | null;
    };
    expect(sessionBody.user?.email).toBe(email);
    expect(sessionBody.user?.role).toBe("manager");

    // Sign-out revokes the session server-side.
    const signout = await req("POST", "/api/auth/sign-out", undefined, cookie);
    expect(signout.status).toBe(200);
    const after = await req("GET", "/api/auth/get-session", undefined, cookie);
    // get-session answers 200 with a null body (or { user: null }) once the
    // session is revoked.
    const afterBody = (await after.json()) as { user?: unknown } | null;
    expect(afterBody?.user ?? null).toBeNull();

    // Cleanup: removing the user cascades sessions + accounts.
    const db2 = createPrisma(env);
    await db2.user.delete({ where: { email } });
    await db2.$disconnect();
  });

  test.skipIf(!hasDb)("sign-in with wrong password is rejected", async () => {
    const email = `__test__${crypto.randomUUID()}@example.com`;
    const signup = await req("POST", "/api/auth/sign-up/email", {
      email,
      password: "password123",
      name: "Test User",
    });
    expect(signup.status).toBe(200);

    const bad = await req("POST", "/api/auth/sign-in/email", {
      email,
      password: "wrong-password",
    });
    expect(bad.status).toBe(401);

    const good = await req("POST", "/api/auth/sign-in/email", {
      email,
      password: "password123",
    });
    expect(good.status).toBe(200);
    expect(cookieHeader(good)).toContain("better-auth.session_token");

    const db = createPrisma(env);
    await db.user.delete({ where: { email } });
    await db.$disconnect();
  });
});
