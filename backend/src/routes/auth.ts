/**
 * better-auth HTTP handler bridge. The raw Request (cookies, body, origin
 * intact) is handed to better-auth; its Response (including Set-Cookie
 * headers) flows back through the shared CORS/error wrapper.
 */
import type { AppContext } from "../app";
import type { RouteContext } from "../lib/router";
import { createAuth } from "../auth";

export async function handleAuth(ctx: RouteContext & AppContext): Promise<Response> {
  const db = await ctx.getDb();
  const auth = createAuth(db, ctx.env);
  return auth.handler(ctx.request);
}
