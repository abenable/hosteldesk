/**
 * DEPLOYED flavor: Prisma client factory for Cloudflare Workers (workerd).
 *
 * The `cloudflare`-runtime client imports its WASM query compiler via
 * Wrangler's `*.wasm?module` syntax, so this module — and everything it
 * imports — must only appear in the Worker bundle graph (`src/index.ts`).
 * Never import this from dev.ts or tests.
 */
import { PrismaPg } from "@prisma/adapter-pg";
import { PrismaClient } from "../generated/prisma/client";

/**
 * Canonical client type for shared code. Both generated flavors expose the
 * same API for the same schema; type-only imports of this are erased at
 * runtime, so shared modules can reference the type safely.
 */
export type DbClient = PrismaClient;

/** Environment slice needed for database access. */
export interface DbEnv {
  DATABASE_URL: string;
}

/** Create a PrismaClient bound to `env.DATABASE_URL` (workerd flavor). */
export function createPrisma(env: DbEnv): DbClient {
  const adapter = new PrismaPg({ connectionString: env.DATABASE_URL });
  return new PrismaClient({ adapter });
}
