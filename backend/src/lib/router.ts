/**
 * Minimal `URLPattern`-based router.
 *
 * `URLPattern` is available in both target runtimes (workerd and Bun), so the
 * same routing code runs locally and deployed. No dependencies.
 */

/** Per-request base context handed to every route handler. */
export interface RouteContext {
  request: Request;
  url: URL;
  /** Named groups from the matched pathname pattern. */
  params: Record<string, string>;
}

/** Handler type parameterized by request-scoped extras (e.g. `db`). */
export type RouteHandler<C extends object> = (ctx: RouteContext & C) => Promise<Response> | Response;

interface Route<C extends object> {
  method: string;
  pattern: URLPattern;
  handler: RouteHandler<C>;
}

export class Router<C extends object> {
  readonly #routes: Route<C>[] = [];

  /** Register a handler for `method` at a URLPattern pathname like `/rooms/:id`. */
  add(method: string, pathname: string, handler: RouteHandler<C>): this {
    this.#routes.push({ method, handler, pattern: new URLPattern({ pathname }) });
    return this;
  }

  /**
   * Find and invoke the handler matching `request.method` and URL.
   * `extras` (e.g. `{ db }`) is merged into the handler context.
   * Returns `null` when no route matches (caller decides 404 behavior).
   */
  async dispatch(request: Request, extras: C): Promise<Response | null> {
    const url = new URL(request.url);
    for (const route of this.#routes) {
      if (route.method !== request.method) {
        continue;
      }
      const match = route.pattern.exec(url);
      if (match) {
        const params: Record<string, string> = {};
        for (const [key, value] of Object.entries(match.pathname.groups)) {
          if (value !== undefined) {
            params[key] = value;
          }
        }
        return route.handler({ request, url, params, ...extras });
      }
    }
    return null;
  }
}
