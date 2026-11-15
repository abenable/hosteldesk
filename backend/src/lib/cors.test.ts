import { describe, expect, test } from "bun:test";
import { createApp, type Env } from "../app";
import { parseAllowlist, resolveAllowedOrigin } from "./cors";

const ALLOW = ["https://app.example.com", "http://localhost:1420"];

describe("resolveAllowedOrigin", () => {
  test("allows an exact origin", () => {
    expect(resolveAllowedOrigin("https://app.example.com", ALLOW)).toBe("https://app.example.com");
  });

  test("rejects prefix-spoofed origins", () => {
    expect(resolveAllowedOrigin("https://app.example.com.attacker.example", ALLOW)).toBeNull();
  });

  test("rejects subdomains (exact match only)", () => {
    expect(resolveAllowedOrigin("https://evil.app.example.com", ALLOW)).toBeNull();
  });

  test("rejects the opaque null origin", () => {
    expect(resolveAllowedOrigin("null", ALLOW)).toBeNull();
  });

  test("rejects missing and unparseable origins", () => {
    expect(resolveAllowedOrigin(null, ALLOW)).toBeNull();
    expect(resolveAllowedOrigin("", ALLOW)).toBeNull();
    expect(resolveAllowedOrigin("not a url", ALLOW)).toBeNull();
  });
});

describe("entry handler", () => {
  const env: Env = {
    DATABASE_URL: "",
    ALLOWED_ORIGINS: "https://app.example.com",
    BETTER_AUTH_SECRET: "",
  };

  // DB-free tests: the factory must never be called (db is lazy).
  const failDb = (): never => {
    throw new Error("db should not be created for these routes");
  };

  const app = createApp(env, failDb);

  test("health returns json without touching the database", async () => {
    const res = await app.fetch(new Request("https://worker.example.com/health"));
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toContain("application/json");
    expect(await res.json()).toEqual({ ok: true });
  });

  test("unknown routes 404 with an error body", async () => {
    const res = await app.fetch(new Request("https://worker.example.com/nope"));
    expect(res.status).toBe(404);
    expect(await res.json()).toEqual({ error: { code: "not_found", message: "route not found" } });
  });

  test("allowed origin gets CORS headers", async () => {
    const res = await app.fetch(
      new Request("https://worker.example.com/health", {
        headers: { origin: "https://app.example.com" },
      }),
    );
    expect(res.headers.get("access-control-allow-origin")).toBe("https://app.example.com");
    expect(res.headers.get("access-control-allow-credentials")).toBe("true");
    expect(res.headers.get("vary")).toBe("Origin");
  });

  test("disallowed origin gets no CORS grant", async () => {
    const res = await app.fetch(
      new Request("https://worker.example.com/health", {
        headers: { origin: "https://attacker.example" },
      }),
    );
    expect(res.headers.get("access-control-allow-origin")).toBeNull();
  });

  test("preflight answered with allow methods only for allowed origins", async () => {
    const ok = await app.fetch(
      new Request("https://worker.example.com/health", {
        method: "OPTIONS",
        headers: { origin: "https://app.example.com" },
      }),
    );
    expect(ok.status).toBe(204);
    expect(ok.headers.get("access-control-allow-methods")).toContain("POST");

    const bad = await app.fetch(
      new Request("https://worker.example.com/health", {
        method: "OPTIONS",
        headers: { origin: "https://attacker.example" },
      }),
    );
    expect(bad.headers.get("access-control-allow-methods")).toBeNull();
  });
});
