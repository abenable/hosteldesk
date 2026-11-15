/**
 * Prisma CLI configuration (Prisma ORM v7).
 *
 * The connection URL comes from DATABASE_URL (backend/.env for local dev,
 * the Neon pooled URL in production/CI).
 */
import "dotenv/config";
import { defineConfig, env } from "prisma/config";

export default defineConfig({
  schema: "prisma/schema.prisma",
  datasource: {
    url: env("DATABASE_URL"),
  },
});
