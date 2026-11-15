/**
 * LOCAL flavor: Prisma client factory for Bun (dev.ts, bun tests).
 *
 * The `bun`-runtime client compiles its WASM query compiler at runtime — fine
 * under Bun, forbidden in workerd. Import this ONLY from Bun-only code paths;
 * it must never enter the Worker bundle graph.
 */
import { PrismaPg } from "@prisma/adapter-pg";
import { PrismaClient as BunPrismaClient } from "../generated/prisma-bun/client";
import type { DbClient } from "./prisma-cf";

/** Create a PrismaClient bound to `env.DATABASE_URL` (Bun flavor). */
export function createPrisma(env: { DATABASE_URL: string }): DbClient {
  const adapter = new PrismaPg({ connectionString: env.DATABASE_URL });
  // Both flavors are generated from the same schema with the same public
  // API; the types differ only in generator bookkeeping, hence the cast.
  return new BunPrismaClient({ adapter }) as unknown as DbClient;
}
